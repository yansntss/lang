use crate::error::AppError;
#[cfg(not(windows))]
use crate::services::capture::CaptureOutcome;

pub mod history_window;
pub mod placement;
pub mod popup_window;
pub mod settings_window;
pub mod shortcut;
pub mod tray;
#[cfg(windows)]
pub mod windows_capture;

#[cfg(windows)]
pub(crate) use windows_capture::capture_foreground_selection;

/// A captura de seleção só existe no Windows; nos demais sistemas o popup abre vazio.
#[cfg(not(windows))]
pub(crate) fn capture_foreground_selection() -> Result<CaptureOutcome, AppError> {
    Ok(CaptureOutcome::NoSelection)
}

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
