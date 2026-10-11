//! Fotografia de um monitor inteiro, para a seleção de região. A imagem fica só na memória:
//! nunca vai para disco nem para o log.

use super::placement::{Rect, Size};
use super::region_geometry::region_in_snapshot;
use crate::error::AppError;
use crate::services::ocr::GrayImage;

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

    /// O trecho `rect` (pixels da captura) em escala de cinza, para o OCR. O que passar das
    /// bordas da captura é descartado.
    pub fn crop_gray(&self, rect: Rect) -> GrayImage {
        let span = |start: i32, length: u32, limit: u32| {
            let first = u32::try_from(start.max(0)).unwrap_or(0).min(limit);
            let end = i64::from(start)
                .saturating_add(i64::from(length))
                .clamp(0, i64::from(limit));
            let last = u32::try_from(end).unwrap_or(limit);
            (first, last.saturating_sub(first))
        };
        let (left, width) = span(rect.origin.x, rect.size.width, self.width);
        let (top, height) = span(rect.origin.y, rect.size.height, self.height);

        GrayImage::from_fn(width, height, |x, y| {
            // Dentro da captura por construção: `left + x < self.width`, `top + y < self.height`.
            let index = ((top + y) as usize * self.width as usize + (left + x) as usize) * 4;
            let pixel = self.rgba.get(index..index + 3).unwrap_or(&[0, 0, 0]);
            image::Luma([luma(pixel[0], pixel[1], pixel[2])])
        })
    }

    /// A `region` (pixels físicos da tela) em cinza. `monitor` é a área do monitor fotografado.
    pub fn crop_region(&self, region: Rect, monitor: Rect) -> GrayImage {
        let size = Size {
            width: self.width,
            height: self.height,
        };
        self.crop_gray(region_in_snapshot(region, monitor, size))
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

/// Brilho percebido (pesos da ITU-R BT.601), o que o OCR enxerga como contraste.
fn luma(red: u8, green: u8, blue: u8) -> u8 {
    let weighted = 299 * u32::from(red) + 587 * u32::from(green) + 114 * u32::from(blue);
    u8::try_from(weighted / 1000).unwrap_or(u8::MAX)
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

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        use super::super::placement::{Point, Size};
        Rect {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    /// Captura 3x2 em que cada pixel tem um tom de cinza diferente (10, 20, ... 60).
    fn gradient() -> Snapshot {
        let rgba = (1..=6u8)
            .flat_map(|n| [n * 10, n * 10, n * 10, 255])
            .collect();
        snapshot(3, 2, rgba)
    }

    #[test]
    fn the_crop_keeps_only_the_chosen_pixels_in_gray() {
        let crop = gradient().crop_gray(rect(1, 0, 2, 2));

        assert_eq!(crop.dimensions(), (2, 2));
        assert_eq!(crop.into_raw(), vec![20, 30, 50, 60]);
    }

    #[test]
    fn colors_become_their_perceived_brightness() {
        let red_green_blue = snapshot(3, 1, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);

        let gray = red_green_blue.crop_gray(rect(0, 0, 3, 1)).into_raw();

        assert!(gray[1] > gray[0] && gray[0] > gray[2], "{gray:?}");
    }

    #[test]
    fn a_crop_past_the_edges_is_cut_to_the_capture() {
        let crop = gradient().crop_gray(rect(2, 1, 5, 5));

        assert_eq!(crop.dimensions(), (1, 1));
        assert_eq!(crop.into_raw(), vec![60]);
    }

    #[test]
    fn a_crop_outside_the_capture_is_empty() {
        let crop = gradient().crop_gray(rect(10, 10, 5, 5));

        assert_eq!(crop.dimensions(), (0, 0));
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
