#[cfg(target_os = "macos")]
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use super::history_window;
use super::log_failure;
use super::popup_window::show_near_cursor;
use super::settings_window;
use super::shortcut;

const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_HISTORY: &str = "history";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

pub const TOOLTIP: &str = "Lang";

/// Texto da dica da bandeja: avisa quais atalhos não puderam ser registrados.
pub fn tooltip_for(unavailable: &[String]) -> String {
    match unavailable {
        [] => TOOLTIP.to_owned(),
        [only] => format!("{TOOLTIP} — atalho {only} indisponível"),
        many => format!("{TOOLTIP} — atalhos {} indisponíveis", many.join(" e ")),
    }
}

/// Atualiza a dica da bandeja com os atalhos que estão sem registro no momento.
pub fn update_tooltip(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let tooltip = tooltip_for(&shortcut::unavailable_labels());
    if let Err(error) = tray.set_tooltip(Some(tooltip)) {
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

    builder = with_icon(app, builder)?;
    builder.build(app)
}

/// No macOS a bandeja usa a versão mono como template: o sistema a pinta conforme o tema.
#[cfg(target_os = "macos")]
fn with_icon(
    _app: &AppHandle,
    builder: TrayIconBuilder<Wry>,
) -> tauri::Result<TrayIconBuilder<Wry>> {
    let icon = Image::from_bytes(include_bytes!("../../icons/tray/tray-template@2x.png"))?;
    Ok(builder.icon(icon).icon_as_template(true))
}

/// No Windows e no Linux a bandeja usa o ícone colorido do app.
#[cfg(not(target_os = "macos"))]
fn with_icon(
    app: &AppHandle,
    builder: TrayIconBuilder<Wry>,
) -> tauri::Result<TrayIconBuilder<Wry>> {
    Ok(match app.default_window_icon() {
        Some(icon) => builder.icon(icon.clone()),
        None => builder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_only_warns_about_shortcuts_that_are_unavailable() {
        assert_eq!(tooltip_for(&[]), "Lang");
        assert_eq!(
            tooltip_for(&["Ctrl+Alt+T".into()]),
            "Lang — atalho Ctrl+Alt+T indisponível"
        );
        assert_eq!(
            tooltip_for(&["Ctrl+Alt+T".into(), "Ctrl+Alt+Y".into()]),
            "Lang — atalhos Ctrl+Alt+T e Ctrl+Alt+Y indisponíveis"
        );
    }

    #[test]
    fn the_macos_tray_templates_are_pngs_in_both_densities() {
        let normal =
            tauri::image::Image::from_bytes(include_bytes!("../../icons/tray/tray-template.png"))
                .unwrap();
        let retina = tauri::image::Image::from_bytes(include_bytes!(
            "../../icons/tray/tray-template@2x.png"
        ))
        .unwrap();

        assert_eq!((normal.width(), normal.height()), (22, 22));
        assert_eq!((retina.width(), retina.height()), (44, 44));
    }
}
