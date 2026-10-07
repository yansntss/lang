use tauri::Manager;

pub mod commands;
pub mod error;
pub mod platform;
pub mod secrets;
pub mod services;
pub mod state;

pub use error::AppError;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            app.manage(state::AppState::new()?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate::translate,
            commands::translate::copy_to_clipboard,
            commands::window::hide_popup,
            commands::secrets::set_secret,
            commands::secrets::has_secret,
            commands::secrets::delete_secret,
        ])
        .run(tauri::generate_context!())
}
