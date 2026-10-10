use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::accelerator::{self, Accelerator};
use super::popup_window::show_near_cursor;
use super::{capture_foreground_selection, log_failure, region_window};
use crate::error::AppError;
use crate::services::capture::CaptureOutcome;
use crate::settings::{Settings, SettingsStore, ShortcutSlot};

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

/// O atalho salvo para `slot`; se estiver inválido, o de fábrica.
pub fn startup_accelerator(saved: &str, slot: ShortcutSlot) -> Result<Accelerator, AppError> {
    accelerator::parse(saved).or_else(|error| {
        log::warn!("atalho salvo inválido ({saved:?}): {error:?}; usando o de fábrica");
        accelerator::parse(slot.factory_default())
    })
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            match slot_for(shortcut, registered_shortcut(ShortcutSlot::Ocr).as_ref()) {
                ShortcutSlot::Translate => open_with_selection(app.clone()),
                ShortcutSlot::Ocr => region_window::start(app.clone()),
            }
        })
        .build()
}

/// A qual ação pertence o atalho apertado. Só há dois registrados: o que não é o de OCR é o
/// do tradutor.
pub fn slot_for(pressed: &Shortcut, ocr: Option<&Shortcut>) -> ShortcutSlot {
    if ocr == Some(pressed) {
        ShortcutSlot::Ocr
    } else {
        ShortcutSlot::Translate
    }
}

/// Alternativa de fábrica do OCR quando o tradutor já usa o atalho de OCR de fábrica.
const OCR_COLLISION_FALLBACK: &str = "Ctrl+Alt+Shift+Y";

/// O atalho de OCR que não colide com o do tradutor. Quem já usava `Ctrl+Alt+Y` no tradutor
/// antes de existir o OCR não pode perdê-lo para o atalho novo.
pub fn without_collision(
    translate: &Accelerator,
    ocr: Accelerator,
) -> Result<Accelerator, AppError> {
    if ocr.shortcut != translate.shortcut {
        return Ok(ocr);
    }
    let candidates = [ShortcutSlot::Ocr.factory_default(), OCR_COLLISION_FALLBACK];
    for candidate in candidates {
        let fallback = accelerator::parse(candidate)?;
        if fallback.shortcut != translate.shortcut {
            log::warn!(
                "o atalho de OCR {} é o mesmo do tradutor; usando {}",
                ocr.label,
                fallback.label
            );
            return Ok(fallback);
        }
    }
    // Inalcançável: as duas alternativas são diferentes, e o tradutor só ocupa uma delas.
    Err(AppError::ShortcutInUse)
}

/// O que está registrado no sistema para um atalho: o texto (para os avisos) e, se o registro
/// deu certo, o atalho em si.
#[derive(Clone)]
struct SlotStatus {
    label: String,
    registered: Option<Shortcut>,
}

const NO_STATUS: SlotStatus = SlotStatus {
    label: String::new(),
    registered: None,
};

/// Estado de cada slot, na ordem de `ShortcutSlot::ALL`. Fica em memória, ao lado do registro
/// no sistema: o atalho apertado é comparado com o que foi registrado, não com o arquivo.
static STATUS: Mutex<[SlotStatus; 2]> = Mutex::new([NO_STATUS, NO_STATUS]);

fn slot_index(slot: ShortcutSlot) -> usize {
    match slot {
        ShortcutSlot::Translate => 0,
        ShortcutSlot::Ocr => 1,
    }
}

fn with_status<R>(read: impl FnOnce(&mut [SlotStatus; 2]) -> R) -> R {
    let mut status = STATUS.lock().unwrap_or_else(PoisonError::into_inner);
    read(&mut status)
}

/// Anota o atalho de `slot` e se o sistema aceitou registrá-lo.
pub fn record_status(slot: ShortcutSlot, accelerator: &Accelerator, registered: bool) {
    with_status(|status| {
        status[slot_index(slot)] = SlotStatus {
            label: accelerator.label.clone(),
            registered: registered.then_some(accelerator.shortcut),
        };
    });
}

fn registered_shortcut(slot: ShortcutSlot) -> Option<Shortcut> {
    with_status(|status| status[slot_index(slot)].registered)
}

