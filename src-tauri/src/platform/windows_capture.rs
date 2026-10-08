//! Adaptadores do Windows para a captura de seleção: clipboard, teclado e app em foco.
//!
//! Todo `unsafe` aqui é chamada direta à API Win32; cada bloco explica por que é seguro. A
//! lógica de decisão fica em `services::capture`, testada com fakes.

use std::mem::size_of;
use std::ptr;
use std::thread;
use std::time::Duration;

use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
    HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
};

use super::capture_config;
use crate::error::AppError;
use crate::services::capture::{
    capture_selection, is_capture_blocked, CaptureOutcome, ClipboardFormat, ClipboardPort,
    ClipboardSnapshot, InputPort, Sleeper,
};

const CF_BITMAP: u32 = 2;
const CF_METAFILEPICT: u32 = 3;
const CF_PALETTE: u32 = 9;
const CF_UNICODETEXT: u32 = 13;
const CF_ENHMETAFILE: u32 = 14;
const CF_OWNERDISPLAY: u32 = 0x80;
const CF_DSPMETAFILEPICT: u32 = 0x83;
const CF_DSPENHMETAFILE: u32 = 0x8E;
const CF_DSPBITMAP: u32 = 0x82;
/// Faixas reservadas a dados privados e a objetos GDI, cujos handles não são memória global.
const CF_PRIVATE_AND_GDI_RANGE: std::ops::RangeInclusive<u32> = 0x200..=0x3FF;

const VK_C: VIRTUAL_KEY = VIRTUAL_KEY(0x43);
const KEY_DOWN_BIT: u16 = 0x8000;

/// Tentativas para abrir o clipboard em leituras (~50 ms no total): se estiver ocupado, é
/// melhor desistir da captura do que atrasar o popup.
const OPEN_CLIPBOARD_ATTEMPTS: u32 = 10;
/// Tentativas ao devolver o clipboard do usuário (~500 ms): gerenciadores de clipboard e o
/// histórico do Windows o seguram logo depois de uma mudança, e perder a restauração apagaria
/// o conteúdo dele.
const RESTORE_OPEN_ATTEMPTS: u32 = 100;
const OPEN_CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(5);
const PROCESS_PATH_CAPACITY: usize = 1024;
const CLASS_NAME_CAPACITY: usize = 256;
/// Teto de um formato e do clipboard inteiro: acima disso o snapshot é recusado em vez de
/// duplicar centenas de MB na memória.
const MAX_FORMAT_BYTES: usize = 64 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 128 * 1024 * 1024;

/// Copia a seleção do app em foco, a menos que ele seja um terminal ou desconhecido.
pub fn capture_foreground_selection() -> Result<CaptureOutcome, AppError> {
    let clipboard = WindowsClipboard::new()?;
    capture_selection(&clipboard, &WindowsInput, &ThreadSleeper)
}

// ---------------------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------------------

/// Janela invisível (só de mensagens) que serve de dona do clipboard: sem uma janela dona,
/// `SetClipboardData` falha depois de `EmptyClipboard`.
struct OwnerWindow(HWND);

impl OwnerWindow {
    fn create() -> Result<Self, AppError> {
        // SAFETY: "STATIC" é uma classe do sistema e `HWND_MESSAGE` cria uma janela de
        // mensagens sem interface; nenhum ponteiro de fora do stack é passado.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                None,
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map_err(|error| AppError::Clipboard(error.to_string()))?;
        Ok(Self(window))
    }
}

impl Drop for OwnerWindow {
    fn drop(&mut self) {
        // SAFETY: o handle foi criado por `create` nesta mesma thread e só é destruído aqui.
        // Se falhar não há o que fazer: a janela some junto com o processo.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// Mantém o clipboard aberto enquanto existir e o fecha ao sair do escopo.
struct ClipboardGuard;

impl ClipboardGuard {
    fn open(owner: HWND, attempts: u32) -> Result<Self, AppError> {
        for attempt in 1..=attempts {
            // SAFETY: `owner` é uma janela válida da thread atual; o `Drop` fecha o clipboard.
            if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
                return Ok(Self);
            }
            if attempt < attempts {
                // Outro app pode estar com o clipboard aberto por alguns milissegundos.
                thread::sleep(OPEN_CLIPBOARD_RETRY_DELAY);
            }
        }
        Err(AppError::Clipboard(
            "não foi possível abrir o clipboard".into(),
        ))
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: só existe um guard quando `OpenClipboard` deu certo nesta thread. Um erro ao
        // fechar não é acionável.
        let _ = unsafe { CloseClipboard() };
    }
}

pub struct WindowsClipboard {
    owner: OwnerWindow,
}

impl WindowsClipboard {
    /// Deve ser criado e usado na mesma thread.
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            owner: OwnerWindow::create()?,
        })
    }

    fn open(&self) -> Result<ClipboardGuard, AppError> {
        ClipboardGuard::open(self.owner.0, OPEN_CLIPBOARD_ATTEMPTS)
    }

    fn open_for_restore(&self) -> Result<ClipboardGuard, AppError> {
        ClipboardGuard::open(self.owner.0, RESTORE_OPEN_ATTEMPTS)
    }
}

