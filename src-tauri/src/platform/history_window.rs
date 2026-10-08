use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::window_error;
use crate::error::AppError;

pub const HISTORY_LABEL: &str = "history";

/// Abre a janela de histórico, ou só a traz para frente se já estiver aberta.
pub fn open(app: &AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window(HISTORY_LABEL) {
        window.show().map_err(window_error)?;
        return window.set_focus().map_err(window_error);
    }

    WebviewWindowBuilder::new(app, HISTORY_LABEL, WebviewUrl::App("history.html".into()))
        .title("Histórico")
        .inner_size(560.0, 680.0)
        .min_inner_size(420.0, 400.0)
        .center()
        .build()
        .map_err(window_error)?;
    Ok(())
}
