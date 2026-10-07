use tauri::WebviewWindow;

use crate::error::AppError;

/// Oculta a janela que fez a chamada. O popup nunca é destruído, só escondido.
#[tauri::command]
pub fn hide_popup(window: WebviewWindow) -> Result<(), AppError> {
    window
        .hide()
        .map_err(|error| AppError::Window(error.to_string()))
}
