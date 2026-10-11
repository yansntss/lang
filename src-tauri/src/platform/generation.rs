//! Contador de geração: cada tarefa em segundo plano (uma leitura de região, uma sessão de
//! legenda) recebe um número, e só a mais recente pode mostrar o que fez.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Generation(AtomicU64);

impl Generation {
    /// Começa uma tarefa nova; as anteriores deixam de ser a atual.
    pub fn begin(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.0.load(Ordering::SeqCst) == generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_task_is_current() {
        let tasks = Generation::default();

        let first = tasks.begin();
        assert!(tasks.is_current(first));

        let second = tasks.begin();
        assert!(!tasks.is_current(first));
        assert!(tasks.is_current(second));
    }
}
