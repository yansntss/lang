use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize,
    WebviewWindow, Window, WindowEvent,
};

use super::placement::{
    clamp_into_area, place_near_cursor, should_hide_on_blur, should_hide_unfocused, Point, Rect,
    Size, BLUR_GRACE,
};
use super::popup_size::{
    clamp_size, fit_height, is_echo, max_size, preferred_after_user_resize, same_size, DEFAULT_SIZE,
};
use super::region_geometry::{center_of, place_beside_region};
use super::{monitor_at, window_error};
use crate::error::AppError;
use crate::settings::PopupSize;
use crate::state::AppState;

pub const POPUP_LABEL: &str = "popup";
/// Enviado ao front a cada exibição: limpar o texto, focar o campo e, se houver, preencher.
pub const EVENT_RESET: &str = "popup://reset";

/// Corpo do evento `popup://reset`. `prefill` é o texto capturado da seleção do usuário,
/// `auto_translate` diz se ele deve ser traduzido sozinho ou só depois do Enter, `pinned` se
/// o popup continua fixado na tela, e `notice` um aviso a mostrar (por exemplo, a captura da
/// tela falhou).
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ResetPayload<'a> {
    prefill: Option<&'a str>,
    auto_translate: bool,
    pinned: bool,
    notice: Option<&'a str>,
}
const CURSOR_OFFSET_LOGICAL: f64 = 12.0;
/// Logo depois de exibir, o app mexe no tamanho e na posição da janela; esses `Resized`
/// não são o usuário arrastando as bordas.
const SHOW_SETTLE: Duration = Duration::from_millis(300);

/// Tamanhos do popup em pixels lógicos.
#[derive(Default)]
struct SizeState {
    /// O que o usuário escolheu; `None` até ser lido das configurações.
    preferred: Option<PopupSize>,
    /// O último tamanho que o próprio app pediu à janela (exibição ou crescimento).
    requested: Option<PopupSize>,
    /// O usuário mudou o tamanho desde a última gravação.
    dirty: bool,
}

/// Instante da última exibição, usado para ignorar blurs transitórios (ver `BLUR_GRACE`), e o
/// tamanho do popup.
#[derive(Default)]
pub struct PopupState {
    shown_at: Mutex<Option<Instant>>,
    size: Mutex<SizeState>,
    /// Fixado: não some ao perder o foco. Vale só até o popup ser ocultado.
    pinned: AtomicBool,
    /// O usuário está movendo ou redimensionando a janela. O arrasto nativo do Windows tira o
    /// foco do conteúdo web, e esse blur não é um clique fora.
    interacting: AtomicBool,
}

impl PopupState {
    fn is_pinned(&self) -> bool {
        self.pinned.load(Ordering::Relaxed)
    }

    fn is_interacting(&self) -> bool {
        self.interacting.load(Ordering::Relaxed)
    }

    fn mark_shown(&self) {
        if let Ok(mut shown_at) = self.shown_at.lock() {
            *shown_at = Some(Instant::now());
        }
    }

    /// Com o lock envenenado devolve `None`, e o popup passa a ocultar em qualquer blur.
    fn shown_at(&self) -> Option<Instant> {
        self.shown_at.lock().ok().and_then(|shown_at| *shown_at)
    }

    fn just_shown(&self) -> bool {
        self.shown_at()
            .is_some_and(|shown| shown.elapsed() < SHOW_SETTLE)
    }

    /// O estado de tamanho só guarda números: um lock envenenado continua utilizável.
    fn with_size<R>(&self, change: impl FnOnce(&mut SizeState) -> R) -> R {
        let mut size = self.size.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut size)
    }
}

/// Posiciona o popup perto do cursor (dentro do monitor onde ele está), exibe e dá foco.
/// `prefill` é o texto capturado da seleção, entregue ao front junto do reset.
pub fn show_near_cursor(app: &AppHandle, prefill: Option<&str>) -> Result<(), AppError> {
    show_near_cursor_with(app, prefill, None)
}

/// Abre o popup perto do cursor só para mostrar um aviso (o campo fica vazio).
pub fn show_notice(app: &AppHandle, notice: &str) -> Result<(), AppError> {
    show_near_cursor_with(app, None, Some(notice))
}

fn popup_window(app: &AppHandle) -> Result<WebviewWindow, AppError> {
    app.get_webview_window(POPUP_LABEL)
        .ok_or_else(|| AppError::Window("janela do popup não encontrada".into()))
}

