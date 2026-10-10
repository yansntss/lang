//! Mover e redimensionar o popup a partir do conteúdo web. A janela não tem barra de título, e a
//! borda nativa de redimensionamento de uma janela sem decoração tem só alguns pixels invisíveis:
//! o front desenha as próprias alças e pede aqui o arrasto nativo do sistema.

use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::{Manager, WebviewWindow};
use tauri_runtime::ResizeDirection;

use super::popup_window::set_interacting;
use super::window_error;
use crate::error::AppError;

/// Intervalo entre as verificações do botão do mouse durante o arrasto.
const BUTTON_POLL: Duration = Duration::from_millis(30);
/// Tempo mínimo da marca de arrasto: o blur do arrasto nativo pode chegar depois de um clique
/// rápido na alça, quando o botão já foi solto.
const MIN_INTERACTION: Duration = Duration::from_millis(250);
/// Teto de segurança: se o estado do botão não puder ser lido, a marca não fica presa.
const MAX_INTERACTION: Duration = Duration::from_secs(60);

/// A borda ou o canto que o usuário está arrastando. Lista fechada: o front não escolhe outra
/// coisa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl From<ResizeEdge> for ResizeDirection {
    fn from(edge: ResizeEdge) -> Self {
        match edge {
            ResizeEdge::North => Self::North,
            ResizeEdge::South => Self::South,
            ResizeEdge::East => Self::East,
            ResizeEdge::West => Self::West,
            ResizeEdge::NorthEast => Self::NorthEast,
            ResizeEdge::NorthWest => Self::NorthWest,
            ResizeEdge::SouthEast => Self::SouthEast,
            ResizeEdge::SouthWest => Self::SouthWest,
        }
    }
}

/// Começa a mover a janela com o botão do mouse que está pressionado.
pub fn start_moving(window: &WebviewWindow) -> Result<(), AppError> {
    begin_interaction(window);
    window.start_dragging().map_err(window_error)
}

/// Começa a redimensionar a janela pela borda `edge`, com o botão do mouse pressionado.
pub fn start_resizing(window: &WebviewWindow, edge: ResizeEdge) -> Result<(), AppError> {
    begin_interaction(window);
    window
        .as_ref()
        .window()
        .start_resize_dragging(edge.into())
        .map_err(window_error)
}

/// O arrasto nativo tira o foco do conteúdo web, e o popup se ocultaria como num clique fora.
/// A marca de arrasto segura isso até o botão do mouse ser solto; aí o foco volta ao popup,
/// para Enter e Esc continuarem funcionando e um clique fora voltar a fechá-lo.
fn begin_interaction(window: &WebviewWindow) {
    set_interacting(window.app_handle(), true);
    let window = window.clone();
    thread::spawn(move || {
        let started = Instant::now();
        thread::sleep(MIN_INTERACTION);
        while is_mouse_button_down() && started.elapsed() < MAX_INTERACTION {
            thread::sleep(BUTTON_POLL);
        }
        set_interacting(window.app_handle(), false);
        if window.is_visible().unwrap_or(false) {
            if let Err(error) = window.set_focus() {
                log::warn!("não foi possível devolver o foco ao popup após o arrasto: {error}");
            }
        }
    });
}

/// Algum botão principal do mouse está pressionado agora. Olha os dois botões para cobrir
/// mouses com os botões trocados (canhotos).
#[cfg(windows)]
fn is_mouse_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};

    [VK_LBUTTON, VK_RBUTTON].into_iter().any(|key| {
        // SAFETY: `GetAsyncKeyState` só lê o estado global de uma tecla; não recebe ponteiros nem
        // guarda referências. O bit mais alto (valor negativo) indica a tecla pressionada agora.
        unsafe { GetAsyncKeyState(i32::from(key.0)) < 0 }
    })
}

/// Fora do Windows (sem suporte) a marca dura só o tempo mínimo.
#[cfg(not(windows))]
fn is_mouse_button_down() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_edge_names_the_front_sends() {
        let edge: ResizeEdge = serde_json::from_str(r#""southEast""#).unwrap();

        assert_eq!(edge, ResizeEdge::SouthEast);
    }

    #[test]
    fn rejects_an_edge_outside_the_list() {
        assert!(serde_json::from_str::<ResizeEdge>(r#""up""#).is_err());
    }

    #[test]
    fn each_edge_maps_to_the_same_direction() {
        assert!(matches!(
            ResizeDirection::from(ResizeEdge::NorthWest),
            ResizeDirection::NorthWest
        ));
        assert!(matches!(
            ResizeDirection::from(ResizeEdge::East),
            ResizeDirection::East
        ));
    }
}
