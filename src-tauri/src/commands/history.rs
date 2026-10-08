use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::services::history::{csv, now_ms, HistoryEntry, ListQuery, NewEntry, SqliteHistory};
use crate::state::AppState;

/// Roda `work` fora da thread da interface: o SQLite faz E/S de disco.
async fn blocking<T, F>(state: &State<'_, AppState>, work: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&SqliteHistory) -> Result<T, AppError> + Send + 'static,
{
    let history = state.history()?;
    tauri::async_runtime::spawn_blocking(move || work(&history))
        .await
        .map_err(|error| AppError::History(error.to_string()))?
}

/// Guarda uma tradução confirmada pelo usuário (Enter ou Explicar). Não grava nada se o
/// histórico estiver desligado.
#[tauri::command]
pub async fn record_history(
    state: State<'_, AppState>,
    source_text: String,
    translated_text: String,
    source_lang: String,
    target_lang: String,
) -> Result<(), AppError> {
    blocking(&state, move |history| {
        history.record(&NewEntry {
            source_text: &source_text,
            translated_text: &translated_text,
            source_lang: &source_lang,
            target_lang: &target_lang,
        })
    })
    .await
}

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
    query: Option<String>,
    favorites_only: bool,
    offset: u32,
) -> Result<Vec<HistoryEntry>, AppError> {
    blocking(&state, move |history| {
        history.list(&ListQuery {
            query,
            favorites_only,
            offset,
        })
    })
    .await
}

#[tauri::command]
pub async fn delete_history(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    blocking(&state, move |history| history.delete(id)).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), AppError> {
    blocking(&state, SqliteHistory::clear).await
}

/// Devolve o novo estado do favorito.
#[tauri::command]
pub async fn toggle_favorite(state: State<'_, AppState>, id: i64) -> Result<bool, AppError> {
    blocking(&state, move |history| history.toggle_favorite(id)).await
}

#[tauri::command]
pub async fn get_history_enabled(state: State<'_, AppState>) -> Result<bool, AppError> {
    blocking(&state, SqliteHistory::enabled).await
}

#[tauri::command]
pub async fn set_history_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), AppError> {
    blocking(&state, move |history| history.set_enabled(enabled)).await
}

#[tauri::command]
pub async fn next_review(state: State<'_, AppState>) -> Result<Option<HistoryEntry>, AppError> {
    blocking(&state, SqliteHistory::next_review).await
}

#[tauri::command]
pub async fn mark_reviewed(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    blocking(&state, move |history| history.mark_reviewed(id)).await
}

/// Grava o histórico inteiro em CSV na pasta Downloads e devolve o caminho do arquivo.
#[tauri::command]
pub async fn export_history(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let directory = app
        .path()
        .download_dir()
        .map_err(|error| AppError::History(error.to_string()))?;

    blocking(&state, move |history| {
        let content = csv::export(&history.all()?);
        let path = directory.join(csv::file_name(now_ms()));
        std::fs::write(&path, content).map_err(|error| AppError::History(error.to_string()))?;
        Ok(path.display().to_string())
    })
    .await
}