impl ClipboardPort for WindowsClipboard {
    fn sequence(&self) -> u32 {
        // SAFETY: não recebe argumentos nem exige o clipboard aberto.
        unsafe { GetClipboardSequenceNumber() }
    }

    fn snapshot(&self) -> Result<ClipboardSnapshot, AppError> {
        let _clipboard = self.open()?;
        let mut formats = Vec::new();
        let mut seen_any = false;
        let mut total_bytes = 0usize;
        let mut format = 0;

        loop {
            // SAFETY: o clipboard está aberto por `_clipboard`.
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            seen_any = true;

            match classify(format) {
                FormatKind::Skip => {}
                FormatKind::Unsupported => return Ok(ClipboardSnapshot::Unsupported),
                FormatKind::Copy => match copy_format(format) {
                    Some(data) if !data.is_empty() => {
                        total_bytes = total_bytes.saturating_add(data.len());
                        if total_bytes > MAX_SNAPSHOT_BYTES {
                            return Ok(ClipboardSnapshot::Unsupported);
                        }
                        formats.push(ClipboardFormat { id: format, data });
                    }
                    Some(_) => {}
                    None => return Ok(ClipboardSnapshot::Unsupported),
                },
            }
        }

        Ok(if !seen_any {
            ClipboardSnapshot::Empty
        } else if formats.is_empty() {
            ClipboardSnapshot::Unsupported
        } else {
            ClipboardSnapshot::Formats(formats)
        })
    }

    fn read_text(&self) -> Result<Option<String>, AppError> {
        let _clipboard = self.open()?;
        if is_marked_sensitive() {
            return Ok(None);
        }
        Ok(copy_format(CF_UNICODETEXT).map(|bytes| decode_utf16_text(&bytes)))
    }

    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), AppError> {
        let formats: &[ClipboardFormat] = match snapshot {
            ClipboardSnapshot::Empty => &[],
            ClipboardSnapshot::Formats(formats) => formats,
            ClipboardSnapshot::Unsupported => {
                return Err(AppError::Clipboard("snapshot não restaurável".into()));
            }
        };

        let _clipboard = self.open_for_restore()?;
        // SAFETY: o clipboard está aberto por `_clipboard`.
        unsafe { EmptyClipboard() }.map_err(|error| AppError::Clipboard(error.to_string()))?;

        let mut first_error = None;
        for format in formats {
            if let Err(error) = set_format(format.id, &format.data) {
                first_error.get_or_insert(error);
            }
        }
        if !formats.is_empty() {
            exclude_from_history_and_cloud();
        }

        first_error.map_or(Ok(()), Err)
    }
}

enum FormatKind {
    Copy,
    /// Derivado de outro formato (CF_DIB), recriado pelo sistema sob demanda.
    Skip,
    /// O handle não é memória global; copiar bytes não reproduz o conteúdo.
    Unsupported,
}

fn classify(format: u32) -> FormatKind {
    match format {
        CF_BITMAP | CF_PALETTE => FormatKind::Skip,
        CF_METAFILEPICT | CF_ENHMETAFILE | CF_OWNERDISPLAY | CF_DSPBITMAP | CF_DSPMETAFILEPICT
        | CF_DSPENHMETAFILE => FormatKind::Unsupported,
        id if CF_PRIVATE_AND_GDI_RANGE.contains(&id) => FormatKind::Unsupported,
        _ => FormatKind::Copy,
    }
}

/// Copia os bytes de um formato em memória global. Exige o clipboard aberto.
fn copy_format(format: u32) -> Option<Vec<u8>> {
    // SAFETY: o clipboard está aberto pelo chamador.
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    let global = HGLOBAL(handle.0);

    // SAFETY: `global` é um HGLOBAL devolvido pelo clipboard. O bloqueio dá um ponteiro
    // válido por `GlobalSize` bytes até o `GlobalUnlock`; a cópia termina antes dele.
    unsafe {
        let size = GlobalSize(global);
        if size > MAX_FORMAT_BYTES {
            return None;
        }
        let pointer = GlobalLock(global);
        if pointer.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec();
        // Um erro aqui só indica que o contador de bloqueio chegou a zero.
        let _ = GlobalUnlock(global);
        Some(bytes)
    }
}

