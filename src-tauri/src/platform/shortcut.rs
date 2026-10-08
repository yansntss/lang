use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

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

/// Uma captura que passa disso está travada (por exemplo, o dono do clipboard não responde a um
/// pedido de renderização). Daí em diante o atalho abre o popup mesmo assim, em vez de ficar
/// morto até reiniciar o app.
const CAPTURE_WATCHDOG: Duration = Duration::from_secs(5);

fn capture_is_stale(started: Instant, now: Instant) -> bool {
    now.saturating_duration_since(started) >= CAPTURE_WATCHDOG
}

/// Segurar o atalho repete `Pressed`; enquanto uma captura roda, os demais são ignorados.
static CAPTURE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
/// Quando a captura em andamento começou, para o relógio de segurança.
static CAPTURE_STARTED_AT: Mutex<Option<Instant>> = Mutex::new(None);

fn set_capture_started(started: Option<Instant>) {
    // Com o lock envenenado o relógio simplesmente deixa de valer; a captura segue normal.
    if let Ok(mut slot) = CAPTURE_STARTED_AT.lock() {
        *slot = started;
    }
}

fn capture_started() -> Option<Instant> {
    CAPTURE_STARTED_AT.lock().ok().and_then(|slot| *slot)
}

/// Libera a trava ao sair do escopo, inclusive se a thread de captura entrar em pânico.
struct InFlight;

impl Drop for InFlight {
    fn drop(&mut self) {
        set_capture_started(None);
        CAPTURE_IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

/// Captura a seleção do app em foco e abre o popup com ela. A captura espera as teclas do
/// atalho serem soltas e consulta o clipboard, então roda fora da thread de eventos e antes
/// de o popup roubar o foco.
fn open_with_selection(app: AppHandle) {
    if CAPTURE_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        // Capturas curtas só repetem o `Pressed`; uma que passou do prazo está travada, e o
        // atalho não pode ficar morto por causa dela.
        if capture_started().is_some_and(|started| capture_is_stale(started, Instant::now())) {
            log::warn!("captura da seleção travada; abrindo o popup sem ela");
            log_failure("abrir o tradutor", show_near_cursor(&app, None));
        }
        return;
    }
    set_capture_started(Some(Instant::now()));

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
        set_capture_started(None);
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
    fn a_capture_is_not_stale_before_the_watchdog_expires() {
        let started = Instant::now();

        assert!(!capture_is_stale(started, started + Duration::from_secs(1)));
    }

    #[test]
    fn a_capture_is_stale_once_the_watchdog_expires() {
        let started = Instant::now();

        assert!(capture_is_stale(started, started + CAPTURE_WATCHDOG));
        assert!(capture_is_stale(
            started,
            started + CAPTURE_WATCHDOG + Duration::from_secs(30)
        ));
    }

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
            SkipReason::ClipboardChanged,
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
