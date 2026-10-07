use std::sync::Arc;

use crate::error::AppError;
use crate::secrets::{KeyringStore, SecretStore};
use crate::services::deepl::DeepLClient;
use crate::services::translation::TranslationService;

/// Estado compartilhado entre os comandos Tauri.
pub struct AppState {
    pub translations: TranslationService<DeepLClient>,
    pub secrets: Arc<dyn SecretStore>,
}

impl AppState {
    pub fn new() -> Result<Self, AppError> {
        let secrets: Arc<dyn SecretStore> = Arc::new(KeyringStore::new());
        let deepl = DeepLClient::new(Arc::clone(&secrets))?;

        Ok(Self {
            translations: TranslationService::new(deepl),
            secrets,
        })
    }
}
