use tauri::{Manager, State, WebviewWindow};

use crate::error::AppError;
use crate::platform::popup_drag::{start_moving, start_resizing, ResizeEdge};
use crate::platform::popup_window::{fit_to_content, prepare_to_hide, set_pinned};
use crate::state::AppState;

/// Oculta a janela que fez a chamada. O popup nunca é destruído, só escondido.
/// Uma explicação em andamento é cancelada: ninguém a veria, e ela gastaria cota.
#[tauri::command]
pub fn hide_popup(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), AppError> {
    state.active_explain.cancel_current();
    prepare_to_hide(window.app_handle());
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

/// Fixado, o popup não some ao perder o foco. Ocultá-lo (Esc) o solta de novo.
#[tauri::command]
pub fn set_popup_pinned(window: WebviewWindow, pinned: bool) {
    set_pinned(window.app_handle(), pinned);
}

/// Chamado no mousedown (duplo clique segurado): o sistema move a janela até soltar o botão.
#[tauri::command]
pub fn start_popup_dragging(window: WebviewWindow) -> Result<(), AppError> {
    start_moving(&window)
}

/// Chamado no mousedown de uma alça de borda: o sistema redimensiona até soltar o botão.
#[tauri::command]
pub fn start_popup_resizing(window: WebviewWindow, edge: ResizeEdge) -> Result<(), AppError> {
    start_resizing(&window, edge)
}
