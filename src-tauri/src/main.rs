// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = lang_app_lib::run() {
        // Sem console no release: o erro vira uma caixa de mensagem e um arquivo.
        lang_app_lib::fatal::report_startup_failure(&error);
        std::process::exit(1);
    }
}
