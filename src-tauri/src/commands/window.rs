use tauri::{Manager, State, WebviewWindow};

use crate::error::AppError;
use crate::platform::popup_window::{fit_to_content, save_size_if_changed};
use crate::state::AppState;

/// Oculta a janela que fez a chamada. O popup nunca é destruído, só escondido.
/// Uma explicação em andamento é cancelada: ninguém a veria, e ela gastaria cota.
#[tauri::command]
pub fn hide_popup(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), AppError> {
    state.active_explain.cancel_current();
    save_size_if_changed(window.app_handle());
    window
        .hide()
        .map_err(|error| AppError::Window(error.to_string()))
}

/// O popup informa a altura (em pixels lógicos) que o conteúdo pede, e a janela cresce para
/// caber sem passar do monitor, ou volta ao tamanho escolhido quando o conteúdo diminui.
#[tauri::command]
pub fn fit_popup_height(window: WebviewWindow, content_height: f64) -> Result<(), AppError> {
    fit_to_content(&window, content_height)
}
