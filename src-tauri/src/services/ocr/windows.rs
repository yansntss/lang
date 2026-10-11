//! OCR nativo do Windows (`Windows.Media.Ocr`): offline, sem chave, com o pacote de idioma
//! que o Windows já instala junto do inglês.

use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine as WinOcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

use super::{GrayImage, OcrEngine};
use crate::error::AppError;

/// O texto que o app lê vem quase sempre em inglês (jogos, apps): é o idioma preferido do OCR.
const OCR_LANGUAGE: &str = "en-US";
/// Se o Windows não informar o limite, um valor que todas as versões aceitam.
const FALLBACK_MAX_DIMENSION: u32 = 4096;

pub struct WindowsOcr;

fn ocr_error(error: windows::core::Error) -> AppError {
    AppError::Ocr(error.to_string())
}

/// O WinRT precisa ser inicializado na thread que o chama. Rodar de novo na mesma thread é
/// inofensivo; numa thread que já é STA (a da interface) ele continua utilizável.
fn ensure_winrt() {
    // SAFETY: `RoInitialize` só configura o COM/WinRT da thread atual e não recebe ponteiros.
    // Nunca é desfeito com `RoUninitialize`: a thread de OCR termina logo depois.
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        if error.code() != RPC_E_CHANGED_MODE {
            log::warn!("falha ao inicializar o WinRT: {error}");
        }
    }
}

/// O motor em inglês; sem ele, o de um idioma do usuário. O pacote de OCR de inglês nem
/// sempre vem instalado (mesmo com o Windows em inglês), e o de português, por exemplo, também
/// lê o alfabeto latino, com um pouco menos de precisão.
fn engine() -> Result<WinOcrEngine, AppError> {
    let language = Language::CreateLanguage(&HSTRING::from(OCR_LANGUAGE)).map_err(ocr_error)?;
    if WinOcrEngine::IsLanguageSupported(&language).map_err(ocr_error)? {
        return WinOcrEngine::TryCreateFromLanguage(&language).map_err(ocr_error);
    }
    log::info!("OCR em {OCR_LANGUAGE} não instalado; usando um idioma do perfil do usuário");
    // Sem nenhum idioma de OCR instalado, o Windows devolve nulo, que vira erro aqui.
    WinOcrEngine::TryCreateFromUserProfileLanguages().map_err(|error| {
        log::warn!("nenhum idioma de OCR disponível: {error}");
        AppError::OcrLanguageMissing
    })
}

/// A imagem em cinza vira BGRA, o formato que o OCR do Windows sempre aceita.
fn bitmap(image: &GrayImage) -> Result<SoftwareBitmap, AppError> {
    let dimension =
        |value: u32| i32::try_from(value).map_err(|_| AppError::Ocr("imagem grande demais".into()));
    let (width, height) = (dimension(image.width())?, dimension(image.height())?);
    let bgra: Vec<u8> = image
        .pixels()
        .flat_map(|pixel| {
            let [luma] = pixel.0;
            [luma, luma, luma, u8::MAX]
        })
        .collect();

    let writer = DataWriter::new().map_err(ocr_error)?;
    writer.WriteBytes(&bgra).map_err(ocr_error)?;
    let buffer = writer.DetachBuffer().map_err(ocr_error)?;
    SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, width, height)
        .map_err(ocr_error)
}

impl OcrEngine for WindowsOcr {
    fn max_dimension(&self) -> u32 {
        ensure_winrt();
        WinOcrEngine::MaxImageDimension().unwrap_or(FALLBACK_MAX_DIMENSION)
    }

    fn recognize(&self, image: &GrayImage) -> Result<String, AppError> {
        ensure_winrt();
        let engine = engine()?;
        let bitmap = bitmap(image)?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(ocr_error)?
            .join()
            .map_err(ocr_error)?;

        let lines = result.Lines().map_err(ocr_error)?;
        let count = lines.Size().map_err(ocr_error)?;
        let texts = (0..count)
            .map(|index| {
                let line = lines.GetAt(index).map_err(ocr_error)?;
                Ok(line.Text().map_err(ocr_error)?.to_string_lossy())
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        Ok(texts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use image::Luma;

    use super::*;

    /// Usa o OCR real do Windows: `cargo test -- --ignored`.
    #[test]
    #[ignore = "usa o OCR do Windows"]
    fn a_blank_image_has_no_text() {
        let blank = GrayImage::from_pixel(400, 100, Luma([255]));

        let text = WindowsOcr.recognize(&blank).unwrap();

        assert!(text.trim().is_empty());
    }

    #[test]
    #[ignore = "usa o OCR do Windows"]
    fn reads_light_subtitle_text_on_a_dark_background() {
        let sample =
            image::load_from_memory(include_bytes!("../../../tests/fixtures/ocr-sample.png"))
                .unwrap()
                .to_luma8();
        let service = super::super::OcrService::new(WindowsOcr);

        let text = service.read(&sample).unwrap();

        assert_eq!(text.to_lowercase(), "press any key to continue");
    }

    #[test]
    #[ignore = "usa o OCR do Windows"]
    fn an_ocr_engine_is_available_with_or_without_english() {
        ensure_winrt();

        if let Err(error) = engine() {
            panic!("{error:?}");
        }
        assert!(WindowsOcr.max_dimension() >= 1000);
    }
}
