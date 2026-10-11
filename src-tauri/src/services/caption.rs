//! Modo legenda: relê a mesma região da tela de tempos em tempos e só manda traduzir quando o
//! texto mudou de verdade, para não gastar a cota do DeepL com a mesma fala.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use crate::error::AppError;

pub const DEFAULT_INTERVAL_MS: u32 = 1500;
/// Abaixo disso o OCR e a captura ocupariam a máquina durante o jogo.
pub const MIN_INTERVAL_MS: u32 = 500;
pub const MAX_INTERVAL_MS: u32 = 10_000;
/// Leituras iguais seguidas para o texto valer: segura falas que aparecem letra a letra.
const STABLE_READS: u32 = 2;
/// Falhas seguidas (captura ou OCR) que encerram a legenda.
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// O intervalo configurado, limitado à faixa segura.
pub fn interval(milliseconds: u32) -> Duration {
    Duration::from_millis(u64::from(
        milliseconds.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS),
    ))
}

/// Impressão digital do texto: minúsculas, só letras e números, palavras separadas por um
/// espaço. Variações de pontuação ou espaço do OCR contam como o mesmo texto. `None` quando
/// não sobra nada.
fn fingerprint(text: &str) -> Option<u64> {
    let normalized = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    Some(hasher.finish())
}

/// Decide quais leituras viram tradução.
#[derive(Debug, Default)]
pub struct CaptionGate {
    last_sent: Option<u64>,
    /// O texto candidato e quantas vezes seguidas ele foi lido.
    pending: Option<(u64, u32)>,
}

impl CaptionGate {
    /// Começa sabendo o texto que já foi traduzido (a leitura que abriu o popup).
    pub fn after(text: &str) -> Self {
        Self {
            last_sent: fingerprint(text),
            pending: None,
        }
    }

    /// Recebe uma leitura; devolve o texto a traduzir quando ele é novo e ficou estável.
    pub fn observe(&mut self, text: &str) -> Option<String> {
        let Some(print) = fingerprint(text) else {
            self.pending = None;
            return None;
        };
        if self.last_sent == Some(print) {
            self.pending = None;
            return None;
        }
        let reads = match self.pending {
            Some((pending, count)) if pending == print => count + 1,
            _ => 1,
        };
        if reads < STABLE_READS {
            self.pending = Some((print, reads));
            return None;
        }
        self.pending = None;
        self.last_sent = Some(print);
        Some(text.trim().to_owned())
    }
}

/// De onde vem cada leitura (captura da região + OCR).
pub trait CaptionSource {
    fn read(&self) -> Result<String, AppError>;
}

/// Para onde vão os textos novos e o aviso de falha.
pub trait CaptionSink {
    fn caption(&self, text: &str);
    /// A legenda parou sozinha, depois de falhas seguidas. Parar pelo usuário não avisa nada:
    /// quem parou já sabe.
    fn failed(&self, error: &AppError);
}

