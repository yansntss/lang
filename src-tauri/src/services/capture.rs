use std::time::Duration;

use crate::error::AppError;

/// Quanto esperar o usuário soltar as teclas do atalho antes de simular o copiar.
pub const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(600);
/// Quanto esperar o app de origem colocar a seleção no clipboard.
pub const COPY_TIMEOUT: Duration = Duration::from_millis(500);
pub const POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Leituras do contador, no máximo, até ele parar de mudar depois da cópia (apps que gravam
/// vários formatos em sequência).
const SETTLE_MAX_POLLS: u32 = 20;
/// Teto do texto levado ao popup, para uma seleção gigante não atravessar o IPC. Fica acima do
/// limite de tradução (5000) de propósito: o popup mostra o erro "excede o limite" em vez de
/// truncar a tradução em silêncio.
pub const MAX_PREFILL_CHARS: usize = 20_000;

/// Um formato do clipboard com seus bytes brutos (texto, HTML, RTF, imagem, lista de arquivos…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardFormat {
    pub id: u32,
    pub data: Vec<u8>,
}

/// Conteúdo do clipboard antes da captura, para poder devolvê-lo depois sem perder formatação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardSnapshot {
    Empty,
    Formats(Vec<ClipboardFormat>),
    /// Algum formato não pode ser copiado (handle gráfico, tamanho absurdo…): restaurar
    /// perderia dados, então a captura é recusada.
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// O usuário não soltou as teclas do atalho a tempo.
    ModifiersHeld,
    /// Capturar destruiria o conteúdo atual do clipboard.
    UnsupportedClipboard,
    /// O app em foco é um terminal (ou desconhecido): Ctrl+C ali interrompe processos.
    BlockedApplication,
    /// Outro app escreveu no clipboard durante a captura: o conteúdo dele é mais novo que o
    /// nosso snapshot e não pode ser sobrescrito.
    ClipboardChanged,
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
    /// `None` quando o clipboard não tem texto (ou o app de origem o marcou como sensível).
    fn read_text(&self) -> Result<Option<String>, AppError>;
    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), AppError>;
}

pub trait InputPort {
    /// Alguma tecla do atalho (modificadores ou a tecla principal) ainda está pressionada.
    fn shortcut_keys_pressed(&self) -> bool;
    /// O app em foco aceita a cópia simulada. Consultado de novo logo antes de enviá-la,
    /// porque o foco pode mudar enquanto esperamos.
    fn is_target_allowed(&self) -> bool;
    /// Simula Ctrl+C no app em foco. As teclas devem sempre terminar soltas.
    fn send_copy(&self) -> Result<(), AppError>;
}

pub trait Sleeper {
    fn sleep(&self, duration: Duration);
}