/// Os atalhos anotados que o sistema não aceitou registrar.
pub fn unavailable_labels() -> Vec<String> {
    with_status(|status| {
        status
            .iter()
            .filter(|slot| slot.registered.is_none() && !slot.label.is_empty())
            .map(|slot| slot.label.clone())
            .collect()
    })
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

/// Troca o atalho de `slot` pelo `new` e só então o grava nas configurações. Se o registro
/// falhar, o atual continua valendo; se a gravação falhar, o registro volta ao atual.
pub fn apply_shortcut(
    registrar: &impl ShortcutRegistrar,
    store: &SettingsStore,
    slot: ShortcutSlot,
    new: &Accelerator,
) -> Result<Settings, AppError> {
    let settings = store.get();
    let other = slot.other();
    // O sistema aceitaria registrar o mesmo atalho de novo, e um dos dois deixaria de funcionar.
    if startup_accelerator(settings.shortcut_for(other), other)?.shortcut == new.shortcut {
        return Err(AppError::InvalidInput(format!(
            "Esse atalho já é usado para {}. Escolha outro.",
            other.description()
        )));
    }
    let current = startup_accelerator(settings.shortcut_for(slot), slot)?;
    swap(registrar, &current, new)?;
    store.update_shortcut(slot, &new.label).inspect_err(|_| {
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
            Err(AppError::ShortcutInUse)
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
    use crate::settings::{DEFAULT_OCR_SHORTCUT, DEFAULT_SHORTCUT};

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

        assert_eq!(error.code(), "shortcut_in_use");
        assert_eq!(registrar.registered(), vec![old.shortcut]);
    }

    #[test]
    fn the_error_stays_shortcut_in_use_even_if_the_rollback_fails() {
        let (old, new) = (accelerator("Ctrl+Alt+T"), accelerator("Ctrl+Alt+K"));
        let registrar = FakeRegistrar::with(&[&old], &[&new, &old]);

        let error = swap(&registrar, &old, &new).unwrap_err();

        assert_eq!(error.code(), "shortcut_in_use");
    }

    #[test]
    fn an_invalid_shortcut_is_rejected_before_touching_the_registrar() {
        let old = accelerator("Ctrl+Alt+T");
        let registrar = FakeRegistrar::with(&[&old], &[]);

        let error = accelerator::parse("Ctrl+C").unwrap_err();

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

        let saved = apply_shortcut(&registrar, &store, ShortcutSlot::Translate, &new).unwrap();

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

        let error = apply_shortcut(&registrar, &store, ShortcutSlot::Translate, &new).unwrap_err();

        assert_eq!(error.code(), "shortcut_in_use");
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

        let error = apply_shortcut(&registrar, &store, ShortcutSlot::Translate, &new).unwrap_err();

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
        let translate = ShortcutSlot::Translate;
        assert_eq!(
            startup_accelerator("Ctrl+Shift+J", translate)
                .unwrap()
                .label,
            "Ctrl+Shift+J"
        );
        assert_eq!(
            startup_accelerator("lixo", translate).unwrap().label,
            DEFAULT_SHORTCUT
        );
        assert_eq!(
            startup_accelerator("", translate).unwrap().label,
            DEFAULT_SHORTCUT
        );
    }

    #[test]
    fn an_invalid_saved_ocr_shortcut_falls_back_to_the_ocr_factory_one() {
        assert_eq!(
            startup_accelerator("lixo", ShortcutSlot::Ocr)
                .unwrap()
                .label,
            DEFAULT_OCR_SHORTCUT
        );
    }

    #[test]
    fn applying_the_ocr_shortcut_saves_it_and_leaves_the_translator_one_alone() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let translate = accelerator(DEFAULT_SHORTCUT);
        let old = accelerator(DEFAULT_OCR_SHORTCUT);
        let new = accelerator("Ctrl+Shift+O");
        let registrar = FakeRegistrar::with(&[&translate, &old], &[]);

        let saved = apply_shortcut(&registrar, &store, ShortcutSlot::Ocr, &new).unwrap();

        assert_eq!(saved.ocr_shortcut, "Ctrl+Shift+O");
        assert_eq!(saved.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(
            registrar.registered(),
            vec![translate.shortcut, new.shortcut]
        );
    }

    #[test]
    fn the_translator_shortcut_cannot_be_used_for_ocr_too() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let translate = accelerator(DEFAULT_SHORTCUT);
        let ocr = accelerator(DEFAULT_OCR_SHORTCUT);
        let registrar = FakeRegistrar::with(&[&translate, &ocr], &[]);

        let error = apply_shortcut(&registrar, &store, ShortcutSlot::Ocr, &translate).unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert!(error.to_string().contains("abrir o tradutor"));
        assert_eq!(
            registrar.registered(),
            vec![translate.shortcut, ocr.shortcut]
        );
        assert_eq!(store.get().ocr_shortcut, DEFAULT_OCR_SHORTCUT);
    }

    #[test]
    fn the_ocr_shortcut_cannot_be_used_for_the_translator_too() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let translate = accelerator(DEFAULT_SHORTCUT);
        let ocr = accelerator(DEFAULT_OCR_SHORTCUT);
        let registrar = FakeRegistrar::with(&[&translate, &ocr], &[]);

        let error = apply_shortcut(&registrar, &store, ShortcutSlot::Translate, &ocr).unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert!(error.to_string().contains("ler um trecho da tela"));
        assert_eq!(store.get().shortcut, DEFAULT_SHORTCUT);
    }

    #[test]
    fn the_pressed_shortcut_decides_which_action_runs() {
        let translate = accelerator(DEFAULT_SHORTCUT);
        let ocr = accelerator(DEFAULT_OCR_SHORTCUT);

        assert_eq!(
            slot_for(&ocr.shortcut, Some(&ocr.shortcut)),
            ShortcutSlot::Ocr
        );
        assert_eq!(
            slot_for(&translate.shortcut, Some(&ocr.shortcut)),
            ShortcutSlot::Translate
        );
        assert_eq!(slot_for(&ocr.shortcut, None), ShortcutSlot::Translate);
    }

    #[test]
    fn an_ocr_shortcut_that_does_not_collide_is_kept() {
        let translate = accelerator("Ctrl+Alt+T");

        let ocr = without_collision(&translate, accelerator("Ctrl+Shift+O")).unwrap();

        assert_eq!(ocr.label, "Ctrl+Shift+O");
    }

    #[test]
    fn a_translator_already_on_the_ocr_factory_shortcut_pushes_ocr_to_the_fallback() {
        let translate = accelerator(DEFAULT_OCR_SHORTCUT);

        let ocr = without_collision(&translate, accelerator(DEFAULT_OCR_SHORTCUT)).unwrap();

        assert_eq!(ocr.label, "Ctrl+Alt+Shift+Y");
        assert_ne!(ocr.shortcut, translate.shortcut);
    }

    #[test]
    fn a_custom_ocr_shortcut_equal_to_the_translator_goes_back_to_the_factory_one() {
        let translate = accelerator("Ctrl+Shift+J");

        let ocr = without_collision(&translate, accelerator("Ctrl+Shift+J")).unwrap();

        assert_eq!(ocr.label, DEFAULT_OCR_SHORTCUT);
    }
}
