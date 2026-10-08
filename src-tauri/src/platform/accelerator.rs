//! Atalhos como texto (`"Ctrl+Alt+T"`): leitura, validação e conversão para o que o sistema
//! precisa. O texto vem do front, então tudo é revalidado aqui.

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

use crate::error::AppError;

/// Um atalho já validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accelerator {
    /// Forma canônica, na ordem Ctrl+Alt+Shift+Win+Tecla (é o que fica salvo e é mostrado).
    pub label: String,
    pub shortcut: Shortcut,
    /// Código virtual do Windows da tecla principal.
    pub virtual_key: u16,
}

const VK_F1: u16 = 0x70;
const F_KEYS: std::ops::RangeInclusive<u16> = 1..=12;

fn invalid(message: &str) -> AppError {
    AppError::InvalidInput(message.to_owned())
}

/// Aceita modificadores (Ctrl, Alt, Shift, Win) e uma tecla A–Z, 0–9 ou F1–F12 por último.
/// Exige ao menos Ctrl, Alt ou Win: um atalho global sem eles, ou só com Shift, roubaria a
/// digitação normal de todos os apps.
pub fn parse(text: &str) -> Result<Accelerator, AppError> {
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let Some((key_text, modifier_texts)) = parts.split_last() else {
        return Err(invalid("Informe um atalho, por exemplo Ctrl+Alt+T."));
    };
    if key_text.is_empty() {
        return Err(invalid("Informe um atalho, por exemplo Ctrl+Alt+T."));
    }

    let mut modifiers = Modifiers::empty();
    for modifier in modifier_texts {
        modifiers |= match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Modifiers::CONTROL,
            "alt" | "option" => Modifiers::ALT,
            "shift" => Modifiers::SHIFT,
            "win" | "super" | "meta" | "cmd" => Modifiers::SUPER,
            _ => return Err(invalid("Atalho inválido. Use, por exemplo, Ctrl+Alt+T.")),
        };
    }
    if !modifiers.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER) {
        return Err(invalid("Use Ctrl, Alt ou Win junto da tecla do atalho."));
    }

    let (key_label, code_name, virtual_key) = parse_key(key_text)?;
    reject_common_shortcuts(modifiers, &key_label)?;
    let code: Code = code_name
        .parse()
        .map_err(|_| invalid("Essa tecla não é aceita no atalho."))?;

    Ok(Accelerator {
        label: label(modifiers, &key_label),
        shortcut: Shortcut::new(Some(modifiers), code),
        virtual_key,
    })
}

/// Um atalho global tira a tecla de todos os apps. `Ctrl+C`, `Ctrl+V` ou `Alt+A` sozinhos
/// engoliriam copiar, colar e os menus em qualquer lugar (inclusive a captura da seleção, que
/// simula Ctrl+C), e o rollback não ajuda: o sistema aceita o registro.
///
/// Para letras e números exige duas teclas entre Ctrl, Alt e Shift, ou a tecla Win; teclas de
/// função aceitam um modificador, exceto `Alt+F4` (fechar janela).
fn reject_common_shortcuts(modifiers: Modifiers, key: &str) -> Result<(), AppError> {
    let is_function_key = key.starts_with('F') && key.len() > 1;
    if is_function_key {
        if key == "F4" && modifiers.contains(Modifiers::ALT) {
            return Err(invalid("Alt+F4 fecha janelas. Escolha outro atalho."));
        }
        return Ok(());
    }

    let common_modifiers = [Modifiers::CONTROL, Modifiers::ALT, Modifiers::SHIFT]
        .into_iter()
        .filter(|flag| modifiers.contains(*flag))
        .count();
    if common_modifiers < 2 && !modifiers.contains(Modifiers::SUPER) {
        return Err(invalid(
            "Esse atalho já é usado pelos apps. Combine dois entre Ctrl, Alt e Shift, ou use Win.",
        ));
    }
    Ok(())
}

