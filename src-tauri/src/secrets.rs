use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Chaves de API que o app guarda. Nunca saem do backend Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    Deepl,
    Anthropic,
}

impl SecretKind {
    /// Nome da entrada no keychain do SO.
    pub fn account(self) -> &'static str {
        match self {
            Self::Deepl => "deepl_api_key",
            Self::Anthropic => "anthropic_api_key",
        }
    }
}

impl fmt::Display for SecretKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deepl => f.write_str("DeepL"),
            Self::Anthropic => f.write_str("Anthropic"),
        }
    }
}

/// Armazenamento de segredos. A implementação real (keychain) vem na Fase 1; os serviços
/// dependem só desta trait, o que permite testá-los com [`InMemoryStore`].
pub trait SecretStore: Send + Sync {
    fn get(&self, kind: SecretKind) -> Result<Option<String>, AppError>;
    fn set(&self, kind: SecretKind, value: &str) -> Result<(), AppError>;
    fn delete(&self, kind: SecretKind) -> Result<(), AppError>;

    fn has(&self, kind: SecretKind) -> Result<bool, AppError> {
        Ok(self.get(kind)?.is_some())
    }

    fn require(&self, kind: SecretKind) -> Result<String, AppError> {
        self.get(kind)?.ok_or(AppError::MissingSecret(kind))
    }
}

/// Implementação em memória, para testes.
#[derive(Default)]
pub struct InMemoryStore {
    entries: Mutex<HashMap<SecretKind, String>>,
}

/// `Debug` manual: um `{:?}` descuidado nunca deve imprimir os segredos guardados.
impl fmt::Debug for InMemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryStore").finish_non_exhaustive()
    }
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn entries(&self) -> Result<MutexGuard<'_, HashMap<SecretKind, String>>, AppError> {
        self.entries
            .lock()
            .map_err(|_| AppError::SecretStore("mutex envenenado".into()))
    }
}

impl SecretStore for InMemoryStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>, AppError> {
        Ok(self.entries()?.get(&kind).cloned())
    }

    fn set(&self, kind: SecretKind, value: &str) -> Result<(), AppError> {
        self.entries()?.insert(kind, value.to_owned());
        Ok(())
    }

    fn delete(&self, kind: SecretKind) -> Result<(), AppError> {
        self.entries()?.remove(&kind);
        Ok(())
    }
}

/// Nome do serviço sob o qual as chaves ficam no keychain do SO.
pub const KEYRING_SERVICE: &str = "com.yansa.lang-app";

/// Guarda os segredos no keychain do sistema (Gerenciador de Credenciais no Windows).
#[derive(Debug, Clone)]
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self::with_service(KEYRING_SERVICE)
    }

    /// Permite usar outro nome de serviço, para testes não tocarem nas chaves reais.
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, kind: SecretKind) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(&self.service, kind.account()).map_err(store_error)
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>, AppError> {
        missing_to_none(self.entry(kind)?.get_password())
    }

    fn set(&self, kind: SecretKind, value: &str) -> Result<(), AppError> {
        self.entry(kind)?.set_password(value).map_err(store_error)
    }

    fn delete(&self, kind: SecretKind) -> Result<(), AppError> {
        missing_to_none(self.entry(kind)?.delete_credential()).map(|_| ())
    }
}

/// "Não existe" não é falha: vira `None`. O detalhe de qualquer outro erro fica só no `Debug`.
fn missing_to_none<T>(result: Result<T, keyring::Error>) -> Result<Option<T>, AppError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(store_error(error)),
    }
}

