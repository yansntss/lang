use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use super::log_failure;
use super::popup_window::show_near_cursor;

/// Texto do atalho padrão, para mensagens ao usuário.
pub const DEFAULT_LABEL: &str = "Ctrl+Alt+T";

pub fn default_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyT)
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                log_failure("abrir o tradutor", show_near_cursor(app));
            }
        })
        .build()
}

/// Devolve `false` se o atalho não pôde ser registrado (por exemplo, já em uso por outro app).
pub fn register_default(app: &AppHandle) -> bool {
    match app.global_shortcut().register(default_shortcut()) {
        Ok(()) => true,
        Err(error) => {
            log::error!("não foi possível registrar o atalho {DEFAULT_LABEL}: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_matches_its_label() {
        let parsed: Shortcut = DEFAULT_LABEL.parse().unwrap();

        assert_eq!(default_shortcut(), parsed);
    }
}
