use std::time::{Duration, Instant};

/// Depois de exibir o popup, perder o foco dentro deste intervalo é ignorado: o Windows pode
/// emitir um blur transitório enquanto a janela ainda está ganhando o foco.
pub const BLUR_GRACE: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

/// Posiciona a janela logo abaixo e à direita do cursor. Se estourar a área do monitor num
/// eixo, passa para o outro lado do cursor naquele eixo; se ainda assim não couber, é
/// limitada à área (ou ao canto de origem, quando a janela é maior que a área).
pub fn place_near_cursor(cursor: Point, window: Size, area: Rect, offset: i32) -> Point {
    Point {
        x: place_axis(
            cursor.x,
            window.width,
            area.origin.x,
            area.size.width,
            offset,
        ),
        y: place_axis(
            cursor.y,
            window.height,
            area.origin.y,
            area.size.height,
            offset,
        ),
    }
}

/// Mantém a janela inteira dentro da área; se for maior que ela, fica no canto de origem.
/// Usada quando a janela cresce e poderia passar da borda do monitor.
pub fn clamp_into_area(origin: Point, window: Size, area: Rect) -> Point {
    Point {
        x: clamp_axis(
            i64::from(origin.x),
            window.width,
            area.origin.x,
            area.size.width,
        ),
        y: clamp_axis(
            i64::from(origin.y),
            window.height,
            area.origin.y,
            area.size.height,
        ),
    }
}

fn place_axis(cursor: i32, window_len: u32, area_start: i32, area_len: u32, offset: i32) -> i32 {
    let area_end = i64::from(area_start) + i64::from(area_len);
    let after_cursor = i64::from(cursor) + i64::from(offset);

    let start = if after_cursor + i64::from(window_len) > area_end {
        i64::from(cursor) - i64::from(offset) - i64::from(window_len)
    } else {
        after_cursor
    };

    clamp_axis(start, window_len, area_start, area_len)
}

fn clamp_axis(start: i64, window_len: u32, area_start: i32, area_len: u32) -> i32 {
    let area_end = i64::from(area_start) + i64::from(area_len);
    let latest_start = (area_end - i64::from(window_len)).max(i64::from(area_start));
    let clamped = start.clamp(i64::from(area_start), latest_start);

    i32::try_from(clamped).unwrap_or(area_start)
}

/// Verificação ao fim da tolerância: o blur ignorado nunca é reemitido, então, se a janela
/// não ganhou foco, ela precisa ser ocultada aqui.
pub fn should_hide_unfocused(is_focused: bool, shown_at: Option<Instant>, now: Instant) -> bool {
    !is_focused && should_hide_on_blur(shown_at, now)
}

/// Decide se um evento de perda de foco deve ocultar o popup.
pub fn should_hide_on_blur(shown_at: Option<Instant>, now: Instant) -> bool {
    match shown_at {
        None => true,
        Some(shown) => now.saturating_duration_since(shown) >= BLUR_GRACE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Size = Size {
        width: 420,
        height: 200,
    };
    const OFFSET: i32 = 12;

    fn area(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    fn at(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    fn full_hd() -> Rect {
        area(0, 0, 1920, 1080)
    }

    #[test]
    fn places_the_window_below_and_right_of_the_cursor_when_it_fits() {
        let placed = place_near_cursor(at(100, 100), WINDOW, full_hd(), OFFSET);

        assert_eq!(placed, at(112, 112));
    }

    #[test]
    fn flips_to_the_left_of_the_cursor_near_the_right_edge() {
        let placed = place_near_cursor(at(1800, 100), WINDOW, full_hd(), OFFSET);

        assert_eq!(placed, at(1800 - OFFSET - 420, 112));
    }

    #[test]
    fn flips_above_the_cursor_near_the_bottom_edge() {
        let placed = place_near_cursor(at(100, 1000), WINDOW, full_hd(), OFFSET);

        assert_eq!(placed, at(112, 1000 - OFFSET - 200));
    }

    #[test]
    fn flips_both_axes_in_the_bottom_right_corner() {
        let placed = place_near_cursor(at(1800, 1000), WINDOW, full_hd(), OFFSET);

        assert_eq!(placed, at(1368, 788));
    }

    #[test]
    fn clamps_to_the_monitor_when_flipping_still_overflows() {
        let narrow = area(0, 0, 300, 1080);

        let placed = place_near_cursor(at(100, 100), WINDOW, narrow, OFFSET);

        assert_eq!(placed, at(0, 112));
    }

    #[test]
    fn works_on_a_monitor_with_a_negative_origin() {
        let left_monitor = area(-1920, 0, 1920, 1080);

        let placed = place_near_cursor(at(-100, 500), WINDOW, left_monitor, OFFSET);

        assert_eq!(placed, at(-532, 512));
    }

    #[test]
    fn pins_to_the_monitor_origin_when_the_window_is_larger_than_the_monitor() {
        let tiny = area(0, 0, 400, 150);

        let placed = place_near_cursor(at(50, 50), WINDOW, tiny, OFFSET);

        assert_eq!(placed, at(0, 0));
    }

    #[test]
    fn respects_a_monitor_that_does_not_start_at_the_origin_vertically() {
        let above = area(0, -1080, 1920, 1080);

        let placed = place_near_cursor(at(100, -50), WINDOW, above, OFFSET);

        assert_eq!(placed, at(112, -50 - OFFSET - 200));
    }

    #[test]
    fn a_window_that_grew_past_the_bottom_edge_is_pulled_back_up() {
        let grown = Size {
            width: 420,
            height: 600,
        };

        let moved = clamp_into_area(at(112, 700), grown, full_hd());

        assert_eq!(moved, at(112, 480));
    }

    #[test]
    fn a_window_already_inside_the_area_stays_where_it_is() {
        let moved = clamp_into_area(at(112, 112), WINDOW, full_hd());

        assert_eq!(moved, at(112, 112));
    }

    #[test]
    fn a_window_larger_than_the_area_is_pinned_to_its_origin() {
        let tiny = area(10, 20, 300, 100);

        let moved = clamp_into_area(at(50, 50), WINDOW, tiny);

        assert_eq!(moved, at(10, 20));
    }

    #[test]
    fn keeps_a_focused_popup_open_at_the_end_of_the_grace_period() {
        let shown = Instant::now();

        assert!(!should_hide_unfocused(
            true,
            Some(shown),
            shown + BLUR_GRACE
        ));
    }

    #[test]
    fn hides_an_unfocused_popup_at_the_end_of_the_grace_period() {
        let shown = Instant::now();

        assert!(should_hide_unfocused(
            false,
            Some(shown),
            shown + BLUR_GRACE
        ));
    }

    #[test]
    fn does_not_hide_a_popup_that_was_shown_again_during_the_check() {
        let shown_again = Instant::now();

        assert!(!should_hide_unfocused(
            false,
            Some(shown_again),
            shown_again + Duration::from_millis(50)
        ));
    }

    #[test]
    fn hides_on_blur_when_the_popup_was_never_shown() {
        assert!(should_hide_on_blur(None, Instant::now()));
    }

    #[test]
    fn ignores_a_blur_right_after_the_popup_was_shown() {
        let shown = Instant::now();

        assert!(!should_hide_on_blur(
            Some(shown),
            shown + Duration::from_millis(50)
        ));
    }

    #[test]
    fn hides_on_blur_once_the_grace_period_has_passed() {
        let shown = Instant::now();

        assert!(should_hide_on_blur(Some(shown), shown + BLUR_GRACE));
        assert!(should_hide_on_blur(
            Some(shown),
            shown + Duration::from_secs(5)
        ));
    }
}
