//! Verificação e instalação de atualizações do app.
//!
//! O plugin do atualizador é chamado só daqui, pelo Rust. O endpoint e a chave pública ficam no
//! `tauri.conf.json`: o front não escolhe de onde baixar nem o que instalar.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::AppError;

/// As notas vêm do GitHub, ou seja, de fora do app: só um trecho é mostrado.
pub const MAX_NOTES_CHARS: usize = 4000;

const MILLIS_PER_SECOND: i64 = 1000;

/// Uma versão nova encontrada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    /// Notas da versão, em texto puro e cortadas em [`MAX_NOTES_CHARS`].
    pub notes: Option<String>,
    /// Milissegundos desde 1970-01-01 UTC.
    pub published_at: Option<i64>,
}

/// Caracteres de controle (menos quebra de linha e tab) e os que mudam a direção do texto:
/// as notas vêm de fora, e eles servem para esconder ou embaralhar o que o usuário lê.
fn is_unsafe_in_notes(character: char) -> bool {
    (character.is_control() && character != '\n' && character != '\t')
        || matches!(
            character,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
        )
}

impl AvailableUpdate {
    pub fn new(version: String, notes: Option<String>, published_at: Option<i64>) -> Self {
        let notes = notes
            .map(|text| {
                let clean: String = text
                    .chars()
                    .filter(|character| !is_unsafe_in_notes(*character))
                    .collect();
                clean
                    .trim()
                    .chars()
                    .take(MAX_NOTES_CHARS)
                    .collect::<String>()
            })
            .filter(|text| !text.is_empty());
        Self {
            version,
            notes,
            published_at,
        }
    }
}

/// Andamento da instalação, enviado ao front por um canal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateEvent {
    Progress {
        downloaded: u64,
        total: Option<u64>,
    },
    /// O download terminou e o instalador vai assumir (no Windows o app encerra em seguida).
    Installing,
}

/// De onde vêm as atualizações. A implementação real usa o plugin; os testes usam um fake.
pub trait UpdateSource: Send + Sync {
    fn check(&self) -> impl Future<Output = Result<Option<AvailableUpdate>, AppError>> + Send;

    /// Instala a versão encontrada pelo último `check`.
    fn install<F>(&self, on_event: F) -> impl Future<Output = Result<(), AppError>> + Send
    where
        F: FnMut(UpdateEvent) + Send;
}

/// Libera a trava de operação em andamento ao sair do escopo, inclusive em erro.
struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Garante a ordem verificar → instalar e uma operação por vez: duas verificações (ou uma
/// verificação e uma instalação) ao mesmo tempo poderiam deixar o app mostrando as notas de uma
/// versão e instalando outra.
pub struct UpdateService<S: UpdateSource> {
    source: S,
    found: Mutex<Option<AvailableUpdate>>,
    busy: AtomicBool,
}

fn busy() -> AppError {
    AppError::InvalidInput("Já há uma atualização em andamento.".into())
}

impl<S: UpdateSource> UpdateService<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            found: Mutex::new(None),
            busy: AtomicBool::new(false),
        }
    }

    fn found(&self) -> MutexGuard<'_, Option<AvailableUpdate>> {
        self.found.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Procura uma versão nova. Uma verificação que falha apaga a anterior: nunca se instala o
    /// que não foi confirmado agora.
    pub async fn check(&self) -> Result<Option<AvailableUpdate>, AppError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(busy());
        }
        let _guard = BusyGuard(&self.busy);
        let result = self.source.check().await;
        *self.found() = result.as_ref().ok().cloned().flatten();
        result
    }

    pub async fn install<F>(&self, on_event: F) -> Result<(), AppError>
    where
        F: FnMut(UpdateEvent) + Send,
    {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(busy());
        }
        let _guard = BusyGuard(&self.busy);
        if self.found().is_none() {
            return Err(AppError::InvalidInput(
                "Verifique as atualizações antes de instalar.".into(),
            ));
        }
        self.source.install(on_event).await
    }
}

/// "Ainda não há release" não é erro para quem usa o app: só não há o que atualizar.
fn treat_missing_release<T>(
    result: Result<Option<T>, tauri_plugin_updater::Error>,
) -> Result<Option<T>, AppError> {
    match result {
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => Ok(None),
        other => other.map_err(map_updater_error),
    }
}