fn show_near_cursor_with(
    app: &AppHandle,
    prefill: Option<&str>,
    notice: Option<&str>,
) -> Result<(), AppError> {
    let window = popup_window(app)?;

    // Atalho repetido (ou segurado) com o popup já em uso: não apaga o que o usuário digitou.
    if window.is_visible().map_err(window_error)? && window.is_focused().map_err(window_error)? {
        if prefill.is_some() || notice.is_some() {
            // Só o fato vai para o log, nunca o texto do usuário.
            log::info!("popup já estava em uso; o texto capturado ou o aviso foi descartado");
        }
        return Ok(());
    }

    // Fixado e visível: fica onde o usuário o deixou, no tamanho em que está. Só recebe o
    // texto capturado (se houver; senão mantém o que estava no campo) e o foco.
    if window.is_visible().map_err(window_error)? && app.state::<PopupState>().is_pinned() {
        if prefill.is_some() || notice.is_some() {
            emit_reset(app, prefill, true, notice)?;
        }
        return window.set_focus().map_err(window_error);
    }

    let cursor = app.cursor_position().map_err(window_error)?;
    // Cursor fora de qualquer monitor: `monitor_at` usa o primário, e `place_near_cursor`
    // limita a janela a ele.
    let monitor = monitor_at(
        app,
        Point {
            x: round_to_i32(cursor.x),
            y: round_to_i32(cursor.y),
        },
    )?;
    app.state::<PopupState>().mark_shown();
    if let Some(monitor) = monitor {
        // Sempre volta ao tamanho escolhido: o crescimento pelo conteúdo vale só para a
        // tradução que o causou.
        request_preferred_size(&window, &monitor)?;
        let size = window.outer_size().map_err(window_error)?;
        let placed = place_near_cursor(
            Point {
                x: round_to_i32(cursor.x),
                y: round_to_i32(cursor.y),
            },
            Size {
                width: size.width,
                height: size.height,
            },
            work_rect(&monitor),
            round_to_i32(CURSOR_OFFSET_LOGICAL * monitor.scale_factor()),
        );
        window
            .set_position(PhysicalPosition::new(placed.x, placed.y))
            .map_err(window_error)?;
    }

    emit_reset(app, prefill, false, notice)?;
    window.show().map_err(window_error)?;
    let focus_result = window.set_focus().map_err(window_error);
    hide_if_still_unfocused(window);
    focus_result
}

/// Abre o popup fixado ao lado de `region` (pixels físicos da tela), sem cobri-la: o texto
/// lido da tela continua visível, e uma nova leitura da mesma região não pega o popup.
pub fn show_beside_region(
    app: &AppHandle,
    region: Rect,
    prefill: Option<&str>,
) -> Result<(), AppError> {
    let window = popup_window(app)?;
    let monitor = monitor_at(app, center_of(region))?;

    app.state::<PopupState>().mark_shown();
    if let Some(monitor) = monitor {
        request_preferred_size(&window, &monitor)?;
        let size = window.outer_size().map_err(window_error)?;
        let placed = place_beside_region(
            region,
            Size {
                width: size.width,
                height: size.height,
            },
            work_rect(&monitor),
            round_to_i32(CURSOR_OFFSET_LOGICAL * monitor.scale_factor()),
        );
        window
            .set_position(PhysicalPosition::new(placed.x, placed.y))
            .map_err(window_error)?;
    }

    // Fixa só agora: se algo acima falhou, o popup não fica fixado sem ter aparecido.
    set_pinned(app, true);
    emit_reset(app, prefill, true, None)?;
    window.show().map_err(window_error)?;
    window.set_focus().map_err(window_error)
}

fn emit_reset(
    app: &AppHandle,
    prefill: Option<&str>,
    pinned: bool,
    notice: Option<&str>,
) -> Result<(), AppError> {
    // Sem o estado ainda (uma segunda instância muito cedo), mantém o comportamento de fábrica.
    let auto_translate = app
        .try_state::<AppState>()
        .is_none_or(|state| state.settings.get().preferences.auto_translate_selection);
    app.emit_to(
        POPUP_LABEL,
        EVENT_RESET,
        ResetPayload {
            prefill,
            auto_translate,
            pinned,
            notice,
        },
    )
    .map_err(window_error)
}

/// Fixa ou solta o popup. Fixado, ele não some ao perder o foco.
pub fn set_pinned(app: &AppHandle, pinned: bool) {
    app.state::<PopupState>()
        .pinned
        .store(pinned, Ordering::Relaxed);
}

