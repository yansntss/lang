//! Mover e redimensionar o popup a partir do conteúdo web. A janela não tem barra de título, e a
//! borda nativa de redimensionamento de uma janela sem decoração tem só alguns pixels invisíveis:
//! o front desenha as próprias alças e pede aqui o arrasto nativo do sistema.

use serde::Deserialize;
use tauri::WebviewWindow;
use tauri_runtime::ResizeDirection;

use super::window_error;
use crate::error::AppError;

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
    window.start_dragging().map_err(window_error)
}

/// Começa a redimensionar a janela pela borda `edge`, com o botão do mouse pressionado.
pub fn start_resizing(window: &WebviewWindow, edge: ResizeEdge) -> Result<(), AppError> {
    window
        .as_ref()
        .window()
        .start_resize_dragging(edge.into())
        .map_err(window_error)
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