/// Falha de rede mantém o erro de rede; o resto (assinatura, download, instalador) vira uma
/// frase genérica, com o detalhe só no `Debug`/log.
fn map_updater_error(error: tauri_plugin_updater::Error) -> AppError {
    match &error {
        tauri_plugin_updater::Error::Reqwest(_) | tauri_plugin_updater::Error::Network(_) => {
            AppError::Network(error.to_string())
        }
        _ => AppError::Update(error.to_string()),
    }
}

/// Implementação real, em cima do plugin `tauri-plugin-updater`.
pub struct TauriUpdateSource {
    app: AppHandle,
    pending: Mutex<Option<Update>>,
}

impl TauriUpdateSource {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            pending: Mutex::new(None),
        }
    }

    fn pending(&self) -> MutexGuard<'_, Option<Update>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl UpdateSource for TauriUpdateSource {
    async fn check(&self) -> Result<Option<AvailableUpdate>, AppError> {
        let updater = self.app.updater().map_err(map_updater_error)?;
        let found = treat_missing_release(updater.check().await)?;
        let available = found.as_ref().map(|update| {
            AvailableUpdate::new(
                update.version.clone(),
                update.body.clone(),
                update
                    .date
                    .map(|date| date.unix_timestamp().saturating_mul(MILLIS_PER_SECOND)),
            )
        });
        *self.pending() = found;
        Ok(available)
    }

    async fn install<F>(&self, on_event: F) -> Result<(), AppError>
    where
        F: FnMut(UpdateEvent) + Send,
    {
        let update = self.pending().clone().ok_or_else(|| {
            AppError::InvalidInput("Verifique as atualizações antes de instalar.".into())
        })?;

        // Os dois avisos do plugin (pedaço baixado e fim do download) usam o mesmo destino.
        let sink = Mutex::new(on_event);
        let mut downloaded: u64 = 0;
        update
            .download_and_install(
                |chunk, total| {
                    downloaded = downloaded.saturating_add(chunk as u64);
                    if let Ok(mut send) = sink.lock() {
                        send(UpdateEvent::Progress { downloaded, total });
                    }
                },
                || {
                    if let Ok(mut send) = sink.lock() {
                        send(UpdateEvent::Installing);
                    }
                },
            )
            .await
            .map_err(map_updater_error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicUsize;

    use super::*;

    /// Fonte falsa: devolve verificações roteirizadas e, ao instalar, emite eventos e espera
    /// um sinal (para simular uma instalação demorada).
    #[derive(Default)]
    struct FakeSource {
        checks: Mutex<VecDeque<Result<Option<AvailableUpdate>, AppError>>>,
        install_events: Vec<UpdateEvent>,
        install_failure: Mutex<Option<AppError>>,
        hold_install: AtomicBool,
        hold_check: AtomicBool,
        installs: AtomicUsize,
    }

    impl FakeSource {
        fn finding(update: AvailableUpdate) -> Self {
            Self::with_checks(vec![Ok(Some(update))])
        }

        fn with_checks(checks: Vec<Result<Option<AvailableUpdate>, AppError>>) -> Self {
            Self {
                checks: Mutex::new(checks.into()),
                ..Self::default()
            }
        }
    }

    impl UpdateSource for FakeSource {
        async fn check(&self) -> Result<Option<AvailableUpdate>, AppError> {
            while self.hold_check.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
            self.checks
                .lock()
                .unwrap()
                .pop_front()
                .expect("o teste não roteirizou verificações suficientes")
        }

        async fn install<F>(&self, mut on_event: F) -> Result<(), AppError>
        where
            F: FnMut(UpdateEvent) + Send,
        {
            self.installs.fetch_add(1, Ordering::SeqCst);
            for event in &self.install_events {
                on_event(event.clone());
            }
            while self.hold_install.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
            match self.install_failure.lock().unwrap().take() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    fn update(version: &str) -> AvailableUpdate {
        AvailableUpdate::new(
            version.into(),
            Some("Correções.".into()),
            Some(1_700_000_000_000),
        )
    }

    #[tokio::test]
    async fn reports_the_version_found_by_the_check() {
        let service = UpdateService::new(FakeSource::finding(update("0.2.0")));

        let found = service.check().await.unwrap();

        assert_eq!(found, Some(update("0.2.0")));
    }

    #[tokio::test]
    async fn reports_nothing_when_there_is_no_newer_version() {
        let service = UpdateService::new(FakeSource::with_checks(vec![Ok(None)]));

        assert_eq!(service.check().await.unwrap(), None);
    }

    #[tokio::test]
    async fn installs_after_a_check_and_forwards_the_events_in_order() {
        let source = FakeSource {
            install_events: vec![
                UpdateEvent::Progress {
                    downloaded: 10,
                    total: Some(30),
                },
                UpdateEvent::Progress {
                    downloaded: 30,
                    total: Some(30),
                },
                UpdateEvent::Installing,
            ],
            ..FakeSource::finding(update("0.2.0"))
        };
        let service = UpdateService::new(source);
        service.check().await.unwrap();
        let mut seen = Vec::new();

        service.install(|event| seen.push(event)).await.unwrap();

        assert_eq!(
            seen,
            vec![
                UpdateEvent::Progress {
                    downloaded: 10,
                    total: Some(30)
                },
                UpdateEvent::Progress {
                    downloaded: 30,
                    total: Some(30)
                },
                UpdateEvent::Installing,
            ]
        );
    }

    #[tokio::test]
    async fn refuses_to_install_without_a_check() {
        let service = UpdateService::new(FakeSource::default());

        let error = service.install(|_| {}).await.unwrap_err();

        assert_eq!(error.code(), "invalid_input");
        assert_eq!(service.source.installs.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn refuses_to_install_when_the_last_check_found_nothing() {
        let service = UpdateService::new(FakeSource::with_checks(vec![
            Ok(Some(update("0.2.0"))),
            Ok(None),
        ]));
        service.check().await.unwrap();
        service.check().await.unwrap();

        let error = service.install(|_| {}).await.unwrap_err();

        assert_eq!(error.code(), "invalid_input");
    }

    #[tokio::test]
    async fn a_failed_check_discards_the_version_found_before() {
        let service = UpdateService::new(FakeSource::with_checks(vec![
            Ok(Some(update("0.2.0"))),
            Err(AppError::Network("sem rede".into())),
        ]));
        service.check().await.unwrap();

        let error = service.check().await.unwrap_err();

        assert_eq!(error.code(), "network");
        assert_eq!(
            service.install(|_| {}).await.unwrap_err().code(),
            "invalid_input"
        );
    }

    #[tokio::test]
    async fn a_second_install_while_one_is_running_is_refused() {
        let source = FakeSource::finding(update("0.2.0"));
        source.hold_install.store(true, Ordering::SeqCst);
        let service = UpdateService::new(source);
        service.check().await.unwrap();

        let (first, second) = tokio::join!(service.install(|_| {}), async {
            tokio::task::yield_now().await;
            let refused = service.install(|_| {}).await;
            service.source.hold_install.store(false, Ordering::SeqCst);
            refused
        });

        assert!(first.is_ok());
        assert_eq!(second.unwrap_err().code(), "invalid_input");
        assert_eq!(service.source.installs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn checking_during_an_install_is_refused() {
        let source = FakeSource::finding(update("0.2.0"));
        source.hold_install.store(true, Ordering::SeqCst);
        let service = UpdateService::new(source);
        service.check().await.unwrap();

        let (_, check) = tokio::join!(service.install(|_| {}), async {
            tokio::task::yield_now().await;
            let refused = service.check().await;
            service.source.hold_install.store(false, Ordering::SeqCst);
            refused
        });

        assert_eq!(check.unwrap_err().code(), "invalid_input");
    }

    #[tokio::test]
    async fn a_second_check_while_one_is_running_is_refused() {
        let source = FakeSource::with_checks(vec![Ok(Some(update("0.2.0")))]);
        source.hold_check.store(true, Ordering::SeqCst);
        let service = UpdateService::new(source);

        let (first, second) = tokio::join!(service.check(), async {
            tokio::task::yield_now().await;
            let refused = service.check().await;
            service.source.hold_check.store(false, Ordering::SeqCst);
            refused
        });

        assert_eq!(first.unwrap(), Some(update("0.2.0")));
        assert_eq!(second.unwrap_err().code(), "invalid_input");
    }

    #[tokio::test]
    async fn installing_while_a_check_is_running_is_refused() {
        let source = FakeSource::with_checks(vec![Ok(Some(update("0.2.0"))), Ok(None)]);
        let service = UpdateService::new(source);
        service.check().await.unwrap();
        service.source.hold_check.store(true, Ordering::SeqCst);

        let (check, install) = tokio::join!(service.check(), async {
            tokio::task::yield_now().await;
            let refused = service.install(|_| {}).await;
            service.source.hold_check.store(false, Ordering::SeqCst);
            refused
        });

        assert_eq!(check.unwrap(), None);
        assert_eq!(install.unwrap_err().code(), "invalid_input");
        assert_eq!(service.source.installs.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_service_is_usable_again_after_every_operation() {
        let service = UpdateService::new(FakeSource::with_checks(vec![
            Ok(None),
            Err(AppError::Network("x".into())),
            Ok(Some(update("0.2.0"))),
        ]));

        assert!(service.check().await.is_ok());
        assert!(service.check().await.is_err());
        assert!(service.check().await.is_ok());
        assert!(service.install(|_| {}).await.is_ok());
    }

    #[test]
    fn notes_lose_control_and_text_direction_characters() {
        let tricky = "a\u{202E}b\u{0007}c\r\nd\u{2066}e\te\u{200F}";

        let clean = AvailableUpdate::new("1.0.0".into(), Some(tricky.into()), None);

        assert_eq!(clean.notes.as_deref(), Some("abc\nde\te"));
    }

    #[tokio::test]
    async fn a_failed_install_can_be_tried_again() {
        let source = FakeSource::finding(update("0.2.0"));
        *source.install_failure.lock().unwrap() = Some(AppError::Update("falhou".into()));
        let service = UpdateService::new(source);
        service.check().await.unwrap();

        let first = service.install(|_| {}).await;
        let second = service.install(|_| {}).await;

        assert_eq!(first.unwrap_err().code(), "update");
        assert!(second.is_ok());
        assert_eq!(service.source.installs.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn notes_are_plain_trimmed_and_capped() {
        let long = "a".repeat(MAX_NOTES_CHARS + 500);

        let capped = AvailableUpdate::new("1.0.0".into(), Some(format!("  {long}  ")), None);
        let blank = AvailableUpdate::new("1.0.0".into(), Some("  \n ".into()), None);
        let none = AvailableUpdate::new("1.0.0".into(), None, None);

        assert_eq!(capped.notes.unwrap().chars().count(), MAX_NOTES_CHARS);
        assert_eq!(blank.notes, None);
        assert_eq!(none.notes, None);
    }

    #[test]
    fn notes_are_kept_as_text_without_interpreting_markup() {
        let update = AvailableUpdate::new(
            "1.0.0".into(),
            Some("<script>alert(1)</script> **negrito**".into()),
            None,
        );

        assert_eq!(
            update.notes.as_deref(),
            Some("<script>alert(1)</script> **negrito**")
        );
    }

    #[test]
    fn events_and_updates_serialize_with_the_names_the_front_uses() {
        let progress = serde_json::to_value(UpdateEvent::Progress {
            downloaded: 1024,
            total: Some(4096),
        })
        .unwrap();
        let unknown_total = serde_json::to_value(UpdateEvent::Progress {
            downloaded: 5,
            total: None,
        })
        .unwrap();
        let installing = serde_json::to_value(UpdateEvent::Installing).unwrap();
        let available = serde_json::to_value(update("0.2.0")).unwrap();

        assert_eq!(
            progress,
            serde_json::json!({ "kind": "progress", "downloaded": 1024, "total": 4096 })
        );
        assert_eq!(
            unknown_total,
            serde_json::json!({ "kind": "progress", "downloaded": 5, "total": null })
        );
        assert_eq!(installing, serde_json::json!({ "kind": "installing" }));
        assert_eq!(
            available,
            serde_json::json!({
                "version": "0.2.0",
                "notes": "Correções.",
                "publishedAt": 1_700_000_000_000_i64
            })
        );
    }

    #[test]
    fn a_missing_release_means_there_is_nothing_to_update() {
        let result = treat_missing_release::<()>(Err(tauri_plugin_updater::Error::ReleaseNotFound));

        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn network_failures_keep_their_kind_and_the_rest_becomes_a_generic_update_error() {
        let network = treat_missing_release::<()>(Err(tauri_plugin_updater::Error::Network(
            "timeout".into(),
        )))
        .unwrap_err();
        let other = treat_missing_release::<()>(Err(tauri_plugin_updater::Error::EmptyEndpoints))
            .unwrap_err();

        assert_eq!(network.code(), "network");
        assert_eq!(other.code(), "update");
    }

    #[test]
    fn the_message_shown_to_the_user_never_carries_the_internal_detail() {
        let error = map_updater_error(tauri_plugin_updater::Error::Network(
            "https://github.com/x/y: connection reset".into(),
        ));
        let other = map_updater_error(tauri_plugin_updater::Error::EmptyEndpoints);

        assert!(!error.to_string().contains("github.com"));
        assert!(!other.to_string().contains("endpoints"));
        assert!(format!("{other:?}").contains("endpoints"));
    }

    fn tauri_conf() -> serde_json::Value {
        serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap()
    }

    #[test]
    fn the_updater_section_of_tauri_conf_is_valid_and_strict() {
        let section = tauri_conf()["plugins"]["updater"].clone();

        let config: tauri_plugin_updater::Config = serde_json::from_value(section).unwrap();

        assert_eq!(config.endpoints.len(), 1);
        let endpoint = &config.endpoints[0];
        assert_eq!(endpoint.scheme(), "https");
        assert_eq!(endpoint.host_str(), Some("github.com"));
        assert!(endpoint
            .path()
            .starts_with("/yansntss/lang/releases/latest/download/"));
        assert!(config.require_signed_version);
        assert!(!config.allow_downgrades);
        assert!(!config.dangerous_insecure_transport_protocol);
        assert!(!config.dangerous_accept_invalid_certs);
        assert!(!config.dangerous_accept_invalid_hostnames);
    }

    #[test]
    fn the_public_key_in_tauri_conf_looks_like_a_minisign_public_key() {
        let key = tauri_conf()["plugins"]["updater"]["pubkey"]
            .as_str()
            .unwrap()
            .to_owned();

        // Base64 de "untrusted comment: minisign public key: …" (a chave privada nunca é
        // base64 deste texto: ela começa com "untrusted comment: rsign encrypted secret key").
        assert!(key.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6"));
        assert!(key.len() < 400, "uma chave pública é curta");
    }

    #[test]
    fn releases_build_updater_files_and_the_local_build_does_not_need_the_signing_key() {
        let local: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.local.conf.json")).unwrap();

        assert_eq!(tauri_conf()["bundle"]["createUpdaterArtifacts"], true);
        assert_eq!(local["bundle"]["createUpdaterArtifacts"], false);
    }

    #[test]
    fn the_updater_is_not_granted_to_any_window() {
        for capability in [
            include_str!("../../capabilities/popup.json"),
            include_str!("../../capabilities/settings.json"),
            include_str!("../../capabilities/history.json"),
        ] {
            assert!(!capability.contains("updater:"), "{capability}");
        }
    }

    #[test]
    fn only_the_settings_window_can_check_and_install_updates() {
        assert!(include_str!("../../capabilities/settings.json").contains("allow-check-update"));
        assert!(include_str!("../../capabilities/settings.json").contains("allow-install-update"));
        for capability in [
            include_str!("../../capabilities/popup.json"),
            include_str!("../../capabilities/history.json"),
        ] {
            assert!(!capability.contains("-update"), "{capability}");
        }
    }

    #[test]
    fn a_found_update_passes_a_successful_check_through() {
        let result = treat_missing_release(Ok(Some(7)));

        assert_eq!(result.unwrap(), Some(7));
    }
}
