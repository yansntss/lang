//! Falhas que acontecem antes de haver uma janela, ou que derrubam o app de vez.
//!
//! No release o app não tem console (`windows_subsystem = "windows"`), então um erro de
//! inicialização escrito em `stderr` não aparece em lugar nenhum. Aqui ele vira uma caixa de
//! mensagem e um arquivo de texto.

use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Mesmo valor de `identifier` em `tauri.conf.json` (há um teste que confere): é o nome da
/// pasta de dados e de logs do app.
const APP_IDENTIFIER: &str = "com.yansa.lang-app";
const ERROR_FILE: &str = "startup-error.log";
const DIALOG_TITLE: &str = "Tradutor";

/// Texto da caixa de erro. O detalhe técnico não entra aqui: fica no arquivo indicado.
pub fn startup_failure_message(error_file: Option<&Path>) -> String {
    let headline = "O Tradutor não conseguiu iniciar.";
    let advice = "Se o problema continuar, reinstale o aplicativo.";
    match error_file {
        Some(path) => format!(
            "{headline}\n\nOs detalhes foram gravados em:\n{}\n\n{advice}",
            path.display()
        ),
        None => format!("{headline}\n\n{advice}"),
    }
}

/// Acrescenta uma linha `[segundos desde 1970] detalhe` ao arquivo de erros de inicialização
/// em `directory` e devolve o caminho dele.
pub fn write_startup_error(directory: &Path, detail: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(ERROR_FILE);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "[{seconds}] {detail}")?;
    Ok(path)
}

fn data_directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join(APP_IDENTIFIER))
}

/// Registra a falha de inicialização em arquivo e avisa o usuário numa caixa de mensagem.
pub fn report_startup_failure(error: &dyn Display) {
    let detail = error.to_string();
    let error_file = data_directory().and_then(|directory| {
        write_startup_error(&directory, &detail)
            .inspect_err(|io_error| eprintln!("não foi possível gravar o erro: {io_error}"))
            .ok()
    });
    show_dialog(&startup_failure_message(error_file.as_deref()));
}

#[cfg(windows)]
fn show_dialog(message: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    // SAFETY: os dois `HSTRING` vivem até o fim da chamada, que é síncrona e bloqueia até o
    // usuário fechar a caixa; sem janela dona (`None`), não há outro ponteiro envolvido.
    let _ = unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from(DIALOG_TITLE),
            MB_OK | MB_ICONERROR,
        )
    };
}

#[cfg(not(windows))]
fn show_dialog(message: &str) {
    eprintln!("{DIALOG_TITLE}: {message}");
}

/// Linha de log de um pânico. Só o local: a mensagem do pânico pode conter o texto que o usuário
/// estava traduzindo (por exemplo, ao fatiar uma string), e o log não deve guardar esse texto.
pub fn panic_summary(file: Option<&str>, line: Option<u32>) -> String {
    match (file, line) {
        (Some(file), Some(line)) => format!("pânico em {file}:{line} (mensagem omitida)"),
        _ => "pânico em local desconhecido (mensagem omitida)".to_owned(),
    }
}

/// Com `panic = "abort"` o processo termina logo após o pânico: registra o local e descarrega o
/// log antes disso, para o motivo do fechamento não se perder.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
        let location = info.location();
        log::error!(
            "{}",
            panic_summary(
                location.map(|place| place.file()),
                location.map(|place| place.line())
            )
        );
        log::logger().flush();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_points_to_the_error_file_without_repeating_the_error() {
        let path = Path::new(r"C:\Dados\startup-error.log");

        let message = startup_failure_message(Some(path));

        assert!(message.contains("O Tradutor não conseguiu iniciar."));
        assert!(message.contains(r"C:\Dados\startup-error.log"));
        assert!(message.contains("reinstale"));
    }

    #[test]
    fn the_message_still_works_when_the_file_could_not_be_written() {
        let message = startup_failure_message(None);

        assert!(message.contains("não conseguiu iniciar"));
        assert!(!message.contains("gravados em"));
    }

    #[test]
    fn startup_errors_are_appended_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("novo").join("dados");

        let first = write_startup_error(&folder, "primeiro erro").unwrap();
        let second = write_startup_error(&folder, "segundo erro").unwrap();

        assert_eq!(first, second);
        let content = fs::read_to_string(&first).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with('[') && lines[0].ends_with("] primeiro erro"));
        assert!(lines[1].ends_with("] segundo erro"));
    }

    #[test]
    fn a_failure_to_write_the_error_is_reported_not_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("arquivo");
        fs::write(&blocker, "x").unwrap();

        assert!(write_startup_error(&blocker.join("dados"), "erro").is_err());
    }

    #[test]
    fn a_panic_is_logged_with_its_location_only() {
        assert_eq!(
            panic_summary(Some("src/lib.rs"), Some(42)),
            "pânico em src/lib.rs:42 (mensagem omitida)"
        );
        assert_eq!(
            panic_summary(None, None),
            "pânico em local desconhecido (mensagem omitida)"
        );
    }

    #[test]
    fn the_data_folder_name_matches_the_app_identifier() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();

        assert_eq!(config["identifier"], APP_IDENTIFIER);
    }
}
