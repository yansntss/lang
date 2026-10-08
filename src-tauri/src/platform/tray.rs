use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use super::history_window;
use super::log_failure;
use super::popup_window::show_near_cursor;
use super::settings_window;

const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_HISTORY: &str = "history";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

pub const TOOLTIP: &str = "Tradutor";

pub fn create(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Abrir tradutor", true, None::<&str>)?;
    let history = MenuItem::with_id(app, MENU_HISTORY, "Histórico", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, MENU_SETTINGS, "Configurações", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &history, &settings, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => log_failure("abrir o tradutor", show_near_cursor(app, None)),
            MENU_HISTORY => log_failure("abrir o histórico", history_window::open(app)),
            MENU_SETTINGS => log_failure("abrir as configurações", settings_window::open(app)),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                log_failure(
                    "abrir o tradutor",
                    show_near_cursor(tray.app_handle(), None),
                );
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)
}
