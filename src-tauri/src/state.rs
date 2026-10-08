use std::sync::Arc;

use crate::error::AppError;
use crate::secrets::{KeyringStore, SecretStore};
use crate::services::active_explain::ActiveExplain;
use crate::services::claude::ClaudeClient;
use crate::services::deepl::DeepLClient;
use crate::services::explain::ExplainService;
use crate::services::translation::TranslationService;

/// Estado compartilhado entre os comandos Tauri.
pub struct AppState {
    pub translations: TranslationService<DeepLClient>,
    /// `Arc` porque a explicação roda numa tarefa própria, que sobrevive ao comando.
    pub explanations: Arc<ExplainService<ClaudeClient>>,
    pub active_explain: Arc<ActiveExplain>,
    pub secrets: Arc<dyn SecretStore>,
}

impl AppState {
    pub fn new() -> Result<Self, AppError> {
        let secrets: Arc<dyn SecretStore> = Arc::new(KeyringStore::new());
        let deepl = DeepLClient::new(Arc::clone(&secrets))?;
        let claude = ClaudeClient::new(Arc::clone(&secrets))?;

        Ok(Self {
            translations: TranslationService::new(deepl),
            explanations: Arc::new(ExplainService::new(claude)),
            active_explain: Arc::new(ActiveExplain::default()),
            secrets,
        })
    }
}
