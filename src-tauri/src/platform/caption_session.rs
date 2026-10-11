//! Sessão da legenda ao vivo: relê a última região lida pelo OCR e entrega ao popup cada fala
//! nova. Só uma sessão por vez; ocultar ou reabrir o popup, ler outra região ou desligar a
//! opção a encerra.

use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;

use tauri::{AppHandle, Emitter, Manager};

use super::generation::Generation;
use super::placement::Rect;
use super::popup_window::POPUP_LABEL;
use super::region_geometry::center_of;
use super::region_window::geometry_of;
use super::screen_capture::capture_monitor_at;
use super::{monitor_at, window_error};
use crate::error::AppError;
use crate::services::caption::{self, CaptionGate, CaptionSink, CaptionSource};
use crate::state::AppState;

/// Cada fala nova, para o popup traduzir.
pub const EVENT_CAPTION: &str = "popup://caption";
/// A legenda parou sozinha (falhas seguidas); o corpo traz o motivo.
pub const EVENT_CAPTION_STOPPED: &str = "popup://caption-stopped";

/// Corpo de `popup://caption`. `auto_translate` segue a mesma preferência da seleção.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptionLine<'a> {
    text: &'a str,
    auto_translate: bool,
}

/// A última região lida e o texto que saiu dela.
#[derive(Clone)]
struct LastRead {
    region: Rect,
    text: String,
}

/// A sessão em andamento.
struct Running {
    generation: u64,
    /// Nunca é usado para enviar: soltá-lo (ao trocar ou tirar a sessão) é o sinal de parar.
    _stop: Sender<()>,
}

#[derive(Default)]
pub struct CaptionState {
    last_read: Mutex<Option<LastRead>>,
    /// Começar e parar passam por este lock, junto da geração: o fim de uma sessão antiga
    /// nunca encerra a nova.
    running: Mutex<Option<Running>>,
    sessions: Generation,
}

impl CaptionState {
    fn running(&self) -> MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Guarda a região que acabou de ser lida, para a legenda poder relê-la.
pub fn remember(app: &AppHandle, region: Rect, text: &str) {
    let state = app.state::<CaptionState>();
    *state
        .last_read
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(LastRead {
        region,
        text: text.to_owned(),
    });
}

/// Liga a legenda sobre a última região lida.
pub fn start(app: &AppHandle) -> Result<(), AppError> {
    let preferences = app.state::<AppState>().settings.get().preferences;
    if !preferences.caption_mode_enabled {
        return Err(AppError::InvalidInput(
            "Ligue a legenda ao vivo nas Configurações.".into(),
        ));
    }
    let state = app.state::<CaptionState>();
    let last = state
        .last_read
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| {
            AppError::InvalidInput("Leia um trecho da tela antes de ligar a legenda.".into())
        })?;
    // O monitor da região não muda durante a sessão: é procurado uma vez só.
    let monitor = monitor_at(app, center_of(last.region))?
        .ok_or_else(|| AppError::ScreenCapture("nenhum monitor encontrado".into()))?;
    let source = RegionSource {
        app: app.clone(),
        region: last.region,
        monitor: geometry_of(&monitor).area,
    };

    let (sender, receiver) = mpsc::channel();
    let generation = {
        let mut running = state.running();
        // Uma sessão anterior acaba aqui: o remetente dela é solto ao ser substituído.
        let generation = state.sessions.begin();
        *running = Some(Running {
            generation,
            _stop: sender,
        });
        generation
    };
    // O popup não pode entrar na própria captura, nem se o usuário o arrastar sobre a região.
    protect_popup(app, true);

    let sink = PopupSink {
        app: app.clone(),
        generation,
        auto_translate: preferences.auto_translate_selection,
    };
    let interval = caption::interval(preferences.caption_interval_ms);
    let spawned = thread::Builder::new()
        .name("caption".into())
        .spawn(move || {
            caption::run(
                &source,
                &sink,
                CaptionGate::after(&last.text),
                interval,
                &receiver,
            );
        });
    if let Err(error) = spawned {
        stop_if(app, generation);
        return Err(AppError::Window(error.to_string()));
    }
    Ok(())
}

/// Encerra a legenda, se houver uma.
pub fn stop(app: &AppHandle) {
    end(app, |_| true);
}

/// Encerra a sessão `generation`, só se ela ainda for a atual.
fn stop_if(app: &AppHandle, generation: u64) -> bool {
    end(app, |running| running.generation == generation)
}

fn end(app: &AppHandle, should_end: impl FnOnce(&Running) -> bool) -> bool {
    let state = app.state::<CaptionState>();
    let ended = {
        let mut running = state.running();
        match running.as_ref() {
            Some(current) if should_end(current) => {
                state.sessions.begin();
                running.take();
                true
            }
            _ => false,
        }
    };
    if ended {
        protect_popup(app, false);
    }
    ended
}

/// Relê a região: fotografa o monitor dela, recorta e roda o OCR.
struct RegionSource {
    app: AppHandle,
    region: Rect,
    monitor: Rect,
}

impl CaptionSource for RegionSource {
    fn read(&self) -> Result<String, AppError> {
        let center = center_of(self.monitor);
        let snapshot = capture_monitor_at(center.x, center.y)?;
        // Preta, a tela não tem fala nenhuma a esperar: o jogo foi para a tela cheia exclusiva.
        if snapshot.is_blank() {
            return Err(AppError::BlankCapture);
        }
        let image = snapshot.crop_region(self.region, self.monitor);
        self.app.state::<AppState>().ocr.read(&image)
    }
}

/// Entrega ao popup o que a sessão produz, enquanto ela for a atual.
struct PopupSink {
    app: AppHandle,
    generation: u64,
    auto_translate: bool,
}

impl CaptionSink for PopupSink {
    fn caption(&self, text: &str) {
        let state = self.app.state::<CaptionState>();
        if !state.sessions.is_current(self.generation) {
            return;
        }
        let line = CaptionLine {
            text,
            auto_translate: self.auto_translate,
        };
        if let Err(error) = self.app.emit_to(POPUP_LABEL, EVENT_CAPTION, line) {
            log::warn!(
                "não foi possível entregar a legenda: {:?}",
                window_error(error)
            );
        }
    }

    fn failed(&self, failure: &AppError) {
        if !stop_if(&self.app, self.generation) {
            return;
        }
        let notice = failure.to_string();
        if let Err(error) = self.app.emit_to(POPUP_LABEL, EVENT_CAPTION_STOPPED, notice) {
            log::warn!(
                "não foi possível avisar o fim da legenda: {:?}",
                window_error(error)
            );
        }
    }
}

/// Tira (ou devolve) o popup das capturas de tela. Só vale durante a legenda: fora dela o popup
/// continua aparecendo em prints e compartilhamentos de tela.
fn protect_popup(app: &AppHandle, protected: bool) {
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };
    if let Err(error) = window.set_content_protected(protected) {
        log::warn!("não foi possível ajustar a captura do popup: {error}");
    }
}