/// Entrega os bytes ao clipboard. Em caso de sucesso o sistema passa a ser dono da memória.
fn set_format(format: u32, data: &[u8]) -> Result<(), AppError> {
    let failure = |detail: String| AppError::Clipboard(detail);

    // SAFETY: a memória é alocada e preenchida aqui; só é liberada por nós se o clipboard
    // não a aceitar. O clipboard está aberto pelo chamador.
    unsafe {
        let global =
            GlobalAlloc(GMEM_MOVEABLE, data.len()).map_err(|error| failure(error.to_string()))?;
        let pointer = GlobalLock(global);
        if pointer.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(failure("GlobalLock devolveu nulo".into()));
        }
        ptr::copy_nonoverlapping(data.as_ptr(), pointer.cast::<u8>(), data.len());
        let _ = GlobalUnlock(global);

        match SetClipboardData(format, Some(HANDLE(global.0))) {
            Ok(_) => Ok(()),
            Err(error) => {
                let _ = GlobalFree(Some(global));
                Err(failure(error.to_string()))
            }
        }
    }
}

/// Pede ao Windows que não registre a restauração no histórico (Win+V) nem na nuvem: o item
/// original já está lá. Melhor esforço; falhas são ignoradas porque não afetam o conteúdo.
fn exclude_from_history_and_cloud() {
    let zero = 0u32.to_le_bytes();
    // SAFETY: os nomes são literais UTF-16 terminados em nulo.
    let formats = unsafe {
        [
            RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
            RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
        ]
    };
    for format in formats.into_iter().filter(|&id| id != 0) {
        let _ = set_format(format, &zero);
    }
}

fn decode_utf16_text(bytes: &[u8]) -> String {
    let (pairs, _trailing_byte) = bytes.as_chunks::<2>();
    let units: Vec<u16> = pairs
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

// ---------------------------------------------------------------------------------------
// Teclado
// ---------------------------------------------------------------------------------------

pub struct WindowsInput;

impl InputPort for WindowsInput {
    /// Modificadores e a tecla principal do atalho configurado: segurar a tecla depois de soltar
    /// Ctrl/Alt faria o auto-repeat digitá-la no app de origem.
    fn shortcut_keys_pressed(&self) -> bool {
        [
            VK_CONTROL,
            VK_MENU,
            VK_SHIFT,
            VK_LWIN,
            VK_RWIN,
            VIRTUAL_KEY(capture_config::main_key()),
        ]
        .into_iter()
        .any(is_key_down)
    }

    fn is_target_allowed(&self) -> bool {
        let process_path = foreground_process_path();
        let window_class = foreground_window_class();
        !is_capture_blocked(
            process_path.as_deref(),
            window_class.as_deref(),
            capture_config::allow_editors(),
        )
    }

    fn send_copy(&self) -> Result<(), AppError> {
        let inputs = [
            key_input(VK_CONTROL, false),
            key_input(VK_C, false),
            key_input(VK_C, true),
            key_input(VK_CONTROL, true),
        ];

        // SAFETY: `inputs` é um slice válido de INPUT e o tamanho passado é o de um INPUT.
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            return Ok(());
        }

        // Entrega parcial: garante que nenhuma tecla fique pressionada.
        let release = [key_input(VK_C, true), key_input(VK_CONTROL, true)];
        // SAFETY: igual ao anterior.
        unsafe { SendInput(&release, size_of::<INPUT>() as i32) };
        Err(AppError::Capture(format!(
            "SendInput entregou {sent} de {} eventos",
            inputs.len()
        )))
    }
}

fn is_key_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: consulta de estado sem efeitos colaterais.
    let state = unsafe { GetAsyncKeyState(i32::from(key.0)) };
    (state as u16) & KEY_DOWN_BIT != 0
}

