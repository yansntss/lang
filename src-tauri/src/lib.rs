pub mod error;
pub mod secrets;

pub use error::AppError;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default().run(tauri::generate_context!())
}
