use std::time::Duration;

use crate::error::AppError;

/// Quanto esperar o usuário soltar `Ctrl`/`Alt`/`Shift`/`Win` antes de simular o copiar.
pub const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(600);
/// Quanto esperar o app de origem colocar a seleção no clipboard.
pub const COPY_TIMEOUT: Duration = Duration::from_millis(500);
pub const POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Teto do texto levado ao popup, para uma seleção gigante não atravessar o IPC.
pub const MAX_PREFILL_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    /// Pixels RGBA, 4 bytes por pixel.
    pub bytes: Vec<u8>,
}

/// Conteúdo do clipboard antes da captura, para poder devolvê-lo depois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardSnapshot {
    Empty,
    Text(String),
    Image(ClipboardImage),
    /// Arquivos, HTML rico etc.: não dá para restaurar, então a captura é recusada.
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// O usuário não soltou as teclas do atalho a tempo.
    ModifiersHeld,
    /// Capturar destruiria o conteúdo atual do clipboard.
    UnsupportedClipboard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOutcome {
    Captured {
        text: String,
        /// O clipboard anterior não pôde ser devolvido; vale registrar no log.
        restore_failed: bool,
    },
    NoSelection,
    Skipped(SkipReason),
}

pub trait ClipboardPort {
    /// Contador que muda a cada alteração do clipboard, mesmo com conteúdo igual.
    fn sequence(&self) -> u32;
    fn snapshot(&self) -> Result<ClipboardSnapshot, AppError>;
    /// `None` quando o clipboard não tem texto.
    fn read_text(&self) -> Result<Option<String>, AppError>;
    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), AppError>;
}

pub trait InputPort {
    fn modifiers_pressed(&self) -> bool;
    /// Simula Ctrl+C no app em foco. As teclas devem sempre terminar soltas.
    fn send_copy(&self) -> Result<(), AppError>;
}

pub trait Sleeper {
    fn sleep(&self, duration: Duration);
}

/// Copia a seleção do app em foco e devolve o clipboard como estava.
///
/// O clipboard só é alterado depois que o contador de sequência prova que o app copiou algo,
/// e a partir daí é sempre restaurado, inclusive quando a leitura do texto falha.
pub fn capture_selection(
    clipboard: &dyn ClipboardPort,
    input: &dyn InputPort,
    sleeper: &dyn Sleeper,
) -> Result<CaptureOutcome, AppError> {
    // Com Ctrl/Alt do atalho ainda pressionados, o "C" simulado viraria Ctrl+Alt+C.
    if !wait_until(sleeper, MODIFIER_RELEASE_TIMEOUT, || {
        !input.modifiers_pressed()
    }) {
        return Ok(CaptureOutcome::Skipped(SkipReason::ModifiersHeld));
    }

    let snapshot = clipboard.snapshot()?;
    if snapshot == ClipboardSnapshot::Unsupported {
        return Ok(CaptureOutcome::Skipped(SkipReason::UnsupportedClipboard));
    }

    let sequence_before = clipboard.sequence();
    input.send_copy()?;

    if !wait_until(sleeper, COPY_TIMEOUT, || {
        clipboard.sequence() != sequence_before
    }) {
        return Ok(CaptureOutcome::NoSelection);
    }

    let copied = clipboard.read_text();
    let restored = clipboard.restore(&snapshot);

    let Some(text) = copied? else {
        return Ok(CaptureOutcome::NoSelection);
    };
    let text = normalize(&text);
    if text.is_empty() {
        return Ok(CaptureOutcome::NoSelection);
    }

    Ok(CaptureOutcome::Captured {
        text,
        restore_failed: restored.is_err(),
    })
}

/// Consulta `condition` a cada `POLL_INTERVAL` até `timeout`. Devolve se ela chegou a valer.
fn wait_until(
    sleeper: &dyn Sleeper,
    timeout: Duration,
    mut condition: impl FnMut() -> bool,
) -> bool {
    let attempts = timeout.as_millis() / POLL_INTERVAL.as_millis();
    for _ in 0..=attempts {
        if condition() {
            return true;
        }
        sleeper.sleep(POLL_INTERVAL);
    }
    false
}

