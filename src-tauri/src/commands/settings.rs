use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::error::AppError;
use crate::platform::{accelerator, capture_config, shortcut, tray};
use crate::secrets::SecretKind;
use crate::services::deepl::Usage;
use crate::settings::{PreferencesUpdate, Settings};
use crate::state::AppState;

/// O que a janela de configurações mostra: o salvo mais o que só o sistema sabe.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    settings: Settings,
    /// Lido do sistema (entrada de inicialização do Windows), não do arquivo.
    autostart: bool,
}

/// Resultado de "Testar chave".
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCheck {
    /// Consumo do mês, quando o provedor informa (DeepL).
    usage: Option<Usage>,
}

fn view(app: &AppHandle, settings: Settings) -> SettingsView {
    let autostart = app.autolaunch().is_enabled().unwrap_or_else(|error| {
        log::warn!("não foi possível ler o início com o Windows: {error}");
        false
    });
    SettingsView {
        settings,
        autostart,
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<'_, AppState>) -> SettingsView {
    view(&app, state.settings.get())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    preferences: PreferencesUpdate,
) -> Result<SettingsView, AppError> {
    let settings = state.settings.update_preferences(preferences.into())?;
    capture_config::set_allow_editors(settings.preferences.capture_in_editors);
    Ok(view(&app, settings))
}

/// Troca o atalho global. Se o novo não puder ser registrado, o atual continua valendo.
#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    accelerator: String,
) -> Result<SettingsView, AppError> {
    let new = accelerator::parse(&accelerator)?;
    let settings = shortcut::apply_shortcut(&app, &state.settings, &new)?;
    capture_config::set_main_key(new.virtual_key);
    tray::update_tooltip(&app, &new.label, true);
    Ok(view(&app, settings))
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), AppError> {
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|error| AppError::Settings(error.to_string()))
}

/// Confere a chave sem gastar a cota: o DeepL informa o consumo do mês e a Anthropic lista os
/// modelos.
#[tauri::command]
pub async fn test_secret(
    state: State<'_, AppState>,
    kind: SecretKind,
) -> Result<KeyCheck, AppError> {
    match kind {
        SecretKind::Deepl => {
            let usage = state.translations.translator().check_key().await?;
            Ok(KeyCheck { usage: Some(usage) })
        }
        SecretKind::Anthropic => {
            state.explanations.explainer().check_key().await?;
            Ok(KeyCheck { usage: None })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_flattens_the_settings_next_to_the_autostart_flag() {
        let json = serde_json::to_value(SettingsView {
            settings: Settings::default(),
            autostart: true,
        })
        .unwrap();

        assert_eq!(json["shortcut"], "Ctrl+Alt+T");
        assert_eq!(json["theme"], "system");
        assert_eq!(json["autostart"], true);
        assert_eq!(json["autoTranslateSelection"], true);
    }

    #[test]
    fn a_key_check_reports_usage_only_when_known() {
        let with_usage = serde_json::to_value(KeyCheck {
            usage: Some(Usage {
                used: 10,
                limit: 500_000,
            }),
        })
        .unwrap();
        let without = serde_json::to_value(KeyCheck { usage: None }).unwrap();

        assert_eq!(
            with_usage,
            serde_json::json!({ "usage": { "used": 10, "limit": 500000 } })
        );
        assert_eq!(without, serde_json::json!({ "usage": null }));
    }
}
