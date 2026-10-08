use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use super::popup_window::show_near_cursor;
use super::{capture_foreground_selection, log_failure};
use crate::error::AppError;
use crate::services::capture::CaptureOutcome;

/// Texto do atalho padrão, para mensagens ao usuário.
pub const DEFAULT_LABEL: &str = "Ctrl+Alt+T";

pub fn default_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyT)
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                open_with_selection(app.clone());
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

/// Converte o resultado da captura no texto a preencher no popup. Falhas e capturas ignoradas
/// só vão para o log (sem o texto do usuário) e o popup abre vazio.
fn prefill_from(outcome: Result<CaptureOutcome, AppError>) -> Option<String> {
    match outcome {
        Ok(CaptureOutcome::Captured {
            text,
            restore_failed,
        }) => {
            if restore_failed {
                log::warn!("o clipboard anterior não pôde ser restaurado após a captura");
            }
            Some(text)
        }
        Ok(CaptureOutcome::NoSelection) => None,
        Ok(CaptureOutcome::Skipped(reason)) => {
            log::info!("captura da seleção ignorada: {reason:?}");
            None
        }
        Err(error) => {
            log::warn!("falha ao capturar a seleção: {error:?}");
            None
        }
    }
}

/// Segurar o atalho repete `Pressed`; enquanto uma captura roda, os demais são ignorados.
static CAPTURE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Libera a trava ao sair do escopo, inclusive se a thread de captura entrar em pânico.
struct InFlight;

impl Drop for InFlight {
    fn drop(&mut self) {
        CAPTURE_IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

/// Captura a seleção do app em foco e abre o popup com ela. A captura espera as teclas do
/// atalho serem soltas e consulta o clipboard, então roda fora da thread de eventos e antes
/// de o popup roubar o foco.
fn open_with_selection(app: AppHandle) {
    if CAPTURE_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return;
    }

    let worker_app = app.clone();
    let spawned = thread::Builder::new()
        .name("selection-capture".into())
        .spawn(move || {
            let _in_flight = InFlight;
            let prefill = prefill_from(capture_foreground_selection());
            log_failure(
                "abrir o tradutor",
                show_near_cursor(&worker_app, prefill.as_deref()),
            );
        });

    if let Err(error) = spawned {
        CAPTURE_IN_FLIGHT.store(false, Ordering::SeqCst);
        log::error!("não foi possível iniciar a captura da seleção: {error}");
        log_failure("abrir o tradutor", show_near_cursor(&app, None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::capture::SkipReason;

    #[test]
    fn prefills_with_the_captured_text() {
        let outcome = CaptureOutcome::Captured {
            text: "hello world".into(),
            restore_failed: false,
        };

        assert_eq!(prefill_from(Ok(outcome)).as_deref(), Some("hello world"));
    }

    #[test]
    fn still_prefills_when_only_the_clipboard_restore_failed() {
        let outcome = CaptureOutcome::Captured {
            text: "hello".into(),
            restore_failed: true,
        };

        assert_eq!(prefill_from(Ok(outcome)).as_deref(), Some("hello"));
    }

    #[test]
    fn opens_empty_when_nothing_was_selected() {
        assert_eq!(prefill_from(Ok(CaptureOutcome::NoSelection)), None);
    }

    #[test]
    fn opens_empty_for_every_skip_reason() {
        for reason in [
            SkipReason::ModifiersHeld,
            SkipReason::UnsupportedClipboard,
            SkipReason::BlockedApplication,
        ] {
            assert_eq!(prefill_from(Ok(CaptureOutcome::Skipped(reason))), None);
        }
    }

    #[test]
    fn opens_empty_when_the_capture_fails() {
        let error = AppError::Capture("SendInput falhou".into());

        assert_eq!(prefill_from(Err(error)), None);
    }

    #[test]
    fn default_shortcut_matches_its_label() {
        let parsed: Shortcut = DEFAULT_LABEL.parse().unwrap();

        assert_eq!(default_shortcut(), parsed);
    }
}
