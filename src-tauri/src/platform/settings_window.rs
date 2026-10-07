use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::window_error;
use crate::error::AppError;

pub const SETTINGS_LABEL: &str = "settings";

/// Abre a janela de configurações, ou só a traz para frente se já estiver aberta.
pub fn open(app: &AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        window.show().map_err(window_error)?;
        return window.set_focus().map_err(window_error);
    }

    WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
        .title("Configurações")
        .inner_size(480.0, 300.0)
        .resizable(false)
        .center()
        .build()
        .map_err(window_error)?;
    Ok(())
}
