//! Fotografia de um monitor inteiro, para a seleção de região. A imagem fica só na memória:
//! nunca vai para disco nem para o log.

use crate::error::AppError;

/// Pixels de um monitor em RGBA, linha a linha, em pixels físicos.
pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Bytes do cabeçalho de [`Snapshot::encode`]: largura e altura.
const HEADER_LEN: usize = 8;
/// Um canal até este valor ainda conta como preto (ruído de compressão, gama).
const BLANK_CHANNEL_MAX: u8 = 16;

impl Snapshot {
    /// O formato que a janela de seleção recebe: largura e altura (`u32` little-endian)
    /// seguidas dos pixels RGBA. Bytes crus evitam codificar PNG a cada atalho.
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(HEADER_LEN + self.rgba.len());
        encoded.extend_from_slice(&self.width.to_le_bytes());
        encoded.extend_from_slice(&self.height.to_le_bytes());
        encoded.extend_from_slice(&self.rgba);
        encoded
    }

    /// Toda preta: é o que o Windows entrega de um jogo em tela cheia exclusiva. Para no
    /// primeiro pixel aceso, então uma tela comum é decidida logo no início.
    pub fn is_blank(&self) -> bool {
        let (pixels, _) = self.rgba.as_chunks::<4>();
        pixels.iter().all(|[red, green, blue, _]| {
            [red, green, blue].iter().all(|&&c| c <= BLANK_CHANNEL_MAX)
        })
    }
}

/// Fotografa o monitor que contém o ponto (pixels físicos da tela).
#[cfg(any(windows, target_os = "macos"))]
pub fn capture_monitor_at(x: i32, y: i32) -> Result<Snapshot, AppError> {
    let capture_error = |error: xcap::XCapError| AppError::ScreenCapture(error.to_string());
    let monitor = xcap::Monitor::from_point(x, y).map_err(capture_error)?;
    let image = monitor.capture_image().map_err(capture_error)?;
    Ok(Snapshot {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

/// No Linux a captura ainda não existe (X11 e Wayland pedem caminhos diferentes).
#[cfg(not(any(windows, target_os = "macos")))]
pub fn capture_monitor_at(_x: i32, _y: i32) -> Result<Snapshot, AppError> {
    Err(AppError::ScreenCapture(
        "captura de tela não suportada neste sistema".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(width: u32, height: u32, rgba: Vec<u8>) -> Snapshot {
        Snapshot {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn the_encoding_starts_with_width_and_height_then_the_pixels() {
        let pixels = vec![1, 2, 3, 255, 4, 5, 6, 255];

        let encoded = snapshot(2, 1, pixels.clone()).encode();

        assert_eq!(&encoded[..4], &2u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &1u32.to_le_bytes());
        assert_eq!(&encoded[8..], pixels.as_slice());
    }

    #[test]
    fn large_dimensions_survive_the_encoding() {
        let encoded = snapshot(3840, 2160, Vec::new()).encode();

        assert_eq!(u32::from_le_bytes(encoded[..4].try_into().unwrap()), 3840);
        assert_eq!(u32::from_le_bytes(encoded[4..8].try_into().unwrap()), 2160);
    }

    #[test]
    fn an_all_black_capture_is_blank() {
        let black = [0, 0, 0, 255].repeat(100);
        let almost_black = [10, 16, 3, 255].repeat(100);

        assert!(snapshot(10, 10, black).is_blank());
        assert!(snapshot(10, 10, almost_black).is_blank());
    }

    #[test]
    fn a_single_lit_pixel_means_the_capture_is_not_blank() {
        let mut pixels = [0, 0, 0, 255].repeat(100);
        pixels[4 * 57] = 200;

        assert!(!snapshot(10, 10, pixels).is_blank());
    }

    #[test]
    fn a_dark_scene_with_some_light_is_not_blank() {
        let scene = [20, 20, 30, 255].repeat(100);

        assert!(!snapshot(10, 10, scene).is_blank());
    }

    #[test]
    fn the_alpha_channel_does_not_count_as_light() {
        let transparent_black = [0, 0, 0, 0].repeat(4);

        assert!(snapshot(2, 2, transparent_black).is_blank());
    }
}
