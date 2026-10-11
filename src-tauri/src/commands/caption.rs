use tauri::AppHandle;

use crate::error::AppError;
use crate::platform::caption_session;

/// Liga a legenda ao vivo sobre a última região lida pelo OCR.
#[tauri::command]
pub fn start_caption(app: AppHandle) -> Result<(), AppError> {
    caption_session::start(&app)
}

#[tauri::command]
pub fn stop_caption(app: AppHandle) {
    caption_session::stop(&app);
}
