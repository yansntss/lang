use serde::ser::{Serialize, SerializeStruct, Serializer};
use thiserror::Error;

use crate::secrets::SecretKind;

/// Erro único do backend. Os comandos Tauri o devolvem ao front serializado como
/// `{ code, message }`.
///
/// `Display` (e portanto `message`) é sempre uma frase segura para o usuário. Detalhes
/// técnicos ficam apenas no `Debug`, para log, e nunca chegam ao front.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Chave de API do {0} não configurada.")]
    MissingSecret(SecretKind),

    #[error("Falha ao acessar o armazenamento seguro de chaves.")]
    SecretStore(String),

    /// A mensagem é escrita pelo próprio backend e é segura para exibir.
    #[error("{0}")]
    InvalidInput(String),

    /// O sistema recusou registrar o atalho (em geral, outro app já o usa).
    #[error("Esse atalho já está em uso por outro app. Escolha outro.")]
    ShortcutInUse,

    #[error("A chave de API do {0} é inválida ou foi recusada.")]
    InvalidApiKey(SecretKind),

    #[error("A cota mensal do DeepL foi esgotada.")]
    QuotaExceeded,

    #[error("Muitas requisições. Aguarde alguns segundos e tente de novo.")]
    RateLimited,

    #[error("Não foi possível conectar ao serviço. Verifique sua conexão.")]
    Network(String),

    #[error("O serviço respondeu com erro ({0}).")]
    Upstream(u16),

    #[error("Não foi possível copiar para a área de transferência.")]
    Clipboard(String),

    #[error("Não foi possível controlar a janela.")]
    Window(String),

    #[error("Não foi possível capturar o texto selecionado.")]
    Capture(String),

    #[error("Não foi possível capturar a tela.")]
    ScreenCapture(String),

    /// A captura veio toda preta: em geral, um jogo em tela cheia exclusiva.
    #[error(
        "A captura da tela veio preta. Em jogos, use o modo janela sem bordas (borderless) em \
         vez da tela cheia exclusiva."
    )]
    BlankCapture,

    #[error("Selecione uma área maior, que contenha o texto.")]
    RegionEmpty,

    #[error(
        "O Windows não tem nenhum idioma de reconhecimento de texto (OCR). Adicione o Inglês \
         (Estados Unidos) em Configurações > Hora e idioma > Idioma e região."
    )]
    OcrLanguageMissing,

    #[error("Não foi possível ler o texto da imagem.")]
    Ocr(String),

    #[error("Nenhum texto encontrado na área selecionada.")]
    NoTextFound,

    #[error("Não foi possível acessar o histórico.")]
    History(String),

    #[error("Não foi possível salvar as configurações.")]
    Settings(String),

    #[error("Não foi possível concluir a atualização.")]
    Update(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingSecret(_) => "missing_secret",
            Self::SecretStore(_) => "secret_store",
            Self::InvalidInput(_) => "invalid_input",
            Self::ShortcutInUse => "shortcut_in_use",
            Self::InvalidApiKey(_) => "invalid_api_key",
            Self::QuotaExceeded => "quota_exceeded",
            Self::RateLimited => "rate_limited",
            Self::Network(_) => "network",
            Self::Upstream(_) => "upstream",
            Self::Clipboard(_) => "clipboard",
            Self::Window(_) => "window",
            Self::Capture(_) => "capture",
            Self::ScreenCapture(_) => "screen_capture",
            Self::BlankCapture => "blank_capture",
            Self::RegionEmpty => "region_empty",
            Self::OcrLanguageMissing => "ocr_language_missing",
            Self::Ocr(_) => "ocr",
            Self::NoTextFound => "no_text_found",
            Self::History(_) => "history",
            Self::Settings(_) => "settings",
            Self::Update(_) => "update",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_user_message() {
        let error = AppError::MissingSecret(SecretKind::Deepl);

        let json = serde_json::to_string(&error).unwrap();

        assert_eq!(
            json,
            r#"{"code":"missing_secret","message":"Chave de API do DeepL não configurada."}"#
        );
    }

    #[test]
    fn serialization_does_not_leak_internal_detail() {
        let error = AppError::SecretStore("senha-super-secreta".into());

        let json = serde_json::to_string(&error).unwrap();

        assert!(json.contains("secret_store"));
        assert!(!json.contains("senha-super-secreta"));
    }

    #[test]
    fn debug_keeps_detail_for_logs() {
        let error = AppError::SecretStore("lock envenenado".into());

        assert!(format!("{error:?}").contains("lock envenenado"));
    }

    #[test]
    fn serializes_translation_errors_with_user_messages() {
        let cases = [
            (
                AppError::InvalidInput("O texto está vazio.".into()),
                "invalid_input",
                "O texto está vazio.",
            ),
            (
                AppError::ShortcutInUse,
                "shortcut_in_use",
                "Esse atalho já está em uso por outro app. Escolha outro.",
            ),
            (
                AppError::InvalidApiKey(SecretKind::Deepl),
                "invalid_api_key",
                "A chave de API do DeepL é inválida ou foi recusada.",
            ),
            (
                AppError::QuotaExceeded,
                "quota_exceeded",
                "A cota mensal do DeepL foi esgotada.",
            ),
            (
                AppError::RateLimited,
                "rate_limited",
                "Muitas requisições. Aguarde alguns segundos e tente de novo.",
            ),
            (
                AppError::Upstream(500),
                "upstream",
                "O serviço respondeu com erro (500).",
            ),
            (
                AppError::RegionEmpty,
                "region_empty",
                "Selecione uma área maior, que contenha o texto.",
            ),
        ];

        for (error, code, message) in cases {
            let value = serde_json::to_value(&error).unwrap();

            assert_eq!(value["code"], code);
            assert_eq!(value["message"], message);
        }
    }

    #[test]
    fn serializes_system_errors_with_generic_messages_only() {
        let cases = [
            (
                AppError::Clipboard("OpenClipboard falhou: 0x5".into()),
                "clipboard",
                "Não foi possível copiar para a área de transferência.",
            ),
            (
                AppError::Window("HWND inválido".into()),
                "window",
                "Não foi possível controlar a janela.",
            ),
            (
                AppError::Capture("SendInput: acesso negado".into()),
                "capture",
                "Não foi possível capturar o texto selecionado.",
            ),
            (
                AppError::Ocr("RecognizeAsync: 0x80004005".into()),
                "ocr",
                "Não foi possível ler o texto da imagem.",
            ),
            (
                AppError::ScreenCapture("BitBlt falhou: HWND 0x5".into()),
                "screen_capture",
                "Não foi possível capturar a tela.",
            ),
            (
                AppError::History("no such table: history".into()),
                "history",
                "Não foi possível acessar o histórico.",
            ),
            (
                AppError::Settings("Permission denied (os error 5)".into()),
                "settings",
                "Não foi possível salvar as configurações.",
            ),
            (
                AppError::Update("signature verification failed: key id 4F2A".into()),
                "update",
                "Não foi possível concluir a atualização.",
            ),
        ];

        for (error, code, message) in cases {
            let json = serde_json::to_string(&error).unwrap();

            assert!(json.contains(&format!(r#""code":"{code}""#)));
            assert!(json.contains(message));
            assert!(
                !json.contains("0x5")
                    && !json.contains("HWND")
                    && !json.contains("table")
                    && !json.contains("Permission denied")
            );
        }
    }

    #[test]
    fn network_error_hides_internal_detail_from_the_front() {
        let error = AppError::Network("https://api-free.deepl.com/v2/translate: refused".into());

        let json = serde_json::to_string(&error).unwrap();

        assert!(json.contains(r#""code":"network""#));
        assert!(!json.contains("deepl.com"));
        assert!(format!("{error:?}").contains("deepl.com"));
    }
}
