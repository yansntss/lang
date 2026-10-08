use std::future::Future;

use serde::Serialize;

use super::lang_detect::{detect, Detection};
use crate::error::AppError;

/// Tamanho máximo, em caracteres, de um texto enviado para tradução.
pub const MAX_INPUT_CHARS: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TargetLang {
    #[serde(rename = "PT-BR")]
    PtBr,
    #[serde(rename = "PT-PT")]
    PtPt,
    #[serde(rename = "EN-US")]
    EnUs,
    #[serde(rename = "EN-GB")]
    EnGb,
}

impl TargetLang {
    /// Código aceito pelo parâmetro `target_lang` do DeepL.
    pub fn deepl_code(self) -> &'static str {
        match self {
            Self::PtBr => "PT-BR",
            Self::PtPt => "PT-PT",
            Self::EnUs => "EN-US",
            Self::EnGb => "EN-GB",
        }
    }
}

/// Para qual variante traduzir em cada direção: do inglês para o português, e de qualquer outro
/// idioma para o inglês.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetLangs {
    pub to_portuguese: TargetLang,
    pub to_english: TargetLang,
}

impl Default for TargetLangs {
    fn default() -> Self {
        Self {
            to_portuguese: TargetLang::PtBr,
            to_english: TargetLang::EnUs,
        }
    }
}

/// Resposta crua de um tradutor: texto traduzido e idioma de origem detectado (ex.: `"EN"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTranslation {
    pub text: String,
    pub detected_source: String,
}

/// Resultado entregue ao front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub text: String,
    pub source_lang: String,
    pub target_lang: TargetLang,
}

pub trait Translator: Send + Sync {
    fn translate(
        &self,
        text: &str,
        target: TargetLang,
    ) -> impl Future<Output = Result<RawTranslation, AppError>> + Send;
}

pub struct TranslationService<T: Translator> {
    translator: T,
}

impl<T: Translator> TranslationService<T> {
    pub fn new(translator: T) -> Self {
        Self { translator }
    }

    /// O tradutor por baixo, para operações que não são traduções (conferir a chave).
    pub fn translator(&self) -> &T {
        &self.translator
    }

    /// Traduz com as variantes padrão (PT-BR e EN-US).
    pub async fn translate(&self, text: &str) -> Result<Translation, AppError> {
        self.translate_with(text, TargetLangs::default()).await
    }

    pub async fn translate_with(
        &self,
        text: &str,
        langs: TargetLangs,
    ) -> Result<Translation, AppError> {
        let text = validate(text)?;

        match detect(text) {
            Detection::English => self.translate_to(text, langs.to_portuguese).await,
            Detection::Other => self.translate_to(text, langs.to_english).await,
            Detection::Unsure => self.translate_when_unsure(text, langs).await,
        }
    }

    async fn translate_to(&self, text: &str, target: TargetLang) -> Result<Translation, AppError> {
        let raw = self.translator.translate(text, target).await?;
        Ok(Translation::from_raw(raw, target))
    }

    /// A detecção local não bastou: pede PT-BR ao DeepL e usa o idioma que ele detectou.
    /// Só refaz a chamada (para EN-US) se a origem não for inglês.
    async fn translate_when_unsure(
        &self,
        text: &str,
        langs: TargetLangs,
    ) -> Result<Translation, AppError> {
        let first = self.translator.translate(text, langs.to_portuguese).await?;
        if is_english(&first.detected_source) {
            return Ok(Translation::from_raw(first, langs.to_portuguese));
        }
        self.translate_to(text, langs.to_english).await
    }
}

impl Translation {
    fn from_raw(raw: RawTranslation, target_lang: TargetLang) -> Self {
        Self {
            text: raw.text,
            source_lang: raw.detected_source,
            target_lang,
        }
    }
}

fn validate(text: &str) -> Result<&str, AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(
            "Digite ou selecione um texto para traduzir.".into(),
        ));
    }
    if trimmed.chars().count() > MAX_INPUT_CHARS {
        return Err(AppError::InvalidInput(format!(
            "O texto excede o limite de {MAX_INPUT_CHARS} caracteres."
        )));
    }
    Ok(trimmed)
}

