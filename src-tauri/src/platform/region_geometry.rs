//! Geometria da seleção de região: converte o retângulo que o usuário arrastou (pixels lógicos
//! da janela de seleção) em pixels físicos da tela, e posiciona o popup ao lado da região.

use serde::Deserialize;

use super::placement::{clamp_into_area, Point, Rect, Size};
use crate::error::AppError;

/// Menor lado aceito, em pixels físicos: abaixo disso não cabe texto legível.
pub const MIN_REGION_SIDE: u32 = 8;

/// Retângulo arrastado na janela de seleção, em pixels lógicos (CSS) relativos a ela. A largura
/// e a altura ficam negativas quando o arrasto vai para cima ou para a esquerda.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct LogicalSelection {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// O monitor fotografado: a área em pixels físicos da tela e a escala (DPI) dele.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorGeometry {
    pub area: Rect,
    pub scale: f64,
}

/// Converte a seleção em um retângulo físico em coordenadas da tela. Recorta o que passou das
/// bordas do monitor e recusa uma seleção pequena demais para ter texto.
pub fn selection_to_screen(
    selection: LogicalSelection,
    monitor: MonitorGeometry,
) -> Result<Rect, AppError> {
    let LogicalSelection {
        x,
        y,
        width,
        height,
    } = selection;
    let scale = monitor.scale;
    if ![x, y, width, height].iter().all(|value| value.is_finite())
        || !scale.is_finite()
        || scale <= 0.0
    {
        return Err(AppError::InvalidInput("Seleção inválida.".into()));
    }

    let (left, right) = physical_span(x, width, scale, monitor.area.size.width);
    let (top, bottom) = physical_span(y, height, scale, monitor.area.size.height);
    let size = Size {
        width: right - left,
        height: bottom - top,
    };
    if size.width < MIN_REGION_SIDE || size.height < MIN_REGION_SIDE {
        return Err(AppError::RegionEmpty);
    }

    Ok(Rect {
        origin: Point {
            x: offset(monitor.area.origin.x, left),
            y: offset(monitor.area.origin.y, top),
        },
        size,
    })
}

/// Um eixo da seleção em pixels físicos relativos ao monitor, já recortado a ele. As bordas
/// fracionárias crescem para fora (piso no início, teto no fim) para não cortar letras.
fn physical_span(start: f64, length: f64, scale: f64, monitor_length: u32) -> (u32, u32) {
    let (low, high) = if length < 0.0 {
        (start + length, start)
    } else {
        (start, start + length)
    };
    let limit = f64::from(monitor_length);
    // Depois do `clamp` o valor cabe em `u32` (de 0 até o tamanho do monitor).
    let to_pixel = |value: f64| value.clamp(0.0, limit) as u32;
    (
        to_pixel((low * scale).floor()),
        to_pixel((high * scale).ceil()),
    )
}

fn offset(origin: i32, distance: u32) -> i32 {
    origin.saturating_add(i32::try_from(distance).unwrap_or(i32::MAX))
}

/// A região (pixels físicos da tela) em pixels da captura do monitor. Em geral os dois são o
/// mesmo; se a captura veio em outra resolução, a região é reescalada para ela.
pub fn region_in_snapshot(region: Rect, monitor: Rect, snapshot: Size) -> Rect {
    let axis = |position: i32, length: u32, origin: i32, monitor_length: u32, image: u32| {
        let ratio = if monitor_length == 0 {
            1.0
        } else {
            f64::from(image) / f64::from(monitor_length)
        };
        let start = (f64::from(position) - f64::from(origin)) * ratio;
        let scaled = f64::from(length) * ratio;
        // Valores dentro da captura (ou perto dela); `crop_gray` recorta o que sobrar.
        (start.round() as i32, scaled.round() as u32)
    };
    let (x, width) = axis(
        region.origin.x,
        region.size.width,
        monitor.origin.x,
        monitor.size.width,
        snapshot.width,
    );
    let (y, height) = axis(
        region.origin.y,
        region.size.height,
        monitor.origin.y,
        monitor.size.height,
        snapshot.height,
    );
    Rect {
        origin: Point { x, y },
        size: Size { width, height },
    }
}

/// O centro do retângulo (arredondado para baixo).
pub fn center_of(rect: Rect) -> Point {
    let half = |length: u32| i32::try_from(length / 2).unwrap_or(i32::MAX);
    Point {
        x: rect.origin.x.saturating_add(half(rect.size.width)),
        y: rect.origin.y.saturating_add(half(rect.size.height)),
    }
}