/// Marca o início e o fim de um arrasto (mover ou redimensionar) feito pelo usuário. Durante o
/// arrasto, perder o foco não oculta o popup.
pub fn set_interacting(app: &AppHandle, interacting: bool) {
    app.state::<PopupState>()
        .interacting
        .store(interacting, Ordering::Relaxed);
}

/// Antes de ocultar: grava o tamanho escolhido e solta o popup, que volta a abrir perto do
/// cursor na próxima vez.
pub fn prepare_to_hide(app: &AppHandle) {
    set_pinned(app, false);
    save_size_if_changed(app);
}

/// Faz o popup crescer (ou voltar ao tamanho escolhido) para caber `content_height` pixels
/// lógicos de conteúdo, sem passar do monitor. Só a altura muda: a largura é do usuário.
pub fn fit_to_content(window: &WebviewWindow, content_height: f64) -> Result<(), AppError> {
    if !content_height.is_finite() || content_height <= 0.0 {
        return Err(AppError::Window("altura do conteúdo inválida".into()));
    }
    let Some(monitor) = window.current_monitor().map_err(window_error)? else {
        return Ok(());
    };
    let scale = window.scale_factor().map_err(window_error)?;
    let max = max_size(work_area_logical(&monitor));
    let current = logical_size(window.inner_size().map_err(window_error)?, scale);
    let height = fit_height(
        preferred_size(window.app_handle()).height,
        content_height.min(f64::from(max.height)).ceil() as u32,
        max.height,
    );
    let next = PopupSize {
        width: current.width,
        height,
    };
    if same_size(current, next) {
        return Ok(());
    }

    request_size(window, next)?;
    keep_inside_work_area(window, &monitor)
}

/// O tamanho que o usuário escolheu, ou o de fábrica.
fn preferred_size(app: &AppHandle) -> PopupSize {
    let stored = app
        .try_state::<AppState>()
        .map(|state| state.settings.get().popup_size);
    app.state::<PopupState>().with_size(|size| {
        if let Some(preferred) = size.preferred {
            return preferred;
        }
        // Sem o estado ainda, usa o de fábrica sem fixá-lo: a próxima leitura tenta de novo.
        let Some(stored) = stored else {
            return DEFAULT_SIZE;
        };
        let preferred = stored.unwrap_or(DEFAULT_SIZE);
        size.preferred = Some(preferred);
        preferred
    })
}

/// Dá ao popup o tamanho escolhido pelo usuário, limitado ao monitor onde ele vai aparecer.
fn request_preferred_size(window: &WebviewWindow, monitor: &Monitor) -> Result<(), AppError> {
    let size = clamp_size(
        preferred_size(window.app_handle()),
        max_size(work_area_logical(monitor)),
    );
    request_size(window, size)
}

fn request_size(window: &WebviewWindow, size: PopupSize) -> Result<(), AppError> {
    window
        .app_handle()
        .state::<PopupState>()
        .with_size(|state| state.requested = Some(size));
    window
        .set_size(LogicalSize::new(
            f64::from(size.width),
            f64::from(size.height),
        ))
        .map_err(window_error)
}

/// Depois de crescer, a janela pode passar da borda do monitor: puxa-a de volta.
fn keep_inside_work_area(window: &WebviewWindow, monitor: &Monitor) -> Result<(), AppError> {
    let position = window.outer_position().map_err(window_error)?;
    let size = window.outer_size().map_err(window_error)?;
    let current = Point {
        x: position.x,
        y: position.y,
    };
    let kept = clamp_into_area(
        current,
        Size {
            width: size.width,
            height: size.height,
        },
        work_rect(monitor),
    );
    if kept == current {
        return Ok(());
    }
    window
        .set_position(PhysicalPosition::new(kept.x, kept.y))
        .map_err(window_error)
}

/// Um `Resized` que não foi pedido pelo app é o usuário arrastando as bordas: guarda o tamanho
/// (a gravação em disco só acontece ao ocultar, não a cada pixel arrastado).
fn remember_user_resize(window: &Window, resized: PhysicalSize<u32>) {
    if resized.width == 0 || resized.height == 0 {
        return;
    }
    let state = window.state::<PopupState>();
    if !window.is_visible().unwrap_or(false) || state.just_shown() {
        return;
    }
    let Ok(scale) = window.scale_factor() else {
        return;
    };
    let resized = logical_size(resized, scale);
    let previous = preferred_size(window.app_handle());
    state.with_size(|size| {
        if is_echo(resized, size.requested) {
            return;
        }
        size.preferred = Some(preferred_after_user_resize(
            previous,
            size.requested,
            resized,
        ));
        size.dirty = true;
    });
}