fn normalize(text: &str) -> String {
    text.trim().chars().take(MAX_PREFILL_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    use super::*;

    /// Simula clipboard e teclado de um desktop. A cópia pedida por `send_copy` só aparece no
    /// clipboard depois de `copy_latency_polls` consultas ao contador, como num app lento.
    struct FakeDesktop {
        clipboard: Mutex<ClipboardSnapshot>,
        sequence: AtomicU32,
        selection: Option<String>,
        modifier_polls_pressed: AtomicU32,
        copy_latency_polls: u32,
        pending_copy_polls: Mutex<Option<u32>>,
        copies_sent: AtomicU32,
        restores: AtomicU32,
        fail_read: bool,
        fail_restore: bool,
        fail_send: bool,
        unsupported_after_copy: bool,
    }

    impl FakeDesktop {
        fn with_clipboard(content: ClipboardSnapshot) -> Self {
            Self {
                clipboard: Mutex::new(content),
                sequence: AtomicU32::new(1),
                selection: None,
                modifier_polls_pressed: AtomicU32::new(0),
                copy_latency_polls: 0,
                pending_copy_polls: Mutex::new(None),
                copies_sent: AtomicU32::new(0),
                restores: AtomicU32::new(0),
                fail_read: false,
                fail_restore: false,
                fail_send: false,
                unsupported_after_copy: false,
            }
        }

        fn selecting(mut self, text: &str) -> Self {
            self.selection = Some(text.to_owned());
            self
        }

        fn current(&self) -> ClipboardSnapshot {
            self.clipboard.lock().unwrap().clone()
        }

        fn copies_sent(&self) -> u32 {
            self.copies_sent.load(Ordering::SeqCst)
        }

        fn restores(&self) -> u32 {
            self.restores.load(Ordering::SeqCst)
        }
    }

    impl ClipboardPort for FakeDesktop {
        fn sequence(&self) -> u32 {
            let mut pending = self.pending_copy_polls.lock().unwrap();
            if let Some(polls) = *pending {
                if polls >= self.copy_latency_polls {
                    *pending = None;
                    if let Some(selection) = &self.selection {
                        *self.clipboard.lock().unwrap() =
                            ClipboardSnapshot::Text(selection.clone());
                        self.sequence.fetch_add(1, Ordering::SeqCst);
                    }
                } else {
                    *pending = Some(polls + 1);
                }
            }
            self.sequence.load(Ordering::SeqCst)
        }

        fn snapshot(&self) -> Result<ClipboardSnapshot, AppError> {
            Ok(self.current())
        }

        fn read_text(&self) -> Result<Option<String>, AppError> {
            if self.fail_read {
                return Err(AppError::Clipboard("leitura falhou".into()));
            }
            if self.unsupported_after_copy {
                return Ok(None);
            }
            match self.current() {
                ClipboardSnapshot::Text(text) => Ok(Some(text)),
                _ => Ok(None),
            }
        }

        fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), AppError> {
            self.restores.fetch_add(1, Ordering::SeqCst);
            if self.fail_restore {
                return Err(AppError::Clipboard("restauração falhou".into()));
            }
            *self.clipboard.lock().unwrap() = snapshot.clone();
            self.sequence.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    impl InputPort for FakeDesktop {
        fn modifiers_pressed(&self) -> bool {
            let remaining = self.modifier_polls_pressed.load(Ordering::SeqCst);
            if remaining == u32::MAX {
                return true;
            }
            if remaining > 0 {
                self.modifier_polls_pressed
                    .store(remaining - 1, Ordering::SeqCst);
                return true;
            }
            false
        }

        fn send_copy(&self) -> Result<(), AppError> {
            if self.fail_send {
                return Err(AppError::Capture("SendInput falhou".into()));
            }
            self.copies_sent.fetch_add(1, Ordering::SeqCst);
            *self.pending_copy_polls.lock().unwrap() = Some(0);
            Ok(())
        }
    }

    #[derive(Default)]
    struct CountingSleeper {
        sleeps: AtomicU32,
    }

    impl Sleeper for CountingSleeper {
        fn sleep(&self, _duration: Duration) {
            self.sleeps.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn run(desktop: &FakeDesktop) -> Result<CaptureOutcome, AppError> {
        capture_selection(desktop, desktop, &CountingSleeper::default())
    }

    fn text(content: &str) -> ClipboardSnapshot {
        ClipboardSnapshot::Text(content.to_owned())
    }

    fn captured(content: &str) -> CaptureOutcome {
        CaptureOutcome::Captured {
            text: content.to_owned(),
            restore_failed: false,
        }
    }

    #[test]
    fn captures_the_selection_and_restores_the_previous_text() {
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting("olá mundo");

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, captured("olá mundo"));
        assert_eq!(desktop.current(), text("antes"));
        assert_eq!(desktop.copies_sent(), 1);
    }

    #[test]
    fn restores_an_image_that_was_on_the_clipboard() {
        let image = ClipboardSnapshot::Image(ClipboardImage {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0, 255],
        });
        let desktop = FakeDesktop::with_clipboard(image.clone()).selecting("texto");

        run(&desktop).unwrap();

        assert_eq!(desktop.current(), image);
    }

    #[test]
    fn restores_an_empty_clipboard() {
        let desktop = FakeDesktop::with_clipboard(ClipboardSnapshot::Empty).selecting("texto");

        run(&desktop).unwrap();

        assert_eq!(desktop.current(), ClipboardSnapshot::Empty);
    }

    #[test]
    fn waits_for_the_modifiers_to_be_released_before_copying() {
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.modifier_polls_pressed.store(5, Ordering::SeqCst);
        let sleeper = CountingSleeper::default();

        let outcome = capture_selection(&desktop, &desktop, &sleeper).unwrap();

        assert_eq!(outcome, captured("texto"));
        assert!(sleeper.sleeps.load(Ordering::SeqCst) >= 5);
        assert_eq!(desktop.copies_sent(), 1);
    }

    #[test]
    fn skips_without_copying_when_the_modifiers_are_never_released() {
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop
            .modifier_polls_pressed
            .store(u32::MAX, Ordering::SeqCst);

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, CaptureOutcome::Skipped(SkipReason::ModifiersHeld));
        assert_eq!(desktop.copies_sent(), 0);
    }

    #[test]
    fn skips_without_copying_when_the_clipboard_cannot_be_restored() {
        let desktop =
            FakeDesktop::with_clipboard(ClipboardSnapshot::Unsupported).selecting("texto");

        let outcome = run(&desktop).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::Skipped(SkipReason::UnsupportedClipboard)
        );
        assert_eq!(desktop.copies_sent(), 0);
    }

    #[test]
    fn reports_no_selection_and_leaves_the_clipboard_alone_when_nothing_is_copied() {
        let desktop = FakeDesktop::with_clipboard(text("antes"));

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, CaptureOutcome::NoSelection);
        assert_eq!(desktop.restores(), 0);
        assert_eq!(desktop.current(), text("antes"));
    }

    #[test]
    fn detects_a_selection_identical_to_the_current_clipboard_text() {
        let desktop = FakeDesktop::with_clipboard(text("igual")).selecting("igual");

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, captured("igual"));
    }

    #[test]
    fn waits_for_a_slow_app_to_fill_the_clipboard() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("lento");
        desktop.copy_latency_polls = 20;

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, captured("lento"));
    }

    #[test]
    fn gives_up_on_an_app_slower_than_the_copy_timeout() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("lento");
        desktop.copy_latency_polls = 1_000;

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, CaptureOutcome::NoSelection);
    }

    #[test]
    fn treats_a_whitespace_only_selection_as_no_selection_and_restores_the_clipboard() {
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting("  \n\t ");

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, CaptureOutcome::NoSelection);
        assert_eq!(desktop.current(), text("antes"));
    }

    #[test]
    fn treats_a_non_text_copy_result_as_no_selection_and_restores_the_clipboard() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("x");
        desktop.unsupported_after_copy = true;

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, CaptureOutcome::NoSelection);
        assert_eq!(desktop.current(), text("antes"));
    }

    #[test]
    fn trims_the_selection_and_caps_its_length() {
        let long = format!("  {}  ", "a".repeat(MAX_PREFILL_CHARS + 500));
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting(&long);

        let outcome = run(&desktop).unwrap();

        let CaptureOutcome::Captured { text, .. } = outcome else {
            panic!("esperava uma captura");
        };
        assert_eq!(text.chars().count(), MAX_PREFILL_CHARS);
        assert!(!text.starts_with(' '));
    }

    #[test]
    fn still_returns_the_text_when_the_clipboard_cannot_be_restored() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.fail_restore = true;

        let outcome = run(&desktop).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::Captured {
                text: "texto".into(),
                restore_failed: true,
            }
        );
    }

    #[test]
    fn restores_the_clipboard_even_when_reading_the_copy_fails() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.fail_read = true;

        let error = run(&desktop).unwrap_err();

        assert_eq!(error.code(), "clipboard");
        assert_eq!(desktop.restores(), 1);
        assert_eq!(desktop.current(), text("antes"));
    }

    #[test]
    fn propagates_a_failure_to_send_the_copy_without_touching_the_clipboard() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.fail_send = true;

        let error = run(&desktop).unwrap_err();

        assert_eq!(error.code(), "capture");
        assert_eq!(desktop.restores(), 0);
        assert_eq!(desktop.current(), text("antes"));
    }
}
