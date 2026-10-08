use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, Window, WindowEvent};

use super::placement::{
    place_near_cursor, should_hide_on_blur, should_hide_unfocused, Point, Rect, Size, BLUR_GRACE,
};
use super::window_error;
use crate::error::AppError;

pub const POPUP_LABEL: &str = "popup";
/// Enviado ao front a cada exibição: limpar o texto, focar o campo e, se houver, preencher.
pub const EVENT_RESET: &str = "popup://reset";

/// Corpo do evento `popup://reset`. `prefill` é o texto capturado da seleção do usuário.
#[derive(Clone, serde::Serialize)]
struct ResetPayload<'a> {
    prefill: Option<&'a str>,
}
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
/// `prefill` é o texto capturado da seleção, entregue ao front junto do reset.
pub fn show_near_cursor(app: &AppHandle, prefill: Option<&str>) -> Result<(), AppError> {
    let window = app
        .get_webview_window(POPUP_LABEL)
        .ok_or_else(|| AppError::Window("janela do popup não encontrada".into()))?;

    // Atalho repetido (ou segurado) com o popup já em uso: não apaga o que o usuário digitou.
    if window.is_visible().map_err(window_error)? && window.is_focused().map_err(window_error)? {
        if prefill.is_some() {
            // Só o fato vai para o log, nunca o texto do usuário.
            log::info!("popup já estava em uso; o texto capturado foi descartado");
        }
        return Ok(());
    }

    let cursor = app.cursor_position().map_err(window_error)?;
    // Cursor fora de qualquer monitor (ex.: após desconectar um): usa o monitor primário,
    // e `place_near_cursor` limita a janela a ele.
    let monitor = match app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(window_error)?
    {
        Some(monitor) => Some(monitor),
        None => app.primary_monitor().map_err(window_error)?,
    };
    if let Some(monitor) = monitor {
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
    app.emit_to(POPUP_LABEL, EVENT_RESET, ResetPayload { prefill })
        .map_err(window_error)?;
    window.show().map_err(window_error)?;
    let focus_result = window.set_focus().map_err(window_error);
    hide_if_still_unfocused(window);
    focus_result
}

/// Um blur dentro de `BLUR_GRACE` é ignorado e nunca reemitido. Se o Windows recusar o foco,
/// o popup (always-on-top) ficaria preso na tela; esta verificação final o oculta.
fn hide_if_still_unfocused(window: WebviewWindow) {
    thread::spawn(move || {
        thread::sleep(BLUR_GRACE);

        let shown_at = window.app_handle().state::<PopupState>().shown_at();
        match window.is_focused() {
            Ok(is_focused) => {
                if should_hide_unfocused(is_focused, shown_at, Instant::now()) {
                    if let Err(error) = window.hide() {
                        log::warn!("não foi possível ocultar o popup sem foco: {error}");
                    }
                }
            }
            Err(error) => log::warn!("não foi possível consultar o foco do popup: {error}"),
        }
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_payload_carries_the_captured_text() {
        let json = serde_json::to_string(&ResetPayload {
            prefill: Some("olá mundo"),
        })
        .unwrap();

        assert_eq!(json, r#"{"prefill":"olá mundo"}"#);
    }

    #[test]
    fn reset_payload_without_capture_serializes_prefill_as_null() {
        let json = serde_json::to_string(&ResetPayload { prefill: None }).unwrap();

        assert_eq!(json, r#"{"prefill":null}"#);
    }
}
