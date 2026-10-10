//! Janela de seleção de região: cobre o monitor sob o cursor com uma foto dele, escurecida, e
//! o usuário arrasta um retângulo sobre o texto. Esc ou perder o foco cancela.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Instant;

use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, Window, WindowEvent,
};

use super::placement::{should_hide_on_blur, Point, Rect, Size};
use super::popup_window::{show_beside_region, show_notice};
use super::region_geometry::{center_of, selection_to_screen, LogicalSelection, MonitorGeometry};
use super::screen_capture::{capture_monitor_at, Snapshot};
use super::{log_failure, monitor_at, window_error};
use crate::error::AppError;

pub const REGION_LABEL: &str = "region";
/// Enviado à janela de seleção a cada abertura: buscar a captura nova e limpar a seleção.
pub const EVENT_REGION_SHOW: &str = "region://show";

/// A captura em andamento e o monitor de onde ela veio.
struct ActiveRegion {
    snapshot: Snapshot,
    monitor: MonitorGeometry,
}

#[derive(Default)]
pub struct RegionState {
    /// Só existe enquanto a janela de seleção está aberta.
    active: Mutex<Option<ActiveRegion>>,
    /// Segurar o atalho repete `Pressed`; enquanto uma captura roda, os demais são ignorados.
    opening: AtomicBool,
    shown_at: Mutex<Option<Instant>>,
}