fn key_input(key: VIRTUAL_KEY, release: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if release {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

// ---------------------------------------------------------------------------------------
// App em foco
// ---------------------------------------------------------------------------------------

/// Classe da janela em foco, usada como segunda camada além do nome do executável.
fn foreground_window_class() -> Option<String> {
    // SAFETY: consulta sem argumentos.
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }

    let mut buffer = [0u16; CLASS_NAME_CAPACITY];
    // SAFETY: `buffer` é um slice válido que vive durante a chamada.
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    let length = usize::try_from(length).ok().filter(|&length| length > 0)?;
    Some(String::from_utf16_lossy(&buffer[..length]))
}

/// Conteúdo que o app de origem marcou para ficar fora do histórico e da nuvem do clipboard
/// (gerenciadores de senhas fazem isso). Enviar um segredo ao DeepL não faz sentido, então
/// esse conteúdo nunca é lido. Exige o clipboard aberto.
fn is_marked_sensitive() -> bool {
    // SAFETY: os nomes são literais UTF-16 terminados em nulo.
    let (exclude, can_include) = unsafe {
        (
            RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
            RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
        )
    };

    // SAFETY: o clipboard está aberto pelo chamador; a consulta só lê.
    if exclude != 0 && unsafe { IsClipboardFormatAvailable(exclude) }.is_ok() {
        return true;
    }
    can_include != 0
        && copy_format(can_include).is_some_and(|data| data.len() >= 4 && data[..4] == [0; 4])
}

/// Caminho do executável da janela em foco. `None` se não der para saber, ou se a janela for
/// do próprio app (não se captura de si mesmo).
fn foreground_process_path() -> Option<String> {
    // SAFETY: consulta sem argumentos.
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }

    let mut process_id = 0u32;
    // SAFETY: `process_id` é um u32 válido que vive durante a chamada.
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    // SAFETY: consulta sem argumentos.
    if process_id == 0 || process_id == unsafe { GetCurrentProcessId() } {
        return None;
    }

    // SAFETY: pedimos só direito de consulta limitada; o handle é fechado abaixo.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;

    let mut buffer = [0u16; PROCESS_PATH_CAPACITY];
    let mut length = PROCESS_PATH_CAPACITY as u32;
    // SAFETY: `buffer` tem `length` unidades e ambos vivem durante a chamada.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    // SAFETY: `process` foi aberto acima e não é usado depois.
    let _ = unsafe { CloseHandle(process) };

    queried.ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_copy(format: u32) -> bool {
        matches!(classify(format), FormatKind::Copy)
    }

    #[test]
    fn copies_ordinary_memory_formats() {
        for format in [1, 7, 8, 13, 15, 16, 17, 0xC000, 0xC123] {
            assert!(is_copy(format), "{format:#x}");
        }
    }

    #[test]
    fn skips_bitmap_formats_that_the_system_derives_from_dib() {
        assert!(matches!(classify(CF_BITMAP), FormatKind::Skip));
        assert!(matches!(classify(CF_PALETTE), FormatKind::Skip));
    }

    #[test]
    fn refuses_handle_based_formats_that_cannot_be_copied_as_bytes() {
        for format in [
            CF_METAFILEPICT,
            CF_ENHMETAFILE,
            CF_OWNERDISPLAY,
            CF_DSPBITMAP,
            CF_DSPMETAFILEPICT,
            CF_DSPENHMETAFILE,
            0x200,
            0x2FF,
            0x300,
            0x3FF,
        ] {
            assert!(
                matches!(classify(format), FormatKind::Unsupported),
                "{format:#x}"
            );
        }
    }

    #[test]
    fn decodes_utf16_text_up_to_the_terminating_nul() {
        let bytes: Vec<u8> = "Olá, mundo\0lixo"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();

        assert_eq!(decode_utf16_text(&bytes), "Olá, mundo");
    }

    /// Escreve no clipboard real do usuário e o devolve ao final. Rodar manualmente:
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore = "usa o clipboard real do sistema operacional"]
    fn real_clipboard_round_trips_text_and_custom_formats() {
        let clipboard = WindowsClipboard::new().unwrap();
        let original = clipboard.snapshot().unwrap();
        if original == ClipboardSnapshot::Unsupported {
            return;
        }

        // SAFETY: nome literal terminado em nulo.
        let custom = unsafe { RegisterClipboardFormatW(w!("lang-app-test-format")) };
        let sample = ClipboardSnapshot::Formats(vec![
            ClipboardFormat {
                id: CF_UNICODETEXT,
                data: "Olá, teste\0"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            },
            ClipboardFormat {
                id: custom,
                data: vec![1, 2, 3, 4],
            },
        ]);

        let before = clipboard.sequence();
        clipboard.restore(&sample).unwrap();
        let changed = clipboard.sequence() != before;
        let read_back = clipboard.read_text().unwrap();
        let after = clipboard.snapshot().unwrap();
        clipboard.restore(&original).unwrap();

        assert!(changed);
        // A restauração marca o conteúdo como fora do histórico; por isso ele conta como
        // sensível e `read_text` se recusa a lê-lo.
        assert_eq!(read_back, None);
        let ClipboardSnapshot::Formats(formats) = after else {
            panic!("esperava formatos no clipboard");
        };
        assert!(formats
            .iter()
            .any(|format| format.id == CF_UNICODETEXT
                && decode_utf16_text(&format.data) == "Olá, teste"));
        assert!(formats
            .iter()
            .any(|format| format.id == custom && format.data == [1, 2, 3, 4]));
    }
}
