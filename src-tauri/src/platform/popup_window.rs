use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Window, WindowEvent};

use super::placement::{place_near_cursor, should_hide_on_blur, Point, Rect, Size};
use super::window_error;
use crate::error::AppError;

pub const POPUP_LABEL: &str = "popup";
/// Enviado ao front a cada exibição: limpar o texto e focar o campo.
pub const EVENT_RESET: &str = "popup://reset";
const CURSOR_OFFSET_LOGICAL: f64 = 12.0;

/// Instante da última exibição, usado para ignorar blurs transitórios (ver `BLUR_GRACE`).
#[derive(Default)]
pub struct PopupState {
    shown_at: Mutex<Option<Instant>>,
}

impl PopupState {
    fn mark_shown(&self) {
        if let Ok(mut shown_at) = self.shown_at.lock() {
            *shown_at = Some(Instant::now());
        }
    }

    /// Com o lock envenenado devolve `None`, e o popup passa a ocultar em qualquer blur.
    fn shown_at(&self) -> Option<Instant> {
        self.shown_at.lock().ok().and_then(|shown_at| *shown_at)
    }
}

/// Posiciona o popup perto do cursor (dentro do monitor onde ele está), exibe e dá foco.
pub fn show_near_cursor(app: &AppHandle) -> Result<(), AppError> {
    let window = app
        .get_webview_window(POPUP_LABEL)
        .ok_or_else(|| AppError::Window("janela do popup não encontrada".into()))?;

    let cursor = app.cursor_position().map_err(window_error)?;
    if let Some(monitor) = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(window_error)?
    {
        let size = window.outer_size().map_err(window_error)?;
        let work_area = monitor.work_area();
        let placed = place_near_cursor(
            Point {
                x: round_to_i32(cursor.x),
                y: round_to_i32(cursor.y),
            },
            Size {
                width: size.width,
                height: size.height,
            },
            Rect {
                origin: Point {
                    x: work_area.position.x,
                    y: work_area.position.y,
                },
                size: Size {
                    width: work_area.size.width,
                    height: work_area.size.height,
                },
            },
            round_to_i32(CURSOR_OFFSET_LOGICAL * monitor.scale_factor()),
        );
        window
            .set_position(PhysicalPosition::new(placed.x, placed.y))
            .map_err(window_error)?;
    }

    app.state::<PopupState>().mark_shown();
    app.emit_to(POPUP_LABEL, EVENT_RESET, ())
        .map_err(window_error)?;
    window.show().map_err(window_error)?;
    window.set_focus().map_err(window_error)
}

/// Oculta o popup ao perder o foco e impede que seja destruído ao ser "fechado".
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != POPUP_LABEL {
        return;
    }

    match event {
        WindowEvent::Focused(false) => {
            let shown_at = window.state::<PopupState>().shown_at();
            if should_hide_on_blur(shown_at, Instant::now()) {
                hide(window);
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(window);
        }
        _ => {}
    }
}

fn hide(window: &Window) {
    if let Err(error) = window.hide() {
        log::warn!("não foi possível ocultar o popup: {error}");
    }
}

fn round_to_i32(value: f64) -> i32 {
    value.round() as i32
}