fn store_error(error: keyring::Error) -> AppError {
    AppError::SecretStore(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_secret_was_never_set() {
        let store = InMemoryStore::new();

        assert_eq!(store.get(SecretKind::Deepl).unwrap(), None);
        assert!(!store.has(SecretKind::Deepl).unwrap());
    }

    #[test]
    fn stores_and_overwrites_a_secret() {
        let store = InMemoryStore::new();

        store.set(SecretKind::Deepl, "primeira").unwrap();
        store.set(SecretKind::Deepl, "segunda").unwrap();

        assert_eq!(
            store.get(SecretKind::Deepl).unwrap().as_deref(),
            Some("segunda")
        );
        assert!(store.has(SecretKind::Deepl).unwrap());
    }

    #[test]
    fn keeps_kinds_independent() {
        let store = InMemoryStore::new();

        store.set(SecretKind::Deepl, "chave-deepl").unwrap();

        assert!(!store.has(SecretKind::Anthropic).unwrap());
    }

    #[test]
    fn deletes_a_secret_and_tolerates_deleting_a_missing_one() {
        let store = InMemoryStore::new();
        store.set(SecretKind::Anthropic, "chave").unwrap();

        store.delete(SecretKind::Anthropic).unwrap();
        store.delete(SecretKind::Anthropic).unwrap();

        assert!(!store.has(SecretKind::Anthropic).unwrap());
    }

    #[test]
    fn require_returns_missing_secret_error_when_absent() {
        let store = InMemoryStore::new();

        let error = store.require(SecretKind::Deepl).unwrap_err();

        assert_eq!(error.code(), "missing_secret");
    }

    #[test]
    fn require_returns_the_value_when_present() {
        let store = InMemoryStore::new();
        store.set(SecretKind::Deepl, "abc:fx").unwrap();

        assert_eq!(store.require(SecretKind::Deepl).unwrap(), "abc:fx");
    }

    #[test]
    fn kinds_deserialize_from_snake_case() {
        let kind: SecretKind = serde_json::from_str(r#""anthropic""#).unwrap();

        assert_eq!(kind, SecretKind::Anthropic);
    }

    #[test]
    fn in_memory_store_debug_output_never_shows_the_secrets() {
        let store = InMemoryStore::new();
        store
            .set(SecretKind::Deepl, "segredo-que-nao-pode-vazar")
            .unwrap();

        assert!(!format!("{store:?}").contains("segredo-que-nao-pode-vazar"));
    }

    #[test]
    fn missing_keyring_entry_becomes_none() {
        let result = missing_to_none::<String>(Err(keyring::Error::NoEntry));

        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn existing_keyring_value_is_wrapped_in_some() {
        let result = missing_to_none(Ok("valor".to_owned()));

        assert_eq!(result.unwrap().as_deref(), Some("valor"));
    }

    #[test]
    fn other_keyring_errors_become_generic_secret_store_errors() {
        let result = missing_to_none::<String>(Err(keyring::Error::Invalid(
            "service".into(),
            "detalhe interno".into(),
        )));

        let error = result.unwrap_err();

        assert_eq!(error.code(), "secret_store");
        assert!(!error.to_string().contains("detalhe interno"));
        assert!(format!("{error:?}").contains("detalhe interno"));
    }

    /// Toca no Gerenciador de Credenciais do Windows, sob um nome de serviço de teste que
    /// é removido ao fim. Rodar com `cargo test -- --ignored`.
    #[test]
    #[ignore = "usa o keychain real do sistema operacional"]
    fn keyring_store_round_trips_against_the_real_credential_store() {
        let store = KeyringStore::with_service("com.yansa.lang-app.test");
        store.delete(SecretKind::Deepl).unwrap();

        assert!(!store.has(SecretKind::Deepl).unwrap());

        store.set(SecretKind::Deepl, "primeira:fx").unwrap();
        store.set(SecretKind::Deepl, "segunda:fx").unwrap();
        assert_eq!(
            store.get(SecretKind::Deepl).unwrap().as_deref(),
            Some("segunda:fx")
        );

        store.delete(SecretKind::Deepl).unwrap();
        assert!(!store.has(SecretKind::Deepl).unwrap());
        store.delete(SecretKind::Deepl).unwrap();
    }
}