/// Relê a cada `interval` até `stop` receber um sinal (ou o remetente sumir) ou até
/// [`MAX_CONSECUTIVE_FAILURES`] falhas seguidas. Região sem texto não é falha: é o intervalo
/// entre duas falas.
pub fn run(
    source: &impl CaptionSource,
    sink: &impl CaptionSink,
    gate: CaptionGate,
    interval: Duration,
    stop: &Receiver<()>,
) {
    let mut gate = gate;
    let mut failures = 0;
    loop {
        match stop.recv_timeout(interval) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
        match source.read() {
            Ok(text) => {
                failures = 0;
                if let Some(new) = gate.observe(&text) {
                    sink.caption(&new);
                }
            }
            Err(AppError::NoTextFound) => {
                failures = 0;
                gate.observe("");
            }
            Err(error) => {
                failures += 1;
                log::warn!("falha ao reler a legenda: {error:?}");
                if failures >= MAX_CONSECUTIVE_FAILURES {
                    sink.failed(&error);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::mpsc;
    use std::sync::Mutex;

    use super::*;

    fn feed(gate: &mut CaptionGate, reads: &[&str]) -> Vec<String> {
        reads.iter().filter_map(|text| gate.observe(text)).collect()
    }

    #[test]
    fn a_new_text_is_sent_once_it_is_read_twice_in_a_row() {
        let mut gate = CaptionGate::default();

        assert_eq!(feed(&mut gate, &["Run!", "Run!", "Run!"]), vec!["Run!"]);
    }

    #[test]
    fn the_text_that_opened_the_popup_is_not_translated_again() {
        let mut gate = CaptionGate::after("Press any key");

        assert!(feed(&mut gate, &["Press any key", "Press any key"]).is_empty());
    }

    #[test]
    fn ocr_noise_in_case_spacing_or_punctuation_is_the_same_text() {
        let mut gate = CaptionGate::after("Hello, there!");

        assert!(feed(&mut gate, &["hello  there", "HELLO THERE."]).is_empty());
    }

    #[test]
    fn text_that_appears_letter_by_letter_is_sent_only_when_complete() {
        let mut gate = CaptionGate::default();

        let sent = feed(
            &mut gate,
            &[
                "Wh",
                "Where a",
                "Where are you",
                "Where are you",
                "Where are you",
            ],
        );

        assert_eq!(sent, vec!["Where are you"]);
    }

    #[test]
    fn an_empty_read_breaks_the_sequence() {
        let mut gate = CaptionGate::default();

        assert!(feed(&mut gate, &["Go", "", "Go"]).is_empty());
        assert_eq!(feed(&mut gate, &["Go"]), vec!["Go"]);
    }

    #[test]
    fn a_line_that_comes_back_after_another_is_sent_again() {
        let mut gate = CaptionGate::default();

        let sent = feed(&mut gate, &["Yes", "Yes", "No", "No", "Yes", "Yes"]);

        assert_eq!(sent, vec!["Yes", "No", "Yes"]);
    }

    #[test]
    fn the_interval_is_kept_inside_the_safe_range() {
        assert_eq!(interval(1500), Duration::from_millis(1500));
        assert_eq!(interval(10), Duration::from_millis(500));
        assert_eq!(interval(u32::MAX), Duration::from_millis(10_000));
    }

    /// Fonte falsa: devolve as leituras combinadas e, quando acabam, falha.
    struct Script(Mutex<VecDeque<Result<String, AppError>>>, Mutex<u32>);

    impl Script {
        fn new(reads: Vec<Result<&str, AppError>>) -> Self {
            Self(
                Mutex::new(
                    reads
                        .into_iter()
                        .map(|read| read.map(str::to_owned))
                        .collect(),
                ),
                Mutex::new(0),
            )
        }

        fn reads(&self) -> u32 {
            *self.1.lock().unwrap()
        }
    }

    impl CaptionSource for Script {
        fn read(&self) -> Result<String, AppError> {
            *self.1.lock().unwrap() += 1;
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(AppError::Ocr("fim do roteiro".into())))
        }
    }

    #[derive(Default)]
    struct Recorder {
        captions: Mutex<Vec<String>>,
        failures: Mutex<Vec<String>>,
    }

    impl CaptionSink for Recorder {
        fn caption(&self, text: &str) {
            self.captions.lock().unwrap().push(text.to_owned());
        }

        fn failed(&self, error: &AppError) {
            self.failures.lock().unwrap().push(error.code().to_owned());
        }
    }

    const TICK: Duration = Duration::from_millis(1);

    #[test]
    fn sends_the_new_lines_and_stops_after_repeated_failures() {
        let source = Script::new(vec![Ok("Run!"), Ok("Run!"), Ok("Hide"), Ok("Hide")]);
        let sink = Recorder::default();
        let (_keep, stop) = mpsc::channel();

        run(&source, &sink, CaptionGate::default(), TICK, &stop);

        assert_eq!(*sink.captions.lock().unwrap(), vec!["Run!", "Hide"]);
        assert_eq!(*sink.failures.lock().unwrap(), vec!["ocr".to_owned()]);
        assert_eq!(source.reads(), 4 + MAX_CONSECUTIVE_FAILURES);
    }

    #[test]
    fn a_region_without_text_is_not_a_failure() {
        let source = Script::new(vec![
            Err(AppError::NoTextFound),
            Err(AppError::NoTextFound),
            Err(AppError::NoTextFound),
            Err(AppError::NoTextFound),
            Ok("Hi"),
            Ok("Hi"),
        ]);
        let sink = Recorder::default();
        let (_keep, stop) = mpsc::channel();

        run(&source, &sink, CaptionGate::default(), TICK, &stop);

        assert_eq!(*sink.captions.lock().unwrap(), vec!["Hi"]);
        assert_eq!(source.reads(), 6 + MAX_CONSECUTIVE_FAILURES);
    }

    #[test]
    fn a_success_resets_the_failure_count() {
        let failure = || Err(AppError::ScreenCapture("tela".into()));
        let source = Script::new(vec![failure(), failure(), Ok("x"), failure(), failure()]);
        let sink = Recorder::default();
        let (_keep, stop) = mpsc::channel();

        run(&source, &sink, CaptionGate::default(), TICK, &stop);

        assert_eq!(source.reads(), 5 + 1);
    }

    #[test]
    fn a_black_capture_counts_as_a_failure() {
        let blank = || Err(AppError::BlankCapture);
        let source = Script::new(vec![blank(), blank(), blank()]);
        let sink = Recorder::default();
        let (_keep, stop) = mpsc::channel();

        run(&source, &sink, CaptionGate::default(), TICK, &stop);

        assert_eq!(
            *sink.failures.lock().unwrap(),
            vec!["blank_capture".to_owned()]
        );
        assert_eq!(source.reads(), MAX_CONSECUTIVE_FAILURES);
    }

    #[test]
    fn stopping_ends_the_loop_without_reading_and_without_a_failure() {
        let source = Script::new(vec![Ok("never")]);
        let sink = Recorder::default();
        let (sender, stop) = mpsc::channel();
        sender.send(()).unwrap();

        run(
            &source,
            &sink,
            CaptionGate::default(),
            Duration::from_secs(60),
            &stop,
        );

        assert_eq!(source.reads(), 0);
        assert!(sink.failures.lock().unwrap().is_empty());
    }

    #[test]
    fn dropping_the_stop_handle_also_ends_the_loop() {
        let source = Script::new(vec![]);
        let sink = Recorder::default();
        let (sender, stop) = mpsc::channel::<()>();
        drop(sender);

        run(
            &source,
            &sink,
            CaptionGate::default(),
            Duration::from_secs(60),
            &stop,
        );

        assert_eq!(source.reads(), 0);
        assert!(sink.failures.lock().unwrap().is_empty());
    }
}