impl RegionState {
    /// O estado só guarda dados: um lock envenenado continua utilizável.
    fn with_active<R>(&self, change: impl FnOnce(&mut Option<ActiveRegion>) -> R) -> R {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut active)
    }

    /// A captura no formato que a janela de seleção desenha.
    pub fn encoded_snapshot(&self) -> Option<Vec<u8>> {
        self.with_active(|active| active.as_ref().map(|region| region.snapshot.encode()))
    }

    fn monitor(&self) -> Option<MonitorGeometry> {
        self.with_active(|active| active.as_ref().map(|region| region.monitor))
    }

    fn mark_shown(&self) {
        *self.shown_at.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    fn shown_at(&self) -> Option<Instant> {
        *self.shown_at.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Atalho de OCR: fotografa o monitor sob o cursor e abre a janela de seleção sobre ele. A
/// captura roda fora da thread de eventos.
pub fn start(app: AppHandle) {
    {
        let state = app.state::<RegionState>();
        if state.opening.swap(true, Ordering::SeqCst) {
            return;
        }
        if is_open(&app) {
            state.opening.store(false, Ordering::SeqCst);
            return;
        }
    }

    let worker_app = app.clone();
    let spawned = thread::Builder::new()
        .name("region-capture".into())
        .spawn(move || {
            let result = open(&worker_app);
            worker_app
                .state::<RegionState>()
                .opening
                .store(false, Ordering::SeqCst);
            if let Err(error) = result {
                report(&worker_app, &error);
            }
        });

    if let Err(error) = spawned {
        app.state::<RegionState>()
            .opening
            .store(false, Ordering::SeqCst);
        log::error!("não foi possível iniciar a captura da tela: {error}");
    }
}

/// A captura falhou antes de haver janela de seleção: o popup avisa o usuário perto do cursor.
fn report(app: &AppHandle, error: &AppError) {
    log::warn!("falha ao capturar a tela: {error:?}");
    log_failure(
        "avisar a falha da captura",
        show_notice(app, &error.to_string()),
    );
}

fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(REGION_LABEL)
        .is_some_and(|window| window.is_visible().unwrap_or(false))
}

fn open(app: &AppHandle) -> Result<(), AppError> {
    let cursor = app.cursor_position().map_err(window_error)?;
    let point = Point {
        x: cursor.x.round() as i32,
        y: cursor.y.round() as i32,
    };
    let monitor = monitor_at(app, point)?
        .ok_or_else(|| AppError::ScreenCapture("nenhum monitor encontrado".into()))?;
    let geometry = geometry_of(&monitor);

    // Fotografa pelo centro do monitor escolhido: assim a foto e a janela nunca discordam
    // de monitor, mesmo com o cursor na divisa entre dois.
    let center = center_of(geometry.area);
    let snapshot = capture_monitor_at(center.x, center.y)?;
    if snapshot.is_blank() {
        return Err(AppError::BlankCapture);
    }
    if (snapshot.width, snapshot.height) != (geometry.area.size.width, geometry.area.size.height) {
        log::warn!(
            "captura de {}x{} para um monitor de {}x{}",
            snapshot.width,
            snapshot.height,
            geometry.area.size.width,
            geometry.area.size.height
        );
    }

    app.state::<RegionState>().with_active(|active| {
        *active = Some(ActiveRegion {
            snapshot,
            monitor: geometry,
        });
    });
    // Sem janela de seleção, ninguém usaria a captura: ela não fica para trás na memória.
    present(app, geometry.area).inspect_err(|_| {
        log_failure("descartar a captura", close(app));
    })
}

/// Cobre `area` (o monitor fotografado) com a janela de seleção e lhe dá o foco.
fn present(app: &AppHandle, area: Rect) -> Result<(), AppError> {
    let window = region_window(app)?;
    window
        .set_position(PhysicalPosition::new(area.origin.x, area.origin.y))
        .map_err(window_error)?;
    window
        .set_size(PhysicalSize::new(area.size.width, area.size.height))
        .map_err(window_error)?;
    app.state::<RegionState>().mark_shown();
    app.emit_to(REGION_LABEL, EVENT_REGION_SHOW, ())
        .map_err(window_error)?;
    window.show().map_err(window_error)?;
    window.set_focus().map_err(window_error)
}

/// A janela é criada no primeiro uso e depois só ocultada.
fn region_window(app: &AppHandle) -> Result<WebviewWindow, AppError> {
    if let Some(window) = app.get_webview_window(REGION_LABEL) {
        return Ok(window);
    }
    WebviewWindowBuilder::new(app, REGION_LABEL, WebviewUrl::App("region.html".into()))
        .title("Lang — selecionar texto")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(window_error)
}

/// O usuário soltou o mouse: converte a seleção em pixels da tela, fecha a janela de seleção
/// e abre o popup fixado ao lado da região. Uma seleção recusada deixa a janela aberta, para
/// o usuário tentar de novo.
pub fn confirm(app: &AppHandle, selection: LogicalSelection) -> Result<(), AppError> {
    let monitor = app
        .state::<RegionState>()
        .monitor()
        .ok_or_else(|| AppError::InvalidInput("Nenhuma seleção em andamento.".into()))?;
    let region = selection_to_screen(selection, monitor)?;
    close(app)?;
    show_beside_region(app, region, None)
}

/// Descarta a captura e oculta a janela de seleção.
pub fn close(app: &AppHandle) -> Result<(), AppError> {
    app.state::<RegionState>()
        .with_active(|active| *active = None);
    match app.get_webview_window(REGION_LABEL) {
        Some(window) => window.hide().map_err(window_error),
        None => Ok(()),
    }
}

/// Perder o foco cancela a seleção (o usuário foi para outro app); fechar só oculta.
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != REGION_LABEL {
        return;
    }
    match event {
        WindowEvent::Focused(false) => {
            let shown_at = window.state::<RegionState>().shown_at();
            if should_hide_on_blur(shown_at, Instant::now()) {
                log_failure("cancelar a seleção", close(window.app_handle()));
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            log_failure("cancelar a seleção", close(window.app_handle()));
        }
        _ => {}
    }
}

fn geometry_of(monitor: &Monitor) -> MonitorGeometry {
    let position = monitor.position();
    let size = monitor.size();
    MonitorGeometry {
        area: Rect {
            origin: Point {
                x: position.x,
                y: position.y,
            },
            size: Size {
                width: size.width,
                height: size.height,
            },
        },
        scale: monitor.scale_factor(),
    }
}
