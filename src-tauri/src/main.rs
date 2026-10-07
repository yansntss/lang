// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(err) = lang_app_lib::run() {
        eprintln!("erro ao iniciar o aplicativo: {err}");
        std::process::exit(1);
    }
}
