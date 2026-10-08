use std::path::Path;
use std::sync::Arc;

use crate::error::AppError;
use crate::secrets::{KeyringStore, SecretStore};
use crate::services::active_explain::ActiveExplain;
use crate::services::claude::ClaudeClient;
use crate::services::deepl::DeepLClient;
use crate::services::explain::ExplainService;
use crate::services::history::SqliteHistory;
use crate::services::translation::TranslationService;

/// Nome do arquivo do histórico dentro da pasta de dados do app.
const HISTORY_FILE: &str = "history.db";

/// Estado compartilhado entre os comandos Tauri.
pub struct AppState {
    pub translations: TranslationService<DeepLClient>,
    /// `Arc` porque a explicação roda numa tarefa própria, que sobrevive ao comando.
    pub explanations: Arc<ExplainService<ClaudeClient>>,
    pub active_explain: Arc<ActiveExplain>,
    /// `None` se o banco não abriu: o app continua traduzindo, sem histórico.
    history: Option<Arc<SqliteHistory>>,
    pub secrets: Arc<dyn SecretStore>,
}

impl AppState {
    pub fn new(data_dir: &Path) -> Result<Self, AppError> {
        let secrets: Arc<dyn SecretStore> = Arc::new(KeyringStore::new());
        let deepl = DeepLClient::new(Arc::clone(&secrets))?;
        let claude = ClaudeClient::new(Arc::clone(&secrets))?;
        let history = match SqliteHistory::open(&data_dir.join(HISTORY_FILE)) {
            Ok(history) => Some(Arc::new(history)),
            Err(error) => {
                log::error!("histórico indisponível: {error:?}");
                None
            }
        };

        Ok(Self {
            translations: TranslationService::new(deepl),
            explanations: Arc::new(ExplainService::new(claude)),
            active_explain: Arc::new(ActiveExplain::default()),
            history,
            secrets,
        })
    }

    pub fn history(&self) -> Result<Arc<SqliteHistory>, AppError> {
        self.history
            .clone()
            .ok_or_else(|| AppError::History("banco indisponível".into()))
    }
}
