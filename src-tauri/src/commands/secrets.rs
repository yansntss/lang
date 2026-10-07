use tauri::State;

use crate::error::AppError;
use crate::secrets::{SecretKind, SecretStore};
use crate::state::AppState;

/// Limite generoso: chaves reais têm dezenas de caracteres.
const MAX_SECRET_CHARS: usize = 512;

/// Grava a chave. Não existe comando para lê-la: ela nunca volta ao front.
#[tauri::command]
pub fn set_secret(
    state: State<'_, AppState>,
    kind: SecretKind,
    value: String,
) -> Result<(), AppError> {
    store_secret(state.secrets.as_ref(), kind, &value)
}

#[tauri::command]
pub fn has_secret(state: State<'_, AppState>, kind: SecretKind) -> Result<bool, AppError> {
    state.secrets.has(kind)
}

#[tauri::command]
pub fn delete_secret(state: State<'_, AppState>, kind: SecretKind) -> Result<(), AppError> {
    state.secrets.delete(kind)
}

/// Valida e grava. As mensagens de erro são fixas e nunca incluem o valor recebido.
fn store_secret(store: &dyn SecretStore, kind: SecretKind, value: &str) -> Result<(), AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::InvalidInput("Informe a chave de API.".into()));
    }
    if value.chars().count() > MAX_SECRET_CHARS {
        return Err(AppError::InvalidInput(
            "A chave de API informada é longa demais.".into(),
        ));
    }
    store.set(kind, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::InMemoryStore;

    #[test]
    fn stores_the_key_without_surrounding_whitespace() {
        let store = InMemoryStore::new();

        store_secret(&store, SecretKind::Deepl, "  abc-123:fx \n").unwrap();

        assert_eq!(store.require(SecretKind::Deepl).unwrap(), "abc-123:fx");
    }

    #[test]
    fn rejects_a_blank_key_and_keeps_the_previous_one() {
        let store = InMemoryStore::new();
        store.set(SecretKind::Deepl, "antiga:fx").unwrap();

        let error = store_secret(&store, SecretKind::Deepl, "   ").unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert_eq!(store.require(SecretKind::Deepl).unwrap(), "antiga:fx");
    }

    #[test]
    fn rejects_an_absurdly_long_key() {
        let store = InMemoryStore::new();
        let too_long = "k".repeat(MAX_SECRET_CHARS + 1);

        let error = store_secret(&store, SecretKind::Anthropic, &too_long).unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert!(!store.has(SecretKind::Anthropic).unwrap());
    }

    #[test]
    fn accepts_a_key_exactly_at_the_limit() {
        let store = InMemoryStore::new();
        let at_limit = "k".repeat(MAX_SECRET_CHARS);

        store_secret(&store, SecretKind::Anthropic, &at_limit).unwrap();

        assert!(store.has(SecretKind::Anthropic).unwrap());
    }

    #[test]
    fn validation_messages_never_echo_the_key() {
        let store = InMemoryStore::new();
        let too_long = format!("segredo-{}", "k".repeat(MAX_SECRET_CHARS));

        let error = store_secret(&store, SecretKind::Deepl, &too_long).unwrap_err();

        assert!(!error.to_string().contains("segredo-"));
    }
}
