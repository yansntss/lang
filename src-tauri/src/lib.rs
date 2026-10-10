use tauri::Manager;

use platform::popup_window::{self, show_near_cursor, PopupState};
use platform::region_window::{self, RegionState};
use platform::{capture_config, log_failure, settings_window, shortcut, tray};
use secrets::SecretKind;
use services::update::{TauriUpdateSource, UpdateService};
use settings::{Settings, ShortcutSlot};
use state::AppState;

pub mod commands;
pub mod error;
pub mod fatal;
pub mod platform;
pub mod secrets;
pub mod services;
pub mod settings;
pub mod state;

pub use error::AppError;

pub fn run() -> tauri::Result<()> {
    fatal::install_panic_hook();
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // O front não tem permissão no plugin: só os comandos `check_update` e `install_update`.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(shortcut::plugin())
        .manage(PopupState::default())
        .manage(RegionState::default())
        .setup(setup)
        .on_window_event(|window, event| {
            popup_window::handle_window_event(window, event);
            region_window::handle_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate::translate,
            commands::translate::copy_to_clipboard,
            commands::explain::explain,
            commands::explain::cancel_explain,
            commands::history::record_history,
            commands::history::list_history,
            commands::history::delete_history,
            commands::history::clear_history,
            commands::history::toggle_favorite,
            commands::history::export_history,
            commands::history::get_history_enabled,
            commands::history::set_history_enabled,
            commands::history::next_review,
            commands::history::mark_reviewed,
            commands::window::hide_popup,
            commands::window::fit_popup_height,
            commands::window::set_popup_pinned,
            commands::window::start_popup_dragging,
            commands::window::start_popup_resizing,
            commands::region::get_region_snapshot,
            commands::region::confirm_region,
            commands::region::cancel_region,
            commands::secrets::set_secret,
            commands::secrets::has_secret,
            commands::secrets::delete_secret,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::set_shortcut,
            commands::settings::set_autostart,
            commands::settings::test_secret,
            commands::update::check_update,
            commands::update::install_update,
        ])
        .run(tauri::generate_context!())
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    app.manage(AppState::new(&data_dir)?);
    app.manage(UpdateService::new(TauriUpdateSource::new(
        app.handle().clone(),
    )));

    let settings = app.state::<AppState>().settings.get();
    capture_config::set_allow_editors(settings.preferences.capture_in_editors);
    tray::create(app.handle())?;
    let translate = startup_shortcut(app.handle(), &settings, ShortcutSlot::Translate, None)?;
    let ocr = startup_shortcut(app.handle(), &settings, ShortcutSlot::Ocr, Some(&translate))?;
    capture_config::set_main_key(translate.virtual_key);
    for (slot, accelerator) in [
        (ShortcutSlot::Translate, &translate),
        (ShortcutSlot::Ocr, &ocr),
    ] {
        let registered = shortcut::register_initial(app.handle(), accelerator);
        shortcut::record_status(slot, accelerator, registered);
    }
    tray::update_tooltip(app.handle());

    open_settings_on_first_run(app.handle());
    Ok(())
}

/// O atalho de `slot` que vai valer: o salvo, o de fábrica se o salvo estava inválido, ou outro
/// se ele colidia com `avoid` (o do tradutor). Se mudou, corrige o arquivo, para a janela de
/// configurações mostrar o atalho que realmente vale.
fn startup_shortcut(
    app: &tauri::AppHandle,
    settings: &Settings,
    slot: ShortcutSlot,
    avoid: Option<&platform::accelerator::Accelerator>,
) -> Result<platform::accelerator::Accelerator, AppError> {
    let saved = settings.shortcut_for(slot);
    let parsed = shortcut::startup_accelerator(saved, slot)?;
    let accelerator = match avoid {
        Some(avoid) => shortcut::without_collision(avoid, parsed)?,
        None => parsed,
    };
    if accelerator.label != saved {
        if let Err(error) = app
            .state::<AppState>()
            .settings
            .update_shortcut(slot, &accelerator.label)
        {
            log::warn!("não foi possível corrigir o atalho salvo: {error:?}");
        }
    }
    Ok(accelerator)
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
