use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

use super::{Preferences, Settings};
use crate::error::AppError;

/// Configurações em memória, espelhadas em um JSON no disco. A gravação é atômica (arquivo
/// temporário + renomeação) e a memória só muda depois de gravar: uma falha não deixa o app
/// achando que salvou.
pub struct SettingsStore {
    path: PathBuf,
    current: RwLock<Settings>,
}

fn settings_error(error: impl std::fmt::Display) -> AppError {
    AppError::Settings(error.to_string())
}

impl SettingsStore {
    /// Nunca falha: arquivo ausente usa os padrões; arquivo ilegível é guardado como `.bak`
    /// (para o usuário não perder o que escreveu à mão) e os padrões entram no lugar.
    pub fn load(path: &Path) -> Self {
        let settings = match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
                log::error!("configurações ilegíveis, usando os padrões: {error}");
                keep_backup(path);
                Settings::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(error) => {
                log::error!("não foi possível ler as configurações: {error}");
                Settings::default()
            }
        };
        Self {
            path: path.to_path_buf(),
            current: RwLock::new(settings),
        }
    }

    pub fn get(&self) -> Settings {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn update_preferences(&self, preferences: Preferences) -> Result<Settings, AppError> {
        self.update(|settings| settings.preferences = preferences)
    }

    pub fn update_shortcut(&self, shortcut: &str) -> Result<Settings, AppError> {
        self.update(|settings| settings.shortcut = shortcut.to_owned())
    }

    fn update(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, AppError> {
        let mut guard = self.current.write().unwrap_or_else(PoisonError::into_inner);
        let mut next = guard.clone();
        change(&mut next);
        self.persist(&next)?;
        *guard = next.clone();
        Ok(next)
    }

    fn persist(&self, settings: &Settings) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(settings_error)?;
        }
        let json = serde_json::to_string_pretty(settings).map_err(settings_error)?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, json).map_err(settings_error)?;
        fs::rename(&temporary, &self.path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            settings_error(error)
        })
    }
}

fn keep_backup(path: &Path) {
    let backup = path.with_extension("json.bak");
    if let Err(error) = fs::rename(path, &backup) {
        log::warn!("não foi possível guardar o backup das configurações: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ExplainModel, Theme, DEFAULT_SHORTCUT};
    use super::*;

    fn path_in(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("settings.json")
    }

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();

        let store = SettingsStore::load(&path_in(&dir));

        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn saved_preferences_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(&dir);
        let preferences = Preferences {
            theme: Theme::Dark,
            explain_model: ExplainModel::Sonnet,
            capture_in_editors: true,
            auto_translate_selection: false,
            ..Preferences::default()
        };
        SettingsStore::load(&path)
            .update_preferences(preferences)
            .unwrap();

        let reloaded = SettingsStore::load(&path).get();

        assert_eq!(reloaded.preferences, preferences);
        assert_eq!(reloaded.shortcut, DEFAULT_SHORTCUT);
    }

    #[test]
    fn a_saved_shortcut_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(&dir);
        SettingsStore::load(&path)
            .update_shortcut("Ctrl+Shift+J")
            .unwrap();

        assert_eq!(SettingsStore::load(&path).get().shortcut, "Ctrl+Shift+J");
    }

    #[test]
    fn updating_returns_the_new_settings_and_keeps_the_other_group() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(&path_in(&dir));
        store.update_shortcut("Alt+F2").unwrap();

        let updated = store
            .update_preferences(Preferences {
                theme: Theme::Light,
                ..Preferences::default()
            })
            .unwrap();

        assert_eq!(updated.shortcut, "Alt+F2");
        assert_eq!(updated.preferences.theme, Theme::Light);
        assert_eq!(store.get(), updated);
    }

    #[test]
    fn missing_fields_are_filled_with_defaults_and_unknown_ones_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(&dir);
        fs::write(&path, r#"{"theme":"dark","fieldFromTheFuture":1}"#).unwrap();

        let settings = SettingsStore::load(&path).get();

        assert_eq!(settings.preferences.theme, Theme::Dark);
        assert_eq!(settings.shortcut, DEFAULT_SHORTCUT);
        assert!(settings.preferences.auto_translate_selection);
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults_and_is_kept_as_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(&dir);
        fs::write(&path, "{ isto não é json").unwrap();

        let store = SettingsStore::load(&path);

        assert_eq!(store.get(), Settings::default());
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.json.bak")).unwrap(),
            "{ isto não é json"
        );
        assert!(!path.exists());
    }

    #[test]
    fn a_value_outside_the_allowed_list_counts_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(&dir);
        fs::write(&path, r#"{"explainModel":"gpt-4"}"#).unwrap();

        let store = SettingsStore::load(&path);

        assert_eq!(store.get(), Settings::default());
        assert!(dir.path().join("settings.json.bak").exists());
    }

    #[test]
    fn a_failed_save_reports_an_error_and_keeps_the_previous_settings() {
        let dir = tempfile::tempdir().unwrap();
        // O "arquivo" fica dentro de um arquivo comum, então nem a pasta pode ser criada.
        let blocker = dir.path().join("bloqueio");
        fs::write(&blocker, "x").unwrap();
        let store = SettingsStore::load(&blocker.join("settings.json"));

        let error = store
            .update_preferences(Preferences {
                theme: Theme::Dark,
                ..Preferences::default()
            })
            .unwrap_err();

        assert_eq!(error.code(), "settings");
        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn saving_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(&path_in(&dir));

        store.update_shortcut("Ctrl+Alt+K").unwrap();

        assert!(!dir.path().join("settings.json.tmp").exists());
        assert!(path_in(&dir).exists());
    }
}
