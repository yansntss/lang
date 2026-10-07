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

fn place_axis(cursor: i32, window_len: u32, area_start: i32, area_len: u32, offset: i32) -> i32 {
    let window_len = i64::from(window_len);
    let area_end = i64::from(area_start) + i64::from(area_len);
    let after_cursor = i64::from(cursor) + i64::from(offset);

    let start = if after_cursor + window_len > area_end {
        i64::from(cursor) - i64::from(offset) - window_len
    } else {
        after_cursor
    };

    let latest_start = (area_end - window_len).max(i64::from(area_start));
    let clamped = start.clamp(i64::from(area_start), latest_start);

    i32::try_from(clamped).unwrap_or(area_start)
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
