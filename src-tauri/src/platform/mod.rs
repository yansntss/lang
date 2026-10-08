use crate::error::AppError;

pub mod placement;
pub mod popup_window;
pub mod settings_window;
pub mod shortcut;
pub mod tray;
#[cfg(windows)]
pub mod windows_capture;

/// Registra no log a falha de uma ação disparada por evento do sistema (atalho, bandeja),
/// onde não há ninguém para receber o erro. O `Debug` não contém segredos.
pub(crate) fn log_failure(action: &str, result: Result<(), AppError>) {
    if let Err(error) = result {
        log::error!("falha ao {action}: {error:?}");
    }
}

pub(crate) fn window_error(error: tauri::Error) -> AppError {
    AppError::Window(error.to_string())
}
