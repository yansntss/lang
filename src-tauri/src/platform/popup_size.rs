//! Tamanho do popup: o que o usuário escolheu arrastando as bordas e o quanto a janela cresce
//! para caber o conteúdo. Tudo em pixels lógicos e sem tocar no sistema, para ser testável.

use crate::settings::PopupSize;

pub const DEFAULT_SIZE: PopupSize = PopupSize {
    width: 420,
    height: 200,
};
pub const MIN_WIDTH: u32 = 320;
pub const MIN_HEIGHT: u32 = 140;
/// Parte da área útil do monitor que o popup pode ocupar, para nunca cobrir a tela inteira.
const MAX_AREA_PERCENT: u32 = 90;
/// Diferença (em pixels lógicos) abaixo da qual um tamanho é considerado o mesmo: a conversão
/// entre pixels lógicos e físicos arredonda.
pub const SIZE_TOLERANCE: u32 = 2;

/// O maior tamanho permitido numa área útil de `area` pixels lógicos.
pub fn max_size(area: PopupSize) -> PopupSize {
    PopupSize {
        width: percent_of(area.width).max(MIN_WIDTH),
        height: percent_of(area.height).max(MIN_HEIGHT),
    }
}

/// Multiplica antes de dividir: dividir primeiro perderia até 99 pixels no arredondamento.
fn percent_of(length: u32) -> u32 {
    length.saturating_mul(MAX_AREA_PERCENT) / 100
}

/// Limita `size` ao intervalo permitido: um tamanho salvo num monitor grande não pode estourar
/// um monitor menor, nem um arquivo editado à mão deixar o popup minúsculo.
pub fn clamp_size(size: PopupSize, max: PopupSize) -> PopupSize {
    PopupSize {
        width: size.width.min(max.width).max(MIN_WIDTH),
        height: size.height.min(max.height).max(MIN_HEIGHT),
    }
}

/// Altura da janela: a escolhida pelo usuário, ou a que o conteúdo pede se for maior, até `max`.
pub fn fit_height(preferred: u32, content: u32, max: u32) -> u32 {
    preferred.max(content).min(max).max(MIN_HEIGHT)
}

fn is_close(a: u32, b: u32) -> bool {
    a.abs_diff(b) <= SIZE_TOLERANCE
}

/// Os dois tamanhos são o mesmo, a menos do arredondamento entre pixels lógicos e físicos.
pub fn same_size(a: PopupSize, b: PopupSize) -> bool {
    is_close(a.width, b.width) && is_close(a.height, b.height)
}

/// Um redimensionamento que o próprio app acabou de pedir (e não o usuário arrastando).
pub fn is_echo(resized: PopupSize, requested: Option<PopupSize>) -> bool {
    requested.is_some_and(|requested| same_size(resized, requested))
}

/// O tamanho a lembrar depois de o usuário redimensionar. Se só a largura mudou, a altura da
/// janela é a que o conteúdo pediu (crescimento automático) e não uma escolha do usuário.
pub fn preferred_after_user_resize(
    previous: PopupSize,
    requested: Option<PopupSize>,
    resized: PopupSize,
) -> PopupSize {
    let height_is_automatic =
        requested.is_some_and(|requested| is_close(resized.height, requested.height));
    PopupSize {
        width: resized.width.max(MIN_WIDTH),
        height: if height_is_automatic {
            previous.height
        } else {
            resized.height.max(MIN_HEIGHT)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> PopupSize {
        PopupSize { width, height }
    }

    #[test]
    fn the_maximum_is_a_share_of_the_work_area() {
        assert_eq!(max_size(size(1920, 1040)), size(1728, 936));
    }

    #[test]
    fn the_maximum_never_drops_below_the_minimum() {
        assert_eq!(max_size(size(200, 100)), size(MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn a_saved_size_larger_than_the_screen_is_cut_to_the_maximum() {
        let clamped = clamp_size(size(4000, 3000), size(1728, 936));

        assert_eq!(clamped, size(1728, 936));
    }

    #[test]
    fn a_size_smaller_than_the_minimum_is_raised() {
        let clamped = clamp_size(size(10, 10), size(1728, 936));

        assert_eq!(clamped, size(MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn a_size_inside_the_limits_is_kept() {
        assert_eq!(clamp_size(size(500, 300), size(1728, 936)), size(500, 300));
    }

    #[test]
    fn the_window_grows_to_fit_content_taller_than_the_chosen_height() {
        assert_eq!(fit_height(200, 340, 900), 340);
    }

    #[test]
    fn the_window_goes_back_to_the_chosen_height_when_the_content_shrinks() {
        assert_eq!(fit_height(200, 120, 900), 200);
    }

    #[test]
    fn the_window_stops_growing_at_the_maximum() {
        assert_eq!(fit_height(200, 5000, 900), 900);
    }

    #[test]
    fn a_resize_matching_the_requested_size_is_an_echo() {
        assert!(is_echo(size(420, 341), Some(size(420, 340))));
    }

    #[test]
    fn a_resize_far_from_the_requested_size_is_the_user() {
        assert!(!is_echo(size(520, 340), Some(size(420, 340))));
    }

    #[test]
    fn without_a_request_every_resize_is_the_user() {
        assert!(!is_echo(size(420, 200), None));
    }

    #[test]
    fn dragging_the_height_saves_the_new_height() {
        let saved =
            preferred_after_user_resize(size(420, 200), Some(size(420, 340)), size(420, 260));

        assert_eq!(saved, size(420, 260));
    }

    #[test]
    fn dragging_only_the_width_keeps_the_chosen_height_and_not_the_grown_one() {
        let saved =
            preferred_after_user_resize(size(420, 200), Some(size(420, 340)), size(560, 340));

        assert_eq!(saved, size(560, 200));
    }

    #[test]
    fn a_dragged_size_below_the_minimum_is_saved_as_the_minimum() {
        let saved = preferred_after_user_resize(size(420, 200), None, size(100, 50));

        assert_eq!(saved, size(MIN_WIDTH, MIN_HEIGHT));
    }
}
