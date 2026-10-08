/// Comandos expostos ao front. Cada um vira uma permissão `allow-<nome>` que as
/// capabilities em `capabilities/` concedem janela a janela.
const APP_COMMANDS: &[&str] = &[
    "translate",
    "copy_to_clipboard",
    "explain",
    "cancel_explain",
    "hide_popup",
    "set_secret",
    "has_secret",
    "delete_secret",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS));

    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("falha no build do Tauri: {error:#}");
        std::process::exit(1);
    }
}
