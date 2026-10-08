use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppError;
use crate::services::explain::{build_prompt, ExplainEvent};
use crate::state::AppState;

/// Começa a explicar e devolve o id da explicação. O texto chega ao front pelo canal, em
/// eventos `delta`, terminando com `done` ou `error`. Só é chamado por clique do usuário: o
/// texto só vai ao provedor de IA quando ele pede.
#[tauri::command]
pub fn explain(
    state: State<'_, AppState>,
    text: String,
    translation: String,
    on_event: Channel<ExplainEvent>,
) -> Result<u64, AppError> {
    let mut prompt = build_prompt(&text, &translation)?;
    prompt.model = state.settings.get().preferences.explain_model;
    let service = state.explanations.clone();
    let tasks = state.active_explain.clone();
    let id = tasks.next_id();

    let handle = {
        let tasks = tasks.clone();
        tauri::async_runtime::spawn(async move {
            service
                .run(&prompt, |event| {
                    // Canal fechado (janela recarregada): ninguém mais lê, então a tarefa se
                    // aborta no próximo `.await` em vez de gastar cota até o fim.
                    if let Err(error) = on_event.send(event) {
                        log::warn!("canal da explicação fechado, cancelando: {error}");
                        tasks.cancel(id);
                    }
                })
                .await;
            tasks.finish(id);
        })
    };
    tasks.register(id, move || handle.abort());

    Ok(id)
}

#[tauri::command]
pub fn cancel_explain(state: State<'_, AppState>, id: u64) {
    state.active_explain.cancel(id);
}
