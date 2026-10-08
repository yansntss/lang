use tauri::{State, WebviewWindow};

use crate::error::AppError;
use crate::state::AppState;

/// Oculta a janela que fez a chamada. O popup nunca é destruído, só escondido.
/// Uma explicação em andamento é cancelada: ninguém a veria, e ela gastaria cota.
#[tauri::command]
pub fn hide_popup(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), AppError> {
    state.active_explain.cancel_current();
    window
        .hide()
        .map_err(|error| AppError::Window(error.to_string()))
}
