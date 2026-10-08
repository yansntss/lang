use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

type Abort = Box<dyn FnOnce() + Send>;

struct Running {
    id: u64,
    abort: Abort,
}

/// Guarda a única explicação em andamento. Começar outra cancela a anterior, e o front pode
/// cancelar pelo id: sem isso, fechar o popup deixaria a geração gastando cota.
#[derive(Default)]
pub struct ActiveExplain {
    next_id: AtomicU64,
    running: Mutex<Option<Running>>,
}

impl ActiveExplain {
    /// Reserva o id da próxima explicação, antes de a tarefa existir.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Registra a explicação `id` e cancela a que estava em andamento.
    pub fn register(&self, id: u64, abort: impl FnOnce() + Send + 'static) {
        let previous = self.lock().replace(Running {
            id,
            abort: Box::new(abort),
        });
        if let Some(previous) = previous {
            (previous.abort)();
        }
    }

    /// Cancela `id`, se ainda for a explicação atual.
    pub fn cancel(&self, id: u64) {
        let running = self.take_if(id);
        if let Some(running) = running {
            (running.abort)();
        }
    }

    /// Cancela a explicação em andamento, qualquer que seja o id.
    pub fn cancel_current(&self) {
        let running = self.lock().take();
        if let Some(running) = running {
            (running.abort)();
        }
    }

    /// A explicação `id` terminou sozinha: esquece-a sem acionar o cancelamento.
    pub fn finish(&self, id: u64) {
        drop(self.take_if(id));
    }

    fn take_if(&self, id: u64) -> Option<Running> {
        let mut guard = self.lock();
        if guard.as_ref().is_some_and(|running| running.id == id) {
            guard.take()
        } else {
            None
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    use super::*;

    fn flag() -> (Arc<AtomicBool>, impl FnOnce() + Send + 'static) {
        let flag = Arc::new(AtomicBool::new(false));
        let clone = Arc::clone(&flag);
        (flag, move || clone.store(true, Ordering::SeqCst))
    }

    fn aborted(flag: &AtomicBool) -> bool {
        flag.load(Ordering::SeqCst)
    }

    #[tokio::test]
    async fn cancel_really_aborts_a_running_task() {
        let active = ActiveExplain::default();
        let task = tokio::spawn(std::future::pending::<()>());
        let abort_handle = task.abort_handle();
        active.register(1, move || abort_handle.abort());

        active.cancel(1);

        assert!(task.await.unwrap_err().is_cancelled());
    }

    #[test]
    fn hands_out_increasing_ids() {
        let active = ActiveExplain::default();

        let first = active.next_id();
        let second = active.next_id();

        assert!(first < second);
    }

    #[test]
    fn starting_a_new_explanation_cancels_the_previous_one() {
        let active = ActiveExplain::default();
        let (first, abort_first) = flag();
        let (second, abort_second) = flag();

        active.register(1, abort_first);
        active.register(2, abort_second);

        assert!(aborted(&first));
        assert!(!aborted(&second));
    }

    #[test]
    fn cancel_aborts_the_matching_explanation() {
        let active = ActiveExplain::default();
        let (task, abort) = flag();
        active.register(7, abort);

        active.cancel(7);

        assert!(aborted(&task));
    }

    #[test]
    fn cancel_ignores_a_stale_id() {
        let active = ActiveExplain::default();
        let (task, abort) = flag();
        active.register(2, abort);

        active.cancel(1);
        assert!(!aborted(&task));

        active.cancel(2);
        assert!(aborted(&task));
    }

    #[test]
    fn cancel_current_aborts_whatever_is_running() {
        let active = ActiveExplain::default();
        let (task, abort) = flag();
        active.register(5, abort);

        active.cancel_current();

        assert!(aborted(&task));
    }

    #[test]
    fn cancel_current_without_a_running_explanation_does_nothing() {
        ActiveExplain::default().cancel_current();
    }

    #[test]
    fn finish_forgets_the_explanation_without_aborting_it() {
        let active = ActiveExplain::default();
        let (task, abort) = flag();
        active.register(1, abort);

        active.finish(1);
        active.cancel(1);

        assert!(!aborted(&task));
    }

    #[test]
    fn finish_of_a_stale_id_keeps_the_current_explanation() {
        let active = ActiveExplain::default();
        let (task, abort) = flag();
        active.register(2, abort);

        active.finish(1);
        active.cancel(2);

        assert!(aborted(&task));
    }
}
