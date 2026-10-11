//! Preparo da imagem antes do OCR. O recorte já chega em escala de cinza (ver
//! `Snapshot::crop_gray`); aqui só se amplia o texto pequeno.

use image::imageops::{resize, FilterType};

use super::GrayImage;

/// Recortes com o menor lado abaixo disso são ampliados 2x: uma linha de legenda em 1080p tem
/// letras pequenas, e o OCR do Windows erra menos com elas maiores.
pub const UPSCALE_BELOW: u32 = 300;
const UPSCALE_FACTOR: u32 = 2;

/// Amplia 2x um recorte pequeno, se o resultado couber no limite do motor.
pub fn upscale_small(image: &GrayImage, max_dimension: u32) -> GrayImage {
    let (width, height) = image.dimensions();
    let small = width.min(height) < UPSCALE_BELOW;
    let fits = width.max(height).saturating_mul(UPSCALE_FACTOR) <= max_dimension;
    if !small || !fits {
        return image.clone();
    }
    resize(
        image,
        width * UPSCALE_FACTOR,
        height * UPSCALE_FACTOR,
        FilterType::Triangle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subtitle_strip_is_enlarged_twice() {
        let enlarged = upscale_small(&GrayImage::new(900, 60), 10_000);

        assert_eq!(enlarged.dimensions(), (1800, 120));
    }

    #[test]
    fn a_large_region_is_left_as_it_is() {
        let kept = upscale_small(&GrayImage::new(800, UPSCALE_BELOW), 10_000);

        assert_eq!(kept.dimensions(), (800, UPSCALE_BELOW));
    }

    #[test]
    fn it_never_grows_past_the_engine_limit() {
        let kept = upscale_small(&GrayImage::new(3000, 50), 4000);

        assert_eq!(kept.dimensions(), (3000, 50));
    }

    #[test]
    fn the_enlarged_image_keeps_the_content() {
        let mut source = GrayImage::new(2, 1);
        source.put_pixel(0, 0, image::Luma([0]));
        source.put_pixel(1, 0, image::Luma([255]));

        let enlarged = upscale_small(&source, 10_000);

        assert!(enlarged.get_pixel(0, 0).0[0] < 128);
        assert!(enlarged.get_pixel(3, 1).0[0] > 128);
    }
}
