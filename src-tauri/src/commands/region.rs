use tauri::ipc::Response;
use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::platform::region_geometry::LogicalSelection;
use crate::platform::region_window::{self, RegionState};

/// A captura da tela em andamento, em bytes crus (ver `Snapshot::encode`). Assíncrono para
/// copiar os megabytes da imagem fora da thread da interface.
#[tauri::command]
pub async fn get_region_snapshot(state: State<'_, RegionState>) -> Result<Response, AppError> {
    state
        .encoded_snapshot()
        .map(Response::new)
        .ok_or_else(|| AppError::InvalidInput("Nenhuma captura em andamento.".into()))
}

/// O usuário soltou o mouse sobre a região. Uma seleção recusada (pequena demais) devolve o
/// erro e deixa a janela de seleção aberta.
#[tauri::command]
pub fn confirm_region(app: AppHandle, selection: LogicalSelection) -> Result<(), AppError> {
    region_window::confirm(&app, selection)
}

#[tauri::command]
pub fn cancel_region(app: AppHandle) -> Result<(), AppError> {
    region_window::close(&app)
}
