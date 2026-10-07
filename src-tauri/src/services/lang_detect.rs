use whatlang::{Detector, Lang};

/// Idiomas plausíveis como origem. Restringir o detector evita confusões como inglês
/// detectado como afrikaans ou norueguês, comuns quando todos os idiomas competem.
const CANDIDATE_LANGS: [Lang; 6] = [
    Lang::Eng,
    Lang::Por,
    Lang::Spa,
    Lang::Fra,
    Lang::Deu,
    Lang::Ita,
];

/// Resultado da detecção local de idioma do texto de origem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detection {
    English,
    Other,
    /// Texto curto ou ambíguo demais para decidir sem ajuda do DeepL.
    Unsure,
}

pub fn detect(text: &str) -> Detection {
    let detector = Detector::with_allowlist(CANDIDATE_LANGS.to_vec());

    match detector.detect(text) {
        Some(info) if info.is_reliable() && info.lang() == Lang::Eng => Detection::English,
        Some(info) if info.is_reliable() => Detection::Other,
        _ => Detection::Unsure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_clearly_english_sentence() {
        let text = "The quick brown fox jumps over the lazy dog while the farmer is sleeping.";

        assert_eq!(detect(text), Detection::English);
    }

    #[test]
    fn does_not_mistake_english_for_afrikaans_or_norwegian() {
        let text = "I would like to know how this works because I am learning English.";

        assert_eq!(detect(text), Detection::English);
    }

    #[test]
    fn detects_a_clearly_portuguese_sentence() {
        let text = "O rato roeu a roupa do rei de Roma e fugiu para o campo sem olhar para trás.";

        assert_eq!(detect(text), Detection::Other);
    }

    #[test]
    fn detects_a_clearly_spanish_sentence_as_other() {
        let text =
            "El rápido zorro marrón salta sobre el perro perezoso mientras el granjero duerme.";

        assert_eq!(detect(text), Detection::Other);
    }

    #[test]
    fn is_unsure_for_very_short_text() {
        assert_eq!(detect("ok"), Detection::Unsure);
    }

    #[test]
    fn is_unsure_for_a_short_portuguese_phrase_instead_of_guessing_wrong() {
        assert_eq!(
            detect("Eu gostaria de saber como isso funciona"),
            Detection::Unsure
        );
    }

    #[test]
    fn is_unsure_for_text_without_letters() {
        assert_eq!(detect("1234 !!! ???"), Detection::Unsure);
    }
}
