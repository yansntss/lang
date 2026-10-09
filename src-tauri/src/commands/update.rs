use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppError;
use crate::services::update::{AvailableUpdate, TauriUpdateSource, UpdateEvent, UpdateService};

type Updates = UpdateService<TauriUpdateSource>;

/// Procura uma versão nova. `None` significa que o app já está na versão mais recente.
#[tauri::command]
pub async fn check_update(
    updates: State<'_, Updates>,
) -> Result<Option<AvailableUpdate>, AppError> {
    updates
        .check()
        .await
        .inspect_err(|error| log::warn!("falha ao verificar atualizações: {error:?}"))
}

/// Baixa e instala a versão que a última verificação encontrou. O andamento chega pelo canal.
/// No Windows o app encerra sozinho quando o instalador assume.
#[tauri::command]
pub async fn install_update(
    updates: State<'_, Updates>,
    on_event: Channel<UpdateEvent>,
) -> Result<(), AppError> {
    updates
        .install(|event| {
            if let Err(error) = on_event.send(event) {
                log::warn!("não foi possível enviar o andamento da atualização: {error}");
            }
        })
        .await
        .inspect_err(|error| log::warn!("falha ao instalar a atualização: {error:?}"))
}
