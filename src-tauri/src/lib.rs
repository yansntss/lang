use tauri::Manager;

use platform::popup_window::{handle_window_event, show_near_cursor, PopupState};
use platform::{log_failure, settings_window, shortcut, tray};
use secrets::SecretKind;
use state::AppState;

pub mod commands;
pub mod error;
pub mod platform;
pub mod secrets;
pub mod services;
pub mod state;

pub use error::AppError;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        // A instância única precisa ser o primeiro plugin registrado.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log_failure("abrir o tradutor", show_near_cursor(app, None));
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(shortcut::plugin())
        .manage(PopupState::default())
        .setup(setup)
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::translate::translate,
            commands::translate::copy_to_clipboard,
            commands::explain::explain,
            commands::explain::cancel_explain,
            commands::window::hide_popup,
            commands::secrets::set_secret,
            commands::secrets::has_secret,
            commands::secrets::delete_secret,
        ])
        .run(tauri::generate_context!())
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(AppState::new()?);

    let tray = tray::create(app.handle())?;
    if !shortcut::register_default(app.handle()) {
        tray.set_tooltip(Some(format!(
            "{} — atalho {} indisponível",
            tray::TOOLTIP,
            shortcut::DEFAULT_LABEL
        )))?;
    }

    open_settings_on_first_run(app.handle());
    Ok(())
}

/// Sem chave do DeepL o app não traduz nada: leva o usuário direto às configurações.
fn open_settings_on_first_run(app: &tauri::AppHandle) {
    let configured = match app.state::<AppState>().secrets.has(SecretKind::Deepl) {
        Ok(configured) => configured,
        Err(error) => {
            log::warn!("não foi possível verificar a chave do DeepL: {error:?}");
            false
        }
    };

    if !configured {
        log_failure("abrir as configurações", settings_window::open(app));
    }
}
