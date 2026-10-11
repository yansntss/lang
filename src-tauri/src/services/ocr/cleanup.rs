//! Limpeza do texto do OCR: o motor devolve uma linha por linha da imagem, e a tradução quer
//! frases inteiras.

/// Junta as linhas de cada parágrafo com espaço, refaz palavras quebradas com hífen no fim da
/// linha e colapsa espaços. Linhas em branco separam parágrafos (um por linha no resultado).
pub fn clean(raw: &str) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in raw.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }
        current = join_line(current, &line);
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }
    paragraphs.join("\n")
}

/// Acrescenta `line` ao parágrafo. Com `letra-` no fim da linha anterior e uma letra no começo
/// da próxima, a palavra continua sem espaço e com o hífen (`well-known`, `Spider-Man`): textos
/// de jogos e apps quase nunca hifenizam palavras, mas quebram muito em compostos com hífen.
fn join_line(paragraph: String, line: &str) -> String {
    if paragraph.is_empty() {
        return line.to_owned();
    }
    let mut ending = paragraph.chars().rev();
    let split_word = matches!(
        (ending.next(), ending.next()),
        (Some('-'), Some(letter)) if letter.is_alphabetic()
    );
    let continues = line.chars().next().is_some_and(char::is_alphabetic);
    if split_word && continues {
        format!("{paragraph}{line}")
    } else {
        format!("{paragraph} {line}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_of_a_paragraph_become_one_sentence() {
        assert_eq!(
            clean("You should not\nhave come here.\nLeave now."),
            "You should not have come here. Leave now."
        );
    }

    #[test]
    fn a_compound_word_split_at_its_hyphen_is_joined_again() {
        assert_eq!(clean("a well-\nknown actor"), "a well-known actor");
        assert_eq!(clean("state-of-\nthe-art"), "state-of-the-art");
    }

    #[test]
    fn a_hyphen_before_a_capital_letter_is_kept_as_part_of_the_name() {
        assert_eq!(clean("talk to Spider-\nMan now"), "talk to Spider-Man now");
    }

    #[test]
    fn a_dash_that_is_not_after_a_letter_is_kept_with_a_space() {
        assert_eq!(clean("score 10 -\nlevel 2"), "score 10 - level 2");
    }

    #[test]
    fn blank_lines_separate_paragraphs() {
        assert_eq!(
            clean("First line\ncontinues\n\n\nSecond one"),
            "First line continues\nSecond one"
        );
    }

    #[test]
    fn extra_spaces_and_windows_line_endings_are_collapsed() {
        assert_eq!(
            clean("  Hello   there \r\n  friend  "),
            "Hello there friend"
        );
    }

    #[test]
    fn nothing_but_whitespace_gives_an_empty_text() {
        assert_eq!(clean(""), "");
        assert_eq!(clean(" \n\t\n "), "");
    }
}