/// Devolve (texto canônico da tecla, nome do `Code`, código virtual).
fn parse_key(text: &str) -> Result<(String, String, u16), AppError> {
    let unsupported = || invalid("Use uma letra, um número ou F1 a F12 como tecla do atalho.");
    let upper = text.to_ascii_uppercase();

    if let Some(number) = upper
        .strip_prefix('F')
        .and_then(|rest| rest.parse::<u16>().ok())
    {
        if !F_KEYS.contains(&number) {
            return Err(unsupported());
        }
        return Ok((
            format!("F{number}"),
            format!("F{number}"),
            VK_F1 + number - 1,
        ));
    }

    let mut chars = upper.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), None) if letter.is_ascii_uppercase() => Ok((
            letter.to_string(),
            format!("Key{letter}"),
            u16::from(letter as u8),
        )),
        (Some(digit), None) if digit.is_ascii_digit() => Ok((
            digit.to_string(),
            format!("Digit{digit}"),
            u16::from(digit as u8),
        )),
        _ => Err(unsupported()),
    }
}

fn label(modifiers: Modifiers, key: &str) -> String {
    let mut parts = Vec::new();
    for (flag, name) in [
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Win"),
    ] {
        if modifiers.contains(flag) {
            parts.push(name);
        }
    }
    parts.push(key);
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_factory_shortcut() {
        let parsed = parse("Ctrl+Alt+T").unwrap();

        assert_eq!(parsed.label, "Ctrl+Alt+T");
        assert_eq!(
            parsed.shortcut,
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyT)
        );
        assert_eq!(parsed.virtual_key, 0x54);
    }

    #[test]
    fn ignores_case_spaces_and_modifier_order_in_the_canonical_label() {
        let parsed = parse(" alt + control + shift + j ").unwrap();

        assert_eq!(parsed.label, "Ctrl+Alt+Shift+J");
        assert_eq!(parsed.virtual_key, u16::from(b'J'));
    }

    #[test]
    fn accepts_digits_function_keys_and_the_windows_key() {
        let digit = parse("Win+5").unwrap();
        let function = parse("Ctrl+F12").unwrap();
        let first_function = parse("Alt+f1").unwrap();

        assert_eq!(digit.label, "Win+5");
        assert_eq!(digit.virtual_key, 0x35);
        assert_eq!(function.label, "Ctrl+F12");
        assert_eq!(function.virtual_key, 0x7B);
        assert_eq!(first_function.virtual_key, 0x70);
    }

    #[test]
    fn rejects_shortcuts_that_would_steal_everyday_keys() {
        for text in [
            "Ctrl+C",
            "Ctrl+V",
            "Ctrl+X",
            "Ctrl+Z",
            "Ctrl+A",
            "Ctrl+S",
            "Alt+A",
            "Alt+F4",
            "Ctrl+Alt+F4",
            "Win+Alt+F4",
            "Alt+1",
            "Ctrl+5",
        ] {
            let error = parse(text).unwrap_err();

            assert_eq!(error.code(), "invalid_input", "{text}");
        }
    }

    #[test]
    fn accepts_safe_combinations() {
        for text in [
            "Ctrl+Alt+T",
            "Ctrl+Shift+J",
            "Alt+Shift+K",
            "Ctrl+Alt+Shift+Q",
            "Win+5",
            "Win+Shift+Z",
            "Ctrl+F5",
            "Alt+F2",
            "Win+F9",
        ] {
            assert!(parse(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn requires_ctrl_alt_or_win() {
        for text in ["T", "Shift+T", "F5", "Shift+F5"] {
            let error = parse(text).unwrap_err();

            assert_eq!(error.code(), "invalid_input", "{text}");
        }
    }

    #[test]
    fn rejects_malformed_and_unsupported_shortcuts() {
        for text in [
            "",
            "   ",
            "Ctrl+",
            "+T",
            "Ctrl++T",
            "Ctrl+Alt",
            "Ctrl+Alt+Enter",
            "Ctrl+Alt+Space",
            "Ctrl+Alt+F0",
            "Ctrl+Alt+F13",
            "Ctrl+Alt+TT",
            "Ctrl+Alt+é",
            "Ctrl+Foo+T",
            "Ctrl+Alt+<",
        ] {
            let error = parse(text).unwrap_err();

            assert_eq!(error.code(), "invalid_input", "{text:?}");
        }
    }

    #[test]
    fn error_messages_are_safe_to_show() {
        let message = parse("Ctrl+Foo+T").unwrap_err().to_string();

        assert!(!message.contains("Foo"));
    }
}
