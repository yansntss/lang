//! Preferências do usuário, guardadas em `settings.json` na pasta de dados do app.
//! Segredos (chaves de API) nunca passam por aqui: ficam no keychain.

mod store;

use serde::{Deserialize, Serialize};

pub use store::SettingsStore;

use crate::services::caption::DEFAULT_INTERVAL_MS;
use crate::services::translation::{TargetLang, TargetLangs};

/// Atalho de fábrica; também o que vale quando o salvo é inválido.
pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+T";
/// Atalho de fábrica da leitura de um trecho da tela (OCR).
pub const DEFAULT_OCR_SHORTCUT: &str = "Ctrl+Alt+Y";

/// Qual dos atalhos globais: o do tradutor ou o da leitura de tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShortcutSlot {
    Translate,
    Ocr,
}

impl ShortcutSlot {
    pub const ALL: [Self; 2] = [Self::Translate, Self::Ocr];

    pub fn factory_default(self) -> &'static str {
        match self {
            Self::Translate => DEFAULT_SHORTCUT,
            Self::Ocr => DEFAULT_OCR_SHORTCUT,
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::Translate => Self::Ocr,
            Self::Ocr => Self::Translate,
        }
    }

    /// Nome mostrado ao usuário quando os dois atalhos colidem.
    pub fn description(self) -> &'static str {
        match self {
            Self::Translate => "abrir o tradutor",
            Self::Ocr => "ler um trecho da tela",
        }
    }
}

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
    /// Oferecer a legenda ao vivo depois de ler um trecho da tela: a mesma região é relida a
    /// cada `caption_interval_ms` e o texto novo é traduzido. Gasta cota do DeepL.
    pub caption_mode_enabled: bool,
    /// Intervalo entre as releituras da legenda; o serviço o limita a uma faixa segura.
    pub caption_interval_ms: u32,
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
            caption_mode_enabled: false,
            caption_interval_ms: DEFAULT_INTERVAL_MS,
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
    caption_mode_enabled: bool,
    caption_interval_ms: u32,
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
            caption_mode_enabled: update.caption_mode_enabled,
            caption_interval_ms: update.caption_interval_ms,
        }
    }
}

/// Tamanho do popup em pixels lógicos (independente da escala do monitor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupSize {
    pub width: u32,
    pub height: u32,
}

/// Tudo o que é persistido. O atalho fica à parte das preferências porque trocá-lo mexe no
/// sistema (`set_shortcut`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub shortcut: String,
    pub ocr_shortcut: String,
    #[serde(flatten)]
    pub preferences: Preferences,
    /// Último tamanho que o usuário deu ao popup; ausente até ele redimensionar uma vez.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popup_size: Option<PopupSize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: DEFAULT_SHORTCUT.to_owned(),
            ocr_shortcut: DEFAULT_OCR_SHORTCUT.to_owned(),
            preferences: Preferences::default(),
            popup_size: None,
        }
    }
}

impl Settings {
    /// O atalho salvo para `slot`.
    pub fn shortcut_for(&self, slot: ShortcutSlot) -> &str {
        match slot {
            ShortcutSlot::Translate => &self.shortcut,
            ShortcutSlot::Ocr => &self.ocr_shortcut,
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
        assert!(!settings.preferences.caption_mode_enabled);
        assert_eq!(settings.preferences.caption_interval_ms, 1500);
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
                "ocrShortcut": "Ctrl+Alt+Y",
                "theme": "system",
                "englishVariant": "EN-US",
                "portugueseVariant": "PT-BR",
                "explainModel": "claude-haiku-5-5",
                "captureInEditors": false,
                "autoTranslateSelection": true,
                "captionModeEnabled": false,
                "captionIntervalMs": 1500
            })
        );
    }

    #[test]
    fn the_popup_size_is_only_written_once_the_user_has_chosen_one() {
        let chosen = Settings {
            popup_size: Some(PopupSize {
                width: 500,
                height: 320,
            }),
            ..Settings::default()
        };

        let json = serde_json::to_value(chosen).unwrap();

        assert_eq!(
            json["popupSize"],
            serde_json::json!({"width": 500, "height": 320})
        );
    }

    #[test]
    fn a_file_without_the_popup_size_still_loads() {
        let parsed = serde_json::from_str::<Settings>(r#"{"shortcut":"Ctrl+Alt+T"}"#).unwrap();

        assert_eq!(parsed.popup_size, None);
    }

    #[test]
    fn a_file_from_before_the_ocr_shortcut_gets_the_factory_one() {
        let parsed = serde_json::from_str::<Settings>(r#"{"shortcut":"Ctrl+Shift+J"}"#).unwrap();

        assert_eq!(parsed.shortcut_for(ShortcutSlot::Translate), "Ctrl+Shift+J");
        assert_eq!(parsed.shortcut_for(ShortcutSlot::Ocr), "Ctrl+Alt+Y");
    }

    #[test]
    fn each_slot_has_its_own_factory_shortcut_and_knows_the_other() {
        assert_eq!(ShortcutSlot::Translate.factory_default(), "Ctrl+Alt+T");
        assert_eq!(ShortcutSlot::Ocr.factory_default(), "Ctrl+Alt+Y");
        assert_eq!(ShortcutSlot::Translate.other(), ShortcutSlot::Ocr);
        assert_eq!(ShortcutSlot::Ocr.other(), ShortcutSlot::Translate);
    }

    #[test]
    fn slots_use_the_names_the_front_sends() {
        assert_eq!(
            serde_json::from_str::<ShortcutSlot>(r#""ocr""#).unwrap(),
            ShortcutSlot::Ocr
        );
        assert_eq!(
            serde_json::from_str::<ShortcutSlot>(r#""translate""#).unwrap(),
            ShortcutSlot::Translate
        );
        assert!(serde_json::from_str::<ShortcutSlot>(r#""other""#).is_err());
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
            "autoTranslateSelection": false,
            "captionModeEnabled": true,
            "captionIntervalMs": 2000
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
                caption_mode_enabled: true,
                caption_interval_ms: 2000,
            }
        );
    }

    #[test]
    fn rejects_a_model_outside_the_allowed_list() {
        let parsed = serde_json::from_str::<Preferences>(r#"{"explainModel":"gpt-4"}"#);

        assert!(parsed.is_err());
    }
}
