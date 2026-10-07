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
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingSecret(_) => "missing_secret",
            Self::SecretStore(_) => "secret_store",
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
}
