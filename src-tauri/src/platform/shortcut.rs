use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::accelerator::{self, Accelerator};
use super::popup_window::show_near_cursor;
use super::{capture_foreground_selection, log_failure};
use crate::error::AppError;
use crate::services::capture::CaptureOutcome;
use crate::settings::{Settings, SettingsStore, DEFAULT_SHORTCUT};

/// Registro de atalhos globais no sistema. Existe para a troca de atalho (e o seu rollback)
/// poder ser testada sem o sistema.
pub trait ShortcutRegistrar {
    fn register(&self, shortcut: Shortcut) -> Result<(), String>;
    fn unregister(&self, shortcut: Shortcut) -> Result<(), String>;
}

impl ShortcutRegistrar for AppHandle {
    fn register(&self, shortcut: Shortcut) -> Result<(), String> {
        self.global_shortcut()
            .register(shortcut)
            .map_err(|error| error.to_string())
    }

    fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
        self.global_shortcut()
            .unregister(shortcut)
            .map_err(|error| error.to_string())
    }
}

/// O atalho salvo; se estiver inválido, o de fábrica.
pub fn startup_accelerator(saved: &str) -> Result<Accelerator, AppError> {
    accelerator::parse(saved).or_else(|error| {
        log::warn!("atalho salvo inválido ({saved:?}): {error:?}; usando o de fábrica");
        accelerator::parse(DEFAULT_SHORTCUT)
    })
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

/// Registra o atalho ao iniciar. Devolve `false` se não foi possível (por exemplo, já em uso
/// por outro app).
pub fn register_initial(registrar: &impl ShortcutRegistrar, accelerator: &Accelerator) -> bool {
    match registrar.register(accelerator.shortcut) {
        Ok(()) => true,
        Err(error) => {
            log::error!(
                "não foi possível registrar o atalho {}: {error}",
                accelerator.label
            );
            false
        }
    }
}

/// Troca o atalho registrado pelo `new` e só então o grava nas configurações. Se o registro
/// falhar, o atual continua valendo; se a gravação falhar, o registro volta ao atual.
pub fn apply_shortcut(
    registrar: &impl ShortcutRegistrar,
    store: &SettingsStore,
    new: &Accelerator,
) -> Result<Settings, AppError> {
    let current = startup_accelerator(&store.get().shortcut)?;
    swap(registrar, &current, new)?;
    store.update_shortcut(&new.label).inspect_err(|_| {
        if let Err(error) = swap(registrar, new, &current) {
            log::error!(
                "não foi possível restaurar o atalho {}: {error:?}",
                current.label
            );
        }
    })
}

/// Troca `old` por `new`. Se `new` não puder ser registrado (por exemplo, outro app o usa),
/// `old` volta a valer: o usuário nunca fica sem atalho por causa de uma tentativa.
pub fn swap(
    registrar: &impl ShortcutRegistrar,
    old: &Accelerator,
    new: &Accelerator,
) -> Result<(), AppError> {
    // O antigo pode nem estar registrado (falhou ao iniciar): não é motivo para parar.
    if let Err(error) = registrar.unregister(old.shortcut) {
        log::info!("atalho antigo {} não estava registrado: {error}", old.label);
    }

    match registrar.register(new.shortcut) {
        Ok(()) => Ok(()),
        Err(error) => {
            log::warn!("não foi possível registrar o atalho {}: {error}", new.label);
            if let Err(rollback) = registrar.register(old.shortcut) {
                log::error!(
                    "não foi possível restaurar o atalho {}: {rollback}",
                    old.label
                );
            }
            Err(AppError::InvalidInput(
                "Não foi possível usar esse atalho. Outro app pode já estar usando.".into(),
            ))
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

    /// Registro falso: guarda o que está registrado e recusa os atalhos listados em `refuse`.
    #[derive(Default)]
    struct FakeRegistrar {
        registered: Mutex<Vec<Shortcut>>,
        refuse: Vec<Shortcut>,
    }

    impl FakeRegistrar {
        fn with(registered: &[&Accelerator], refuse: &[&Accelerator]) -> Self {
            Self {
                registered: Mutex::new(registered.iter().map(|a| a.shortcut).collect()),
                refuse: refuse.iter().map(|a| a.shortcut).collect(),
            }
        }

        fn registered(&self) -> Vec<Shortcut> {
            self.registered.lock().unwrap().clone()
        }
    }

    impl ShortcutRegistrar for FakeRegistrar {
        fn register(&self, shortcut: Shortcut) -> Result<(), String> {
            let mut registered = self.registered.lock().unwrap();
            if self.refuse.contains(&shortcut) || registered.contains(&shortcut) {
                return Err("em uso".into());
            }
            registered.push(shortcut);
            Ok(())
        }

        fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
            let mut registered = self.registered.lock().unwrap();
            let before = registered.len();
            registered.retain(|known| *known != shortcut);
            if registered.len() == before {
                return Err("não registrado".into());
            }
            Ok(())
        }
    }

    fn accelerator(text: &str) -> Accelerator {
        accelerator::parse(text).unwrap()
    }

    #[test]
    fn swapping_registers_the_new_shortcut_and_releases_the_old_one() {
        let (old, new) = (accelerator("Ctrl+Alt+T"), accelerator("Ctrl+Alt+K"));
        let registrar = FakeRegistrar::with(&[&old], &[]);

        swap(&registrar, &old, &new).unwrap();

        assert_eq!(registrar.registered(), vec![new.shortcut]);
    }

    #[test]
    fn a_refused_shortcut_rolls_back_to_the_old_one_and_reports_an_error() {
        let (old, new) = (accelerator("Ctrl+Alt+T"), accelerator("Ctrl+Alt+K"));
        let registrar = FakeRegistrar::with(&[&old], &[&new]);

        let error = swap(&registrar, &old, &new).unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert_eq!(registrar.registered(), vec![old.shortcut]);
    }

    #[test]
    fn swapping_works_when_the_old_shortcut_was_never_registered() {
        let (old, new) = (accelerator("Ctrl+Alt+T"), accelerator("Ctrl+Alt+K"));
        let registrar = FakeRegistrar::with(&[], &[]);

        swap(&registrar, &old, &new).unwrap();

        assert_eq!(registrar.registered(), vec![new.shortcut]);
    }

    #[test]
    fn applying_the_same_shortcut_again_keeps_it_registered() {
        let current = accelerator("Ctrl+Alt+T");
        let registrar = FakeRegistrar::with(&[&current], &[]);

        swap(&registrar, &current, &current).unwrap();

        assert_eq!(registrar.registered(), vec![current.shortcut]);
    }

    fn store_in(dir: &tempfile::TempDir) -> SettingsStore {
        SettingsStore::load(&dir.path().join("settings.json"))
    }

    #[test]
    fn applying_a_shortcut_registers_it_and_saves_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let old = accelerator("Ctrl+Alt+T");
        let new = accelerator("Ctrl+Shift+J");
        let registrar = FakeRegistrar::with(&[&old], &[]);

        let saved = apply_shortcut(&registrar, &store, &new).unwrap();

        assert_eq!(saved.shortcut, "Ctrl+Shift+J");
        assert_eq!(store.get().shortcut, "Ctrl+Shift+J");
        assert_eq!(registrar.registered(), vec![new.shortcut]);
        assert_eq!(store_in(&dir).get().shortcut, "Ctrl+Shift+J");
    }

    #[test]
    fn a_refused_shortcut_changes_nothing_not_even_the_saved_setting() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let old = accelerator("Ctrl+Alt+T");
        let new = accelerator("Ctrl+Shift+J");
        let registrar = FakeRegistrar::with(&[&old], &[&new]);

        let error = apply_shortcut(&registrar, &store, &new).unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert_eq!(store.get().shortcut, "Ctrl+Alt+T");
        assert_eq!(registrar.registered(), vec![old.shortcut]);
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn when_saving_fails_the_previous_shortcut_is_registered_again() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("bloqueio");
        std::fs::write(&blocker, "x").unwrap();
        let store = SettingsStore::load(&blocker.join("settings.json"));
        let old = accelerator("Ctrl+Alt+T");
        let new = accelerator("Ctrl+Shift+J");
        let registrar = FakeRegistrar::with(&[&old], &[]);

        let error = apply_shortcut(&registrar, &store, &new).unwrap_err();

        assert_eq!(error.code(), "settings");
        assert_eq!(registrar.registered(), vec![old.shortcut]);
        assert_eq!(store.get().shortcut, "Ctrl+Alt+T");
    }

    #[test]
    fn initial_registration_reports_whether_it_worked() {
        let free = accelerator("Ctrl+Alt+T");
        let taken = accelerator("Ctrl+Alt+K");
        let registrar = FakeRegistrar::with(&[], &[&taken]);

        assert!(register_initial(&registrar, &free));
        assert!(!register_initial(&registrar, &taken));
    }

    #[test]
    fn a_valid_saved_shortcut_is_used_and_an_invalid_one_falls_back_to_the_factory_one() {
        assert_eq!(
            startup_accelerator("Ctrl+Shift+J").unwrap().label,
            "Ctrl+Shift+J"
        );
        assert_eq!(startup_accelerator("lixo").unwrap().label, DEFAULT_SHORTCUT);
        assert_eq!(startup_accelerator("").unwrap().label, DEFAULT_SHORTCUT);
    }
}
