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
#[derive(Debug, Default)]
pub struct InMemoryStore {
    entries: Mutex<HashMap<SecretKind, String>>,
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
}
