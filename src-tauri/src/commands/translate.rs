use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::error::AppError;
use crate::services::translation::Translation;
use crate::state::AppState;

#[tauri::command]
pub async fn translate(state: State<'_, AppState>, text: String) -> Result<Translation, AppError> {
    state.translations.translate(&text).await
}

/// A escrita no clipboard acontece no Rust, então o front não precisa de permissão de clipboard.
#[tauri::command]
pub fn copy_to_clipboard(app: AppHandle, text: String) -> Result<(), AppError> {
    app.clipboard()
        .write_text(text)
        .map_err(|error| AppError::Clipboard(error.to_string()))
}