/// Posiciona o popup sem cobrir a região: abaixo dela, senão acima, senão à direita, senão à
/// esquerda. Sem espaço em nenhum lado, fica abaixo, limitado à área do monitor.
pub fn place_beside_region(region: Rect, window: Size, area: Rect, gap: i32) -> Point {
    let (gap, width, height) = (
        i64::from(gap),
        i64::from(window.width),
        i64::from(window.height),
    );
    let (region_left, region_top) = (i64::from(region.origin.x), i64::from(region.origin.y));
    let region_right = region_left + i64::from(region.size.width);
    let region_bottom = region_top + i64::from(region.size.height);
    let (area_left, area_top) = (i64::from(area.origin.x), i64::from(area.origin.y));
    let area_right = area_left + i64::from(area.size.width);
    let area_bottom = area_top + i64::from(area.size.height);

    let below = region_bottom + gap;
    let above = region_top - gap - height;
    let right = region_right + gap;
    let left = region_left - gap - width;
    let (x, y) = if below + height <= area_bottom {
        (region_left, below)
    } else if above >= area_top {
        (region_left, above)
    } else if right + width <= area_right {
        (right, region_top)
    } else if left >= area_left {
        (left, region_top)
    } else {
        (region_left, below)
    };

    // `clamp_into_area` traz o ponto para dentro da área, que cabe em `i32`.
    let to_i32 =
        |value: i64| i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX });
    clamp_into_area(
        Point {
            x: to_i32(x),
            y: to_i32(y),
        },
        window,
        area,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    fn monitor(x: i32, y: i32, width: u32, height: u32, scale: f64) -> MonitorGeometry {
        MonitorGeometry {
            area: rect(x, y, width, height),
            scale,
        }
    }

    fn selection(x: f64, y: f64, width: f64, height: f64) -> LogicalSelection {
        LogicalSelection {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn at_100_percent_logical_and_physical_pixels_are_the_same() {
        let screen = selection_to_screen(
            selection(100.0, 50.0, 300.0, 40.0),
            monitor(0, 0, 1920, 1080, 1.0),
        )
        .unwrap();

        assert_eq!(screen, rect(100, 50, 300, 40));
    }

    #[test]
    fn the_monitor_scale_turns_logical_into_physical_pixels() {
        for (scale, expected) in [
            (1.25, rect(125, 62, 375, 51)),
            (1.5, rect(150, 75, 450, 60)),
            (2.0, rect(200, 100, 600, 80)),
        ] {
            let screen = selection_to_screen(
                selection(100.0, 50.0, 300.0, 40.0),
                monitor(0, 0, 3840, 2160, scale),
            )
            .unwrap();

            assert_eq!(screen, expected, "escala {scale}");
        }
    }

    #[test]
    fn fractional_edges_grow_outward_so_no_text_is_cut() {
        let screen = selection_to_screen(
            selection(10.3, 10.3, 20.2, 20.2),
            monitor(0, 0, 1920, 1080, 1.0),
        )
        .unwrap();

        assert_eq!(screen, rect(10, 10, 21, 21));
    }

    #[test]
    fn a_selection_on_a_monitor_left_of_the_primary_uses_its_negative_origin() {
        let screen = selection_to_screen(
            selection(100.0, 100.0, 200.0, 50.0),
            monitor(-2560, 0, 2560, 1440, 1.25),
        )
        .unwrap();

        assert_eq!(screen, rect(-2560 + 125, 125, 250, 63));
    }

    #[test]
    fn a_selection_on_a_monitor_above_the_primary_uses_its_negative_origin() {
        let screen = selection_to_screen(
            selection(0.0, 0.0, 100.0, 100.0),
            monitor(0, -1080, 1920, 1080, 1.0),
        )
        .unwrap();

        assert_eq!(screen, rect(0, -1080, 100, 100));
    }

    #[test]
    fn dragging_up_and_left_gives_the_same_region_as_down_and_right() {
        let forward = selection_to_screen(
            selection(100.0, 100.0, 200.0, 50.0),
            monitor(0, 0, 1920, 1080, 1.5),
        )
        .unwrap();
        let backward = selection_to_screen(
            selection(300.0, 150.0, -200.0, -50.0),
            monitor(0, 0, 1920, 1080, 1.5),
        )
        .unwrap();

        assert_eq!(forward, backward);
    }

    #[test]
    fn the_part_outside_the_monitor_is_cut_off() {
        let screen = selection_to_screen(
            selection(-50.0, 1000.0, 150.0, 200.0),
            monitor(0, 0, 1920, 1080, 1.0),
        )
        .unwrap();

        assert_eq!(screen, rect(0, 1000, 100, 80));
    }

    #[test]
    fn a_selection_too_small_to_hold_text_is_refused() {
        for (width, height) in [(0.0, 0.0), (300.0, 3.0), (3.0, 300.0)] {
            let error = selection_to_screen(
                selection(10.0, 10.0, width, height),
                monitor(0, 0, 1920, 1080, 1.0),
            )
            .unwrap_err();

            assert_eq!(error.code(), "region_empty", "{width}x{height}");
        }
    }

    #[test]
    fn a_selection_entirely_outside_the_monitor_is_refused() {
        let error = selection_to_screen(
            selection(2000.0, 10.0, 100.0, 100.0),
            monitor(0, 0, 1920, 1080, 1.0),
        )
        .unwrap_err();

        assert_eq!(error.code(), "region_empty");
    }

    #[test]
    fn numbers_that_are_not_finite_are_refused() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = selection_to_screen(
                selection(bad, 10.0, 100.0, 100.0),
                monitor(0, 0, 1920, 1080, 1.0),
            )
            .unwrap_err();

            assert_eq!(error.code(), "invalid_input");
        }
        let error = selection_to_screen(
            selection(10.0, 10.0, 100.0, 100.0),
            monitor(0, 0, 1920, 1080, f64::NAN),
        )
        .unwrap_err();

        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn a_region_becomes_relative_to_the_monitor_capture() {
        let monitor = rect(-1920, 0, 1920, 1080);

        let inside = region_in_snapshot(
            rect(-1820, 900, 600, 80),
            monitor,
            Size {
                width: 1920,
                height: 1080,
            },
        );

        assert_eq!(inside, rect(100, 900, 600, 80));
    }

    #[test]
    fn a_capture_in_another_resolution_gets_the_region_rescaled() {
        let monitor = rect(0, 0, 3840, 2160);

        let inside = region_in_snapshot(
            rect(200, 100, 600, 80),
            monitor,
            Size {
                width: 1920,
                height: 1080,
            },
        );

        assert_eq!(inside, rect(100, 50, 300, 40));
    }

    #[test]
    fn the_center_works_with_negative_origins_and_odd_sizes() {
        assert_eq!(center_of(rect(0, 0, 1920, 1080)), Point { x: 960, y: 540 });
        assert_eq!(
            center_of(rect(-1920, -1080, 1921, 3)),
            Point { x: -960, y: -1079 }
        );
    }

    const POPUP: Size = Size {
        width: 420,
        height: 200,
    };
    const GAP: i32 = 12;

    fn full_hd() -> Rect {
        rect(0, 0, 1920, 1080)
    }

    #[test]
    fn the_popup_goes_below_the_region_when_it_fits() {
        let placed = place_beside_region(rect(100, 100, 600, 80), POPUP, full_hd(), GAP);

        assert_eq!(placed, Point { x: 100, y: 192 });
    }

    #[test]
    fn subtitles_at_the_bottom_put_the_popup_above_them() {
        let subtitles = rect(400, 900, 1100, 120);

        let placed = place_beside_region(subtitles, POPUP, full_hd(), GAP);

        assert_eq!(
            placed,
            Point {
                x: 400,
                y: 900 - GAP - 200
            }
        );
    }

    #[test]
    fn a_region_as_tall_as_the_screen_puts_the_popup_beside_it() {
        let tall = rect(100, 0, 600, 1080);

        let placed = place_beside_region(tall, POPUP, full_hd(), GAP);

        assert_eq!(placed, Point { x: 712, y: 0 });
    }

    #[test]
    fn without_room_on_the_right_the_popup_goes_to_the_left() {
        let tall = rect(1300, 0, 600, 1080);

        let placed = place_beside_region(tall, POPUP, full_hd(), GAP);

        assert_eq!(
            placed,
            Point {
                x: 1300 - GAP - 420,
                y: 0
            }
        );
    }

    #[test]
    fn a_region_near_the_right_edge_keeps_the_popup_inside_the_screen() {
        let placed = place_beside_region(rect(1700, 100, 200, 50), POPUP, full_hd(), GAP);

        assert_eq!(
            placed,
            Point {
                x: 1920 - 420,
                y: 162
            }
        );
    }

    #[test]
    fn a_region_covering_the_whole_screen_still_gets_a_popup_inside_it() {
        let placed = place_beside_region(full_hd(), POPUP, full_hd(), GAP);

        assert_eq!(
            placed,
            Point {
                x: 0,
                y: 1080 - 200
            }
        );
    }

    #[test]
    fn works_on_a_monitor_with_a_negative_origin() {
        let left = rect(-1920, 0, 1920, 1080);

        let placed = place_beside_region(rect(-1800, 100, 500, 60), POPUP, left, GAP);

        assert_eq!(placed, Point { x: -1800, y: 172 });
    }
}