/// Grava o tamanho escolhido pelo usuário, se mudou. Chamada ao ocultar o popup.
fn save_size_if_changed(app: &AppHandle) {
    let state = app.state::<PopupState>();
    let Some(preferred) = state.with_size(|size| size.preferred.filter(|_| size.dirty)) else {
        return;
    };
    let Some(app_state) = app.try_state::<AppState>() else {
        return;
    };
    match app_state.settings.update_popup_size(preferred) {
        Ok(_) => state.with_size(|size| size.dirty = false),
        Err(error) => log::warn!("não foi possível lembrar o tamanho do popup: {error}"),
    }
}

fn work_rect(monitor: &Monitor) -> Rect {
    let work_area = monitor.work_area();
    Rect {
        origin: Point {
            x: work_area.position.x,
            y: work_area.position.y,
        },
        size: Size {
            width: work_area.size.width,
            height: work_area.size.height,
        },
    }
}

fn work_area_logical(monitor: &Monitor) -> PopupSize {
    logical_size(monitor.work_area().size, monitor.scale_factor())
}

fn logical_size(physical: PhysicalSize<u32>, scale: f64) -> PopupSize {
    PopupSize {
        width: (f64::from(physical.width) / scale).round() as u32,
        height: (f64::from(physical.height) / scale).round() as u32,
    }
}

/// Um blur dentro de `BLUR_GRACE` é ignorado e nunca reemitido. Se o Windows recusar o foco,
/// o popup (always-on-top) ficaria preso na tela; esta verificação final o oculta.
fn hide_if_still_unfocused(window: WebviewWindow) {
    thread::spawn(move || {
        thread::sleep(BLUR_GRACE);

        let state = window.app_handle().state::<PopupState>();
        if state.is_pinned() {
            return;
        }
        let shown_at = state.shown_at();
        match window.is_focused() {
            Ok(is_focused) => {
                if should_hide_unfocused(is_focused, shown_at, Instant::now()) {
                    prepare_to_hide(window.app_handle());
                    if let Err(error) = window.hide() {
                        log::warn!("não foi possível ocultar o popup sem foco: {error}");
                    }
                }
            }
            Err(error) => log::warn!("não foi possível consultar o foco do popup: {error}"),
        }
    });
}

/// Oculta o popup ao perder o foco (a menos que esteja fixado) e impede que seja destruído ao
/// ser "fechado".
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != POPUP_LABEL {
        return;
    }

    match event {
        WindowEvent::Focused(false) => {
            let state = window.state::<PopupState>();
            if !state.is_pinned()
                && !state.is_interacting()
                && should_hide_on_blur(state.shown_at(), Instant::now())
            {
                hide(window);
            }
        }
        WindowEvent::Resized(size) => remember_user_resize(window, *size),
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(window);
        }
        _ => {}
    }
}

fn hide(window: &Window) {
    prepare_to_hide(window.app_handle());
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
            auto_translate: true,
            pinned: false,
            notice: None,
        })
        .unwrap();

        assert_eq!(
            json,
            r#"{"prefill":"olá mundo","autoTranslate":true,"pinned":false,"notice":null}"#
        );
    }

    #[test]
    fn reset_payload_without_capture_serializes_prefill_as_null() {
        let json = serde_json::to_string(&ResetPayload {
            prefill: None,
            auto_translate: false,
            pinned: false,
            notice: None,
        })
        .unwrap();

        assert_eq!(
            json,
            r#"{"prefill":null,"autoTranslate":false,"pinned":false,"notice":null}"#
        );
    }

    #[test]
    fn reset_payload_tells_the_front_the_popup_is_still_pinned() {
        let json = serde_json::to_value(ResetPayload {
            prefill: Some("hello"),
            auto_translate: true,
            pinned: true,
            notice: None,
        })
        .unwrap();

        assert_eq!(json["pinned"], serde_json::json!(true));
    }

    #[test]
    fn reset_payload_carries_a_notice_for_the_user() {
        let json = serde_json::to_value(ResetPayload {
            prefill: None,
            auto_translate: true,
            pinned: false,
            notice: Some("A captura da tela veio preta."),
        })
        .unwrap();

        assert_eq!(
            json["notice"],
            serde_json::json!("A captura da tela veio preta.")
        );
    }

    #[test]
    fn a_physical_size_is_converted_to_logical_pixels_by_the_scale() {
        let logical = logical_size(
            PhysicalSize {
                width: 630,
                height: 300,
            },
            1.5,
        );

        assert_eq!(
            logical,
            PopupSize {
                width: 420,
                height: 200
            }
        );
    }
}
