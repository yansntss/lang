//! Preferências do usuário, guardadas em `settings.json` na pasta de dados do app.
//! Segredos (chaves de API) nunca passam por aqui: ficam no keychain.

mod store;

use serde::{Deserialize, Serialize};

pub use store::SettingsStore;

use crate::services::translation::{TargetLang, TargetLangs};

/// Atalho de fábrica; também o que vale quando o salvo é inválido.
pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+T";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Variante do inglês usada ao traduzir para o inglês.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EnglishVariant {
    #[default]
    #[serde(rename = "EN-US")]
    EnUs,
    #[serde(rename = "EN-GB")]
    EnGb,
}

/// Variante do português usada ao traduzir do inglês.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PortugueseVariant {
    #[default]
    #[serde(rename = "PT-BR")]
    PtBr,
    #[serde(rename = "PT-PT")]
    PtPt,
}

/// Modelos que o "Explicar" aceita. Lista fechada: nada vindo do front vira ID de modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ExplainModel {
    #[default]
    #[serde(rename = "claude-haiku-5-5")]
    Haiku,
    #[serde(rename = "claude-sonnet-5-5")]
    Sonnet,
}

impl ExplainModel {
    /// ID enviado à API da Anthropic.
    pub fn id(self) -> &'static str {
        match self {
            Self::Haiku => "claude-haiku-5-5",
            Self::Sonnet => "claude-sonnet-5-5",
        }
    }
}

/// Preferências que a janela de configurações altera de uma vez (`update_settings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub theme: Theme,
    pub english_variant: EnglishVariant,
    pub portuguese_variant: PortugueseVariant,
    pub explain_model: ExplainModel,
    /// Liberar a captura em VS Code, Cursor e IDEs JetBrains. Desligado: o terminal embutido
    /// desses apps trata o Ctrl+C simulado como interrupção.
    pub capture_in_editors: bool,
    /// Traduzir a seleção capturada assim que o popup abre. Desligado: só após o Enter.
    pub auto_translate_selection: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            english_variant: EnglishVariant::default(),
            portuguese_variant: PortugueseVariant::default(),
            explain_model: ExplainModel::default(),
            capture_in_editors: false,
            auto_translate_selection: true,
        }
    }
}

impl Preferences {
    pub fn target_langs(&self) -> TargetLangs {
        TargetLangs {
            to_portuguese: match self.portuguese_variant {
                PortugueseVariant::PtBr => TargetLang::PtBr,
                PortugueseVariant::PtPt => TargetLang::PtPt,
            },
            to_english: match self.english_variant {
                EnglishVariant::EnUs => TargetLang::EnUs,
                EnglishVariant::EnGb => TargetLang::EnGb,
            },
        }
    }
}

/// O que `update_settings` recebe do front. Ao contrário de [`Preferences`] (que aceita campos
/// ausentes para ler arquivos de versões antigas), aqui todos os campos são obrigatórios e
/// nenhum desconhecido é aceito: um payload incompleto não pode zerar preferências em silêncio.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferencesUpdate {
    theme: Theme,
    english_variant: EnglishVariant,
    portuguese_variant: PortugueseVariant,
    explain_model: ExplainModel,
    capture_in_editors: bool,
    auto_translate_selection: bool,
}

impl From<PreferencesUpdate> for Preferences {
    fn from(update: PreferencesUpdate) -> Self {
        Self {
            theme: update.theme,
            english_variant: update.english_variant,
            portuguese_variant: update.portuguese_variant,
            explain_model: update.explain_model,
            capture_in_editors: update.capture_in_editors,
            auto_translate_selection: update.auto_translate_selection,
        }
    }
}

/// Tudo o que é persistido. O atalho fica à parte das preferências porque trocá-lo mexe no
/// sistema (`set_shortcut`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub shortcut: String,
    #[serde(flatten)]
    pub preferences: Preferences,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: DEFAULT_SHORTCUT.to_owned(),
            preferences: Preferences::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_todays_behavior() {
        let settings = Settings::default();

        assert_eq!(settings.shortcut, "Ctrl+Alt+T");
        assert_eq!(settings.preferences.theme, Theme::System);
        assert_eq!(settings.preferences.explain_model, ExplainModel::Haiku);
        assert!(!settings.preferences.capture_in_editors);
        assert!(settings.preferences.auto_translate_selection);
        assert_eq!(settings.preferences.target_langs(), TargetLangs::default());
    }

    #[test]
    fn variants_map_to_the_deepl_targets() {
        let preferences = Preferences {
            english_variant: EnglishVariant::EnGb,
            portuguese_variant: PortugueseVariant::PtPt,
            ..Preferences::default()
        };

        assert_eq!(
            preferences.target_langs(),
            TargetLangs {
                to_portuguese: TargetLang::PtPt,
                to_english: TargetLang::EnGb,
            }
        );
    }

    #[test]
    fn serializes_with_the_names_the_front_uses() {
        let json = serde_json::to_value(Settings::default()).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "shortcut": "Ctrl+Alt+T",
                "theme": "system",
                "englishVariant": "EN-US",
                "portugueseVariant": "PT-BR",
                "explainModel": "claude-haiku-5-5",
                "captureInEditors": false,
                "autoTranslateSelection": true
            })
        );
    }

    #[test]
    fn model_ids_are_the_ones_sent_to_the_api() {
        assert_eq!(ExplainModel::Haiku.id(), "claude-haiku-5-5");
        assert_eq!(ExplainModel::Sonnet.id(), "claude-sonnet-5-5");
    }

    #[test]
    fn an_update_needs_every_field_and_accepts_no_unknown_one() {
        let complete = serde_json::to_value(Preferences::default()).unwrap();
        assert!(serde_json::from_value::<PreferencesUpdate>(complete.clone()).is_ok());

        let partial = serde_json::json!({ "theme": "dark" });
        assert!(serde_json::from_value::<PreferencesUpdate>(partial).is_err());

        let mut with_extra = complete;
        with_extra["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<PreferencesUpdate>(with_extra).is_err());
    }

    #[test]
    fn an_update_converts_field_for_field() {
        let update: PreferencesUpdate = serde_json::from_value(serde_json::json!({
            "theme": "dark",
            "englishVariant": "EN-GB",
            "portugueseVariant": "PT-PT",
            "explainModel": "claude-sonnet-5-5",
            "captureInEditors": true,
            "autoTranslateSelection": false
        }))
        .unwrap();

        assert_eq!(
            Preferences::from(update),
            Preferences {
                theme: Theme::Dark,
                english_variant: EnglishVariant::EnGb,
                portuguese_variant: PortugueseVariant::PtPt,
                explain_model: ExplainModel::Sonnet,
                capture_in_editors: true,
                auto_translate_selection: false,
            }
        );
    }

    #[test]
    fn rejects_a_model_outside_the_allowed_list() {
        let parsed = serde_json::from_str::<Preferences>(r#"{"explainModel":"gpt-4"}"#);

        assert!(parsed.is_err());
    }
}
