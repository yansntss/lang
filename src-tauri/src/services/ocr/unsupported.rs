//! Fora do Windows ainda não há motor de OCR (macOS: Vision; Linux: Tesseract, no futuro).

use super::{GrayImage, OcrEngine};
use crate::error::AppError;

pub struct UnsupportedOcr;

impl OcrEngine for UnsupportedOcr {
    fn max_dimension(&self) -> u32 {
        0
    }

    fn recognize(&self, _image: &GrayImage) -> Result<String, AppError> {
        Err(AppError::Ocr("OCR não suportado neste sistema".into()))
    }
}