fn is_english(detected_source: &str) -> bool {
    detected_source
        .trim()
        .to_ascii_uppercase()
        .starts_with("EN")
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    const ENGLISH_SENTENCE: &str =
        "The quick brown fox jumps over the lazy dog while the farmer is sleeping.";
    const PORTUGUESE_SENTENCE: &str =
        "O rato roeu a roupa do rei de Roma e fugiu para o campo sem olhar para trás.";

    /// Tradutor falso: devolve respostas roteirizadas e registra cada chamada.
    struct FakeTranslator {
        responses: Mutex<VecDeque<Result<RawTranslation, AppError>>>,
        calls: Mutex<Vec<(String, TargetLang)>>,
    }

    impl FakeTranslator {
        fn replying(responses: Vec<Result<RawTranslation, AppError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, TargetLang)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Translator for FakeTranslator {
        async fn translate(
            &self,
            text: &str,
            target: TargetLang,
        ) -> Result<RawTranslation, AppError> {
            self.calls.lock().unwrap().push((text.to_owned(), target));
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("o teste não roteirizou resposta suficiente")
        }
    }

    fn raw(text: &str, detected: &str) -> Result<RawTranslation, AppError> {
        Ok(RawTranslation {
            text: text.to_owned(),
            detected_source: detected.to_owned(),
        })
    }

    #[tokio::test]
    async fn translates_english_to_pt_br_with_a_single_call() {
        let service = TranslationService::new(FakeTranslator::replying(vec![raw("raposa", "EN")]));

        let result = service.translate(ENGLISH_SENTENCE).await.unwrap();

        assert_eq!(result.text, "raposa");
        assert_eq!(result.source_lang, "EN");
        assert_eq!(result.target_lang, TargetLang::PtBr);
        assert_eq!(
            service.translator.calls(),
            vec![(ENGLISH_SENTENCE.to_owned(), TargetLang::PtBr)]
        );
    }

    #[tokio::test]
    async fn translates_other_languages_to_en_us_with_a_single_call() {
        let service = TranslationService::new(FakeTranslator::replying(vec![raw("rat", "PT")]));

        let result = service.translate(PORTUGUESE_SENTENCE).await.unwrap();

        assert_eq!(result.target_lang, TargetLang::EnUs);
        assert_eq!(result.source_lang, "PT");
        assert_eq!(
            service.translator.calls(),
            vec![(PORTUGUESE_SENTENCE.to_owned(), TargetLang::EnUs)]
        );
    }

    const BRITISH_PORTUGUESE: TargetLangs = TargetLangs {
        to_portuguese: TargetLang::PtPt,
        to_english: TargetLang::EnGb,
    };

    #[tokio::test]
    async fn english_text_goes_to_the_configured_portuguese_variant() {
        let service = TranslationService::new(FakeTranslator::replying(vec![raw("raposa", "EN")]));

        let result = service
            .translate_with(ENGLISH_SENTENCE, BRITISH_PORTUGUESE)
            .await
            .unwrap();

        assert_eq!(result.target_lang, TargetLang::PtPt);
        assert_eq!(
            service.translator.calls(),
            vec![(ENGLISH_SENTENCE.to_owned(), TargetLang::PtPt)]
        );
    }

    #[tokio::test]
    async fn other_languages_go_to_the_configured_english_variant() {
        let service = TranslationService::new(FakeTranslator::replying(vec![raw("rat", "PT")]));

        let result = service
            .translate_with(PORTUGUESE_SENTENCE, BRITISH_PORTUGUESE)
            .await
            .unwrap();

        assert_eq!(result.target_lang, TargetLang::EnGb);
        assert_eq!(
            service.translator.calls(),
            vec![(PORTUGUESE_SENTENCE.to_owned(), TargetLang::EnGb)]
        );
    }

    #[tokio::test]
    async fn unsure_text_uses_the_configured_variants_in_both_calls() {
        let service = TranslationService::new(FakeTranslator::replying(vec![
            raw("bom dia", "PT"),
            raw("good morning", "PT"),
        ]));

        let result = service
            .translate_with("bom", BRITISH_PORTUGUESE)
            .await
            .unwrap();

        assert_eq!(result.target_lang, TargetLang::EnGb);
        assert_eq!(
            service.translator.calls(),
            vec![
                ("bom".to_owned(), TargetLang::PtPt),
                ("bom".to_owned(), TargetLang::EnGb),
            ]
        );
    }

    #[test]
    fn every_variant_has_its_own_deepl_code() {
        assert_eq!(TargetLang::PtBr.deepl_code(), "PT-BR");
        assert_eq!(TargetLang::PtPt.deepl_code(), "PT-PT");
        assert_eq!(TargetLang::EnUs.deepl_code(), "EN-US");
        assert_eq!(TargetLang::EnGb.deepl_code(), "EN-GB");
    }

    #[tokio::test]
    async fn unsure_text_detected_as_english_by_deepl_keeps_the_first_result() {
        let service =
            TranslationService::new(FakeTranslator::replying(vec![raw("obrigado", "EN")]));

        let result = service.translate("thanks").await.unwrap();

        assert_eq!(result.text, "obrigado");
        assert_eq!(result.target_lang, TargetLang::PtBr);
        assert_eq!(
            service.translator.calls(),
            vec![("thanks".to_owned(), TargetLang::PtBr)]
        );
    }

    #[tokio::test]
    async fn unsure_text_not_detected_as_english_is_retranslated_to_en_us() {
        let service = TranslationService::new(FakeTranslator::replying(vec![
            raw("bom dia", "PT"),
            raw("good morning", "PT"),
        ]));

        let result = service.translate("bom dia").await.unwrap();

        assert_eq!(result.text, "good morning");
        assert_eq!(result.source_lang, "PT");
        assert_eq!(result.target_lang, TargetLang::EnUs);
        assert_eq!(
            service.translator.calls(),
            vec![
                ("bom dia".to_owned(), TargetLang::PtBr),
                ("bom dia".to_owned(), TargetLang::EnUs),
            ]
        );
    }

    #[tokio::test]
    async fn trims_surrounding_whitespace_before_translating() {
        let service =
            TranslationService::new(FakeTranslator::replying(vec![raw("obrigado", "EN")]));

        service.translate("  \n thanks \t ").await.unwrap();

        assert_eq!(
            service.translator.calls(),
            vec![("thanks".to_owned(), TargetLang::PtBr)]
        );
    }

    #[tokio::test]
    async fn rejects_blank_text_without_calling_the_translator() {
        let service = TranslationService::new(FakeTranslator::replying(vec![]));

        let error = service.translate("  \n\t ").await.unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert!(service.translator.calls().is_empty());
    }

    #[tokio::test]
    async fn rejects_text_over_the_limit_without_calling_the_translator() {
        let service = TranslationService::new(FakeTranslator::replying(vec![]));
        let too_long = "a".repeat(MAX_INPUT_CHARS + 1);

        let error = service.translate(&too_long).await.unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert!(service.translator.calls().is_empty());
    }

    #[tokio::test]
    async fn accepts_text_exactly_at_the_limit_counting_characters_not_bytes() {
        let service = TranslationService::new(FakeTranslator::replying(vec![raw("x", "EN")]));
        let at_limit = "é".repeat(MAX_INPUT_CHARS);

        let result = service.translate(&at_limit).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn propagates_translator_errors_without_retrying() {
        let service =
            TranslationService::new(FakeTranslator::replying(vec![Err(AppError::QuotaExceeded)]));

        let error = service.translate("bom dia").await.unwrap_err();

        assert_eq!(error.code(), "quota_exceeded");
        assert_eq!(service.translator.calls().len(), 1);
    }

    #[test]
    fn serializes_translation_for_the_front() {
        let translation = Translation {
            text: "olá".into(),
            source_lang: "EN".into(),
            target_lang: TargetLang::PtBr,
        };

        let json = serde_json::to_string(&translation).unwrap();

        assert_eq!(
            json,
            r#"{"text":"olá","sourceLang":"EN","targetLang":"PT-BR"}"#
        );
    }
}
