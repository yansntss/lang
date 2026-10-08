//! Configuração que a captura da seleção lê a cada uso. É estado global (como o resto de
//! `shortcut.rs`) porque a captura roda numa thread própria, disparada pelo atalho.

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

/// Código virtual da tecla `T`, a do atalho de fábrica.
const DEFAULT_MAIN_KEY: u16 = 0x54;

static MAIN_KEY: AtomicU16 = AtomicU16::new(DEFAULT_MAIN_KEY);
static ALLOW_EDITORS: AtomicBool = AtomicBool::new(false);

/// Tecla principal do atalho, para a captura esperar que ela seja solta (segurá-la depois de
/// soltar os modificadores faria o auto-repeat digitar no app de origem).
pub fn set_main_key(virtual_key: u16) {
    MAIN_KEY.store(virtual_key, Ordering::Relaxed);
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn main_key() -> u16 {
    MAIN_KEY.load(Ordering::Relaxed)
}

pub fn set_allow_editors(allow: bool) {
    ALLOW_EDITORS.store(allow, Ordering::Relaxed);
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn allow_editors() -> bool {
    ALLOW_EDITORS.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_values_set_are_the_values_read_and_the_defaults_are_safe() {
        assert_eq!(main_key(), DEFAULT_MAIN_KEY);
        assert!(!allow_editors());

        set_main_key(0x4A);
        set_allow_editors(true);
        assert_eq!(main_key(), 0x4A);
        assert!(allow_editors());

        set_main_key(DEFAULT_MAIN_KEY);
        set_allow_editors(false);
    }
}
