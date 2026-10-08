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

/// Texto da dica da bandeja: avisa quando o atalho não pôde ser registrado.
pub fn tooltip_for(shortcut_label: &str, shortcut_available: bool) -> String {
    if shortcut_available {
        TOOLTIP.to_owned()
    } else {
        format!("{TOOLTIP} — atalho {shortcut_label} indisponível")
    }
}

/// Atualiza a dica da bandeja depois de trocar o atalho.
pub fn update_tooltip(app: &AppHandle, shortcut_label: &str, shortcut_available: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Err(error) = tray.set_tooltip(Some(tooltip_for(shortcut_label, shortcut_available))) {
        log::warn!("não foi possível atualizar a dica da bandeja: {error}");
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_only_warns_when_the_shortcut_is_unavailable() {
        assert_eq!(tooltip_for("Ctrl+Alt+T", true), "Tradutor");
        assert_eq!(
            tooltip_for("Ctrl+Alt+T", false),
            "Tradutor — atalho Ctrl+Alt+T indisponível"
        );
    }
}