/// Copia a seleção do app em foco e devolve o clipboard como estava.
///
/// O clipboard só é alterado pelo app de origem, depois do `Ctrl+C`; a partir daí o snapshot é
/// devolvido, a menos que alguém tenha escrito algo mais novo (o contador de sequência muda), caso
/// em que nada é restaurado para não apagar o conteúdo dele.
pub fn capture_selection(
    clipboard: &dyn ClipboardPort,
    input: &dyn InputPort,
    sleeper: &dyn Sleeper,
) -> Result<CaptureOutcome, AppError> {
    // Com as teclas do atalho ainda pressionadas, o "C" simulado viraria Ctrl+Alt+C, e a tecla
    // principal seguraria auto-repeat digitando no app de origem.
    if !wait_until(sleeper, MODIFIER_RELEASE_TIMEOUT, || {
        !input.shortcut_keys_pressed()
    }) {
        return Ok(CaptureOutcome::Skipped(SkipReason::ModifiersHeld));
    }
    if !input.is_target_allowed() {
        return Ok(CaptureOutcome::Skipped(SkipReason::BlockedApplication));
    }

    // O contador é lido antes do snapshot: se mudar durante a leitura, o snapshot pode já
    // estar velho e restaurá-lo apagaria conteúdo novo.
    let sequence_before = clipboard.sequence();
    let snapshot = clipboard.snapshot()?;
    if snapshot == ClipboardSnapshot::Unsupported {
        return Ok(CaptureOutcome::Skipped(SkipReason::UnsupportedClipboard));
    }
    if clipboard.sequence() != sequence_before {
        return Ok(CaptureOutcome::Skipped(SkipReason::ClipboardChanged));
    }

    // O foco pode ter mudado durante a espera pelas teclas ou o snapshot.
    if !input.is_target_allowed() {
        return Ok(CaptureOutcome::Skipped(SkipReason::BlockedApplication));
    }
    input.send_copy()?;

    if !wait_until(sleeper, COPY_TIMEOUT, || {
        clipboard.sequence() != sequence_before
    }) {
        return Ok(CaptureOutcome::NoSelection);
    }
    let settled = settle(clipboard, sleeper);

    let copied = clipboard.read_text();
    if clipboard.sequence() != settled {
        // Alguém escreveu depois da cópia: o texto lido pode não ser a seleção, e o clipboard
        // atual já é mais novo que o snapshot.
        return Ok(CaptureOutcome::Skipped(SkipReason::ClipboardChanged));
    }
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

/// Espera o contador parar de mudar: alguns apps gravam vários formatos em passos, cada um
/// incrementando o contador. Devolve o último valor visto.
fn settle(clipboard: &dyn ClipboardPort, sleeper: &dyn Sleeper) -> u32 {
    let mut last = clipboard.sequence();
    for _ in 0..SETTLE_MAX_POLLS {
        sleeper.sleep(POLL_INTERVAL);
        let current = clipboard.sequence();
        if current == last {
            return current;
        }
        last = current;
    }
    last
}

fn normalize(text: &str) -> String {
    text.trim().chars().take(MAX_PREFILL_CHARS).collect()
}

/// Decide se a captura é proibida para o app em foco.
///
/// Terminais tratam Ctrl+C como interrupção (SIGINT): simulá-lo ali poderia encerrar um
/// processo em execução do usuário. O mesmo vale para apps com terminal embutido (o terminal do
/// VS Code ou de IDEs JetBrains é indistinguível do editor pela janela) e para consoles remotos
/// e de máquinas virtuais, onde o Ctrl+C vai para o sistema convidado.
///
/// É uma lista de bloqueio, então nunca será exaustiva; por isso um app desconhecido
/// (`process_path` ausente) também é bloqueado.
///
/// Os editores (`EDITOR_PROCESSES`) só deixam de ser bloqueados se o usuário os liberou
/// (`allow_editors`); terminais e consoles remotos são bloqueados sempre.
pub fn is_capture_blocked(
    process_path: Option<&str>,
    window_class: Option<&str>,
    allow_editors: bool,
) -> bool {
    let Some(path) = process_path else {
        return true;
    };

    let file_name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if file_name.is_empty() || BLOCKED_PROCESSES.contains(&file_name.as_str()) {
        return true;
    }
    if !allow_editors && EDITOR_PROCESSES.contains(&file_name.as_str()) {
        return true;
    }

    window_class.is_some_and(|class| {
        BLOCKED_WINDOW_CLASSES
            .iter()
            .any(|blocked| blocked.eq_ignore_ascii_case(class.trim()))
    })
}

/// Nomes de arquivo em minúsculas.
const BLOCKED_PROCESSES: &[&str] = &[
    // Terminais e shells.
    "windowsterminal.exe",
    "wt.exe",
    "openconsole.exe",
    "cmd.exe",
    "powershell.exe",
    "powershell_ise.exe",
    "pwsh.exe",
    "conhost.exe",
    "wsl.exe",
    "wslhost.exe",
    "bash.exe",
    "sh.exe",
    "ssh.exe",
    "git-bash.exe",
    "mintty.exe",
    "alacritty.exe",
    "wezterm.exe",
    "wezterm-gui.exe",
    "hyper.exe",
    "tabby.exe",
    "rxvt.exe",
    "conemu.exe",
    "conemu64.exe",
    "cmder.exe",
    // Clientes SSH/serial.
    "putty.exe",
    "kitty.exe",
    "kitty_portable.exe",
    "mobaxterm.exe",
    "termius.exe",
    "securecrt.exe",
    "xshell.exe",
    "ttermpro.exe",
    // Área de trabalho remota e máquinas virtuais.
    "mstsc.exe",
    "msrdc.exe",
    "vmconnect.exe",
    "vmware.exe",
    "vmplayer.exe",
    "virtualboxvm.exe",
    "vboxsdl.exe",
    "anydesk.exe",
    "teamviewer.exe",
    "parsecd.exe",
    "wfica32.exe",
];

/// Editores e IDEs com terminal embutido: o terminal é indistinguível do editor pela janela, e
/// ali o Ctrl+C simulado interrompe o processo em execução. Bloqueados por padrão; o usuário
/// pode liberá-los nas configurações. Nomes de arquivo em minúsculas.
const EDITOR_PROCESSES: &[&str] = &[
    "code.exe",
    "code - insiders.exe",
    "cursor.exe",
    "windsurf.exe",
    "vscodium.exe",
    "positron.exe",
    "zed.exe",
    "fleet.exe",
    "antigravity.exe",
    "devenv.exe",
    "idea64.exe",
    "pycharm64.exe",
    "webstorm64.exe",
    "phpstorm64.exe",
    "rider64.exe",
    "clion64.exe",
    "goland64.exe",
    "rubymine64.exe",
    "datagrip64.exe",
    "studio64.exe",
];

/// Classes de janela de consoles e terminais, para pegar hospedeiros que o nome do arquivo
/// não revela.
const BLOCKED_WINDOW_CLASSES: &[&str] = &[
    "ConsoleWindowClass",
    "CASCADIA_HOSTING_WINDOW_CLASS",
    "mintty",
    "PuTTY",
    "VirtualConsoleClass",
    "TMobaXtermForm",
];

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    use super::*;

    /// Simula clipboard e teclado de um desktop. A cópia pedida por `send_copy` só aparece no
    /// clipboard depois de `copy_latency_polls` consultas ao contador, como num app lento.
    struct FakeDesktop {
        clipboard: Mutex<ClipboardSnapshot>,
        sequence: AtomicU32,
        selection: Option<String>,
        shortcut_polls_pressed: AtomicU32,
        copy_latency_polls: u32,
        pending_copy_polls: Mutex<Option<u32>>,
        /// Quantas mudanças extras do contador seguem a cópia (app que grava vários formatos).
        settle_bumps: u32,
        settle_remaining: AtomicU32,
        /// Respostas sucessivas de `is_target_allowed`; vazio significa "permitido".
        target_allowed: Mutex<VecDeque<bool>>,
        copies_sent: AtomicU32,
        restores: AtomicU32,
        fail_read: bool,
        fail_restore: bool,
        fail_send: bool,
        unsupported_after_copy: bool,
        external_write_on_snapshot: bool,
        external_write_on_read: bool,
    }

    impl FakeDesktop {
        fn with_clipboard(content: ClipboardSnapshot) -> Self {
            Self {
                clipboard: Mutex::new(content),
                sequence: AtomicU32::new(1),
                selection: None,
                shortcut_polls_pressed: AtomicU32::new(0),
                copy_latency_polls: 0,
                pending_copy_polls: Mutex::new(None),
                settle_bumps: 0,
                settle_remaining: AtomicU32::new(0),
                target_allowed: Mutex::new(VecDeque::new()),
                copies_sent: AtomicU32::new(0),
                restores: AtomicU32::new(0),
                fail_read: false,
                fail_restore: false,
                fail_send: false,
                unsupported_after_copy: false,
                external_write_on_snapshot: false,
                external_write_on_read: false,
            }
        }

        fn selecting(mut self, text: &str) -> Self {
            self.selection = Some(text.to_owned());
            self
        }

        fn allowing(self, answers: &[bool]) -> Self {
            *self.target_allowed.lock().unwrap() = answers.iter().copied().collect();
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
                        *self.clipboard.lock().unwrap() = text(selection);
                        self.sequence.fetch_add(1, Ordering::SeqCst);
                        self.settle_remaining
                            .store(self.settle_bumps, Ordering::SeqCst);
                    }
                } else {
                    *pending = Some(polls + 1);
                }
            } else if self.settle_remaining.load(Ordering::SeqCst) > 0 {
                self.settle_remaining.fetch_sub(1, Ordering::SeqCst);
                self.sequence.fetch_add(1, Ordering::SeqCst);
            }
            self.sequence.load(Ordering::SeqCst)
        }

        fn snapshot(&self) -> Result<ClipboardSnapshot, AppError> {
            let content = self.current();
            if self.external_write_on_snapshot {
                self.sequence.fetch_add(1, Ordering::SeqCst);
            }
            Ok(content)
        }

        fn read_text(&self) -> Result<Option<String>, AppError> {
            if self.external_write_on_read {
                self.sequence.fetch_add(1, Ordering::SeqCst);
            }
            if self.fail_read {
                return Err(AppError::Clipboard("leitura falhou".into()));
            }
            if self.unsupported_after_copy {
                return Ok(None);
            }
            let ClipboardSnapshot::Formats(formats) = self.current() else {
                return Ok(None);
            };
            Ok(formats
                .iter()
                .find(|format| format.id == TEXT_FORMAT)
                .map(|format| String::from_utf8_lossy(&format.data).into_owned()))
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
        fn shortcut_keys_pressed(&self) -> bool {
            let remaining = self.shortcut_polls_pressed.load(Ordering::SeqCst);
            if remaining == u32::MAX {
                return true;
            }
            if remaining > 0 {
                self.shortcut_polls_pressed
                    .store(remaining - 1, Ordering::SeqCst);
                return true;
            }
            false
        }

        fn is_target_allowed(&self) -> bool {
            self.target_allowed
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(true)
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

    /// Identificador de formato de texto usado só pelo fake destes testes.
    const TEXT_FORMAT: u32 = 13;
    const HTML_FORMAT: u32 = 49_300;
    const IMAGE_FORMAT: u32 = 8;

    fn format(id: u32, data: &[u8]) -> ClipboardFormat {
        ClipboardFormat {
            id,
            data: data.to_vec(),
        }
    }

    /// Com a configuração de fábrica: editores bloqueados.
    fn blocked(process_path: Option<&str>, window_class: Option<&str>) -> bool {
        is_capture_blocked(process_path, window_class, false)
    }

    fn text(content: &str) -> ClipboardSnapshot {
        ClipboardSnapshot::Formats(vec![format(TEXT_FORMAT, content.as_bytes())])
    }

    fn captured(content: &str) -> CaptureOutcome {
        CaptureOutcome::Captured {
            text: content.to_owned(),
            restore_failed: false,
        }
    }

    #[test]
    fn blocks_terminals_where_ctrl_c_interrupts_the_running_process() {
        for terminal in [
            "WindowsTerminal.exe",
            "OpenConsole.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "conhost.exe",
            "wsl.exe",
            "bash.exe",
            "git-bash.exe",
            "mintty.exe",
            "alacritty.exe",
            "wezterm.exe",
            "wezterm-gui.exe",
            "ConEmu64.exe",
            "putty.exe",
            "kitty.exe",
            "MobaXterm.exe",
            "Termius.exe",
        ] {
            assert!(blocked(Some(terminal), None), "{terminal}");
        }
    }

    #[test]
    fn blocks_editors_whose_integrated_terminal_receives_ctrl_c_as_interrupt() {
        for editor in [
            "Code.exe",
            "Code - Insiders.exe",
            "Cursor.exe",
            "devenv.exe",
            "idea64.exe",
            "pycharm64.exe",
        ] {
            assert!(blocked(Some(editor), None), "{editor}");
        }
    }

    #[test]
    fn allows_editors_only_when_the_user_released_them() {
        for editor in ["Code.exe", "Cursor.exe", "idea64.exe", r"C:\VS\devenv.exe"] {
            assert!(is_capture_blocked(Some(editor), None, false), "{editor}");
            assert!(!is_capture_blocked(Some(editor), None, true), "{editor}");
        }
    }

    #[test]
    fn releasing_editors_never_releases_terminals_or_remote_consoles() {
        for process in [
            "WindowsTerminal.exe",
            "cmd.exe",
            "pwsh.exe",
            "putty.exe",
            "mstsc.exe",
            "VirtualBoxVM.exe",
        ] {
            assert!(is_capture_blocked(Some(process), None, true), "{process}");
        }
    }

    #[test]
    fn releasing_editors_does_not_release_console_window_classes() {
        assert!(is_capture_blocked(
            Some("code.exe"),
            Some("ConsoleWindowClass"),
            true
        ));
    }

    #[test]
    fn an_unknown_foreground_process_stays_blocked_even_with_editors_released() {
        assert!(is_capture_blocked(None, None, true));
    }

    #[test]
    fn blocks_remote_desktop_and_virtual_machine_consoles() {
        for console in [
            "mstsc.exe",
            "vmconnect.exe",
            "VirtualBoxVM.exe",
            "AnyDesk.exe",
        ] {
            assert!(blocked(Some(console), None), "{console}");
        }
    }

    #[test]
    fn blocks_known_console_window_classes_even_for_unknown_executables() {
        for class in [
            "ConsoleWindowClass",
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "mintty",
            "PuTTY",
            "VirtualConsoleClass",
            "consolewindowclass",
        ] {
            assert!(
                blocked(Some(r"C:\Tools\algum-host.exe"), Some(class)),
                "{class}"
            );
        }
    }

    #[test]
    fn blocks_regardless_of_case_and_full_path() {
        assert!(blocked(Some(r"C:\Windows\System32\CMD.EXE"), None));
        assert!(blocked(
            Some("C:/Program Files/PowerShell/7/pwsh.exe"),
            None
        ));
    }

    #[test]
    fn allows_regular_applications() {
        for app in [
            "notepad.exe",
            "chrome.exe",
            "msedge.exe",
            "WINWORD.EXE",
            "firefox.exe",
        ] {
            assert!(!blocked(Some(app), Some("Chrome_WidgetWin_1")), "{app}");
        }
    }

    #[test]
    fn matches_the_whole_file_name_not_a_fragment() {
        assert!(!blocked(Some(r"C:\Tools\mycmd.exe"), None));
        assert!(!blocked(Some(r"C:\Tools\cmder-notes.exe"), None));
    }

    #[test]
    fn blocks_when_the_foreground_process_is_unknown() {
        assert!(blocked(None, None));
        assert!(blocked(Some(""), None));
        assert!(blocked(Some("   "), None));
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
        let image = ClipboardSnapshot::Formats(vec![format(IMAGE_FORMAT, &[255, 0, 0, 255])]);
        let desktop = FakeDesktop::with_clipboard(image.clone()).selecting("texto");

        run(&desktop).unwrap();

        assert_eq!(desktop.current(), image);
    }

    #[test]
    fn restores_every_format_of_a_rich_clipboard_without_losing_formatting() {
        let rich = ClipboardSnapshot::Formats(vec![
            format(TEXT_FORMAT, b"antes"),
            format(HTML_FORMAT, b"<b>antes</b>"),
        ]);
        let desktop = FakeDesktop::with_clipboard(rich.clone()).selecting("texto");

        run(&desktop).unwrap();

        assert_eq!(desktop.current(), rich);
    }

    #[test]
    fn restores_an_empty_clipboard() {
        let desktop = FakeDesktop::with_clipboard(ClipboardSnapshot::Empty).selecting("texto");

        run(&desktop).unwrap();

        assert_eq!(desktop.current(), ClipboardSnapshot::Empty);
    }

    #[test]
    fn waits_for_the_shortcut_keys_to_be_released_before_copying() {
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.shortcut_polls_pressed.store(5, Ordering::SeqCst);
        let sleeper = CountingSleeper::default();

        let outcome = capture_selection(&desktop, &desktop, &sleeper).unwrap();

        assert_eq!(outcome, captured("texto"));
        assert!(sleeper.sleeps.load(Ordering::SeqCst) >= 5);
        assert_eq!(desktop.copies_sent(), 1);
    }

    #[test]
    fn skips_without_copying_when_the_shortcut_keys_are_never_released() {
        let desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop
            .shortcut_polls_pressed
            .store(u32::MAX, Ordering::SeqCst);

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, CaptureOutcome::Skipped(SkipReason::ModifiersHeld));
        assert_eq!(desktop.copies_sent(), 0);
    }

    #[test]
    fn skips_without_copying_when_the_foreground_app_is_not_allowed() {
        let desktop = FakeDesktop::with_clipboard(text("antes"))
            .selecting("texto")
            .allowing(&[false]);

        let outcome = run(&desktop).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::Skipped(SkipReason::BlockedApplication)
        );
        assert_eq!(desktop.copies_sent(), 0);
        assert_eq!(desktop.restores(), 0);
    }

    #[test]
    fn skips_when_the_focus_moves_to_a_blocked_app_right_before_copying() {
        let desktop = FakeDesktop::with_clipboard(text("antes"))
            .selecting("texto")
            .allowing(&[true, false]);

        let outcome = run(&desktop).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::Skipped(SkipReason::BlockedApplication)
        );
        assert_eq!(desktop.copies_sent(), 0);
        assert_eq!(desktop.current(), text("antes"));
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
    fn skips_when_another_app_writes_the_clipboard_while_it_is_being_saved() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.external_write_on_snapshot = true;

        let outcome = run(&desktop).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::Skipped(SkipReason::ClipboardChanged)
        );
        assert_eq!(desktop.copies_sent(), 0);
        assert_eq!(desktop.restores(), 0);
    }

    #[test]
    fn does_not_restore_over_content_written_by_another_app_after_the_copy() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("texto");
        desktop.external_write_on_read = true;

        let outcome = run(&desktop).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::Skipped(SkipReason::ClipboardChanged)
        );
        assert_eq!(desktop.restores(), 0);
    }

    #[test]
    fn waits_for_the_sequence_to_settle_when_the_source_app_writes_in_steps() {
        let mut desktop = FakeDesktop::with_clipboard(text("antes")).selecting("em partes");
        desktop.settle_bumps = 3;

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, captured("em partes"));
        assert_eq!(desktop.current(), text("antes"));
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
    fn keeps_windows_line_breaks_inside_the_selection() {
        let desktop =
            FakeDesktop::with_clipboard(text("antes")).selecting("linha 1\r\nlinha 2\r\n");

        let outcome = run(&desktop).unwrap();

        assert_eq!(outcome, captured("linha 1\r\nlinha 2"));
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
