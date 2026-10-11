//! Leitura de texto em imagens (OCR), para traduzir o que aparece na tela (legendas de jogos,
//! imagens). Cada sistema tem o seu motor; por enquanto só o Windows (`Windows.Media.Ocr`).

mod cleanup;
mod preprocess;
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;

pub use image::GrayImage;

#[cfg(windows)]
pub use self::windows::WindowsOcr as PlatformOcr;
#[cfg(not(windows))]
pub use unsupported::UnsupportedOcr as PlatformOcr;

use crate::error::AppError;

/// Um motor de OCR. Existe como trait para o serviço ser testado sem o sistema.
pub trait OcrEngine: Send + Sync {
    /// Maior lado de imagem que o motor aceita, em pixels.
    fn max_dimension(&self) -> u32;
    /// O texto reconhecido, uma linha por linha encontrada na imagem.
    fn recognize(&self, image: &GrayImage) -> Result<String, AppError>;
}

pub struct OcrService<E> {
    engine: E,
}

impl<E: OcrEngine> OcrService<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// Lê o texto de uma imagem já recortada: amplia se for pequena, reconhece e limpa as
    /// quebras de linha. Sem texto, é erro: o usuário precisa saber que nada foi lido.
    pub fn read(&self, image: &GrayImage) -> Result<String, AppError> {
        let prepared = preprocess::upscale_small(image, self.engine.max_dimension());
        let text = cleanup::clean(&self.engine.recognize(&prepared)?);
        if text.is_empty() {
            return Err(AppError::NoTextFound);
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Motor falso: devolve o texto (ou o erro) combinado e guarda o tamanho da imagem recebida.
    struct FakeEngine {
        answer: Result<String, fn() -> AppError>,
        seen: Mutex<Option<(u32, u32)>>,
    }

    impl FakeEngine {
        fn answering(text: &str) -> Self {
            Self {
                answer: Ok(text.to_owned()),
                seen: Mutex::new(None),
            }
        }

        fn failing(error: fn() -> AppError) -> Self {
            Self {
                answer: Err(error),
                seen: Mutex::new(None),
            }
        }
    }

    impl OcrEngine for FakeEngine {
        fn max_dimension(&self) -> u32 {
            10_000
        }

        fn recognize(&self, image: &GrayImage) -> Result<String, AppError> {
            *self.seen.lock().unwrap() = Some(image.dimensions());
            self.answer.clone().map_err(|error| error())
        }
    }

    fn image(width: u32, height: u32) -> GrayImage {
        GrayImage::new(width, height)
    }

    #[test]
    fn returns_the_recognized_text_cleaned_up() {
        let service = OcrService::new(FakeEngine::answering("Ask the  well-\nknown\n  smith"));

        let text = service.read(&image(600, 600)).unwrap();

        assert_eq!(text, "Ask the well-known smith");
    }

    #[test]
    fn a_small_region_reaches_the_engine_enlarged() {
        let engine = FakeEngine::answering("hi");
        let service = OcrService::new(engine);

        service.read(&image(500, 40)).unwrap();

        assert_eq!(*service.engine.seen.lock().unwrap(), Some((1000, 80)));
    }

    #[test]
    fn no_text_in_the_region_is_an_error_the_user_sees() {
        let service = OcrService::new(FakeEngine::answering(" \n \n"));

        let error = service.read(&image(600, 600)).unwrap_err();

        assert_eq!(error.code(), "no_text_found");
    }

    #[test]
    fn engine_errors_pass_through() {
        let service = OcrService::new(FakeEngine::failing(|| AppError::OcrLanguageMissing));

        let error = service.read(&image(600, 600)).unwrap_err();

        assert_eq!(error.code(), "ocr_language_missing");
    }
}
