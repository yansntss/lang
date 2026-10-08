use crate::error::AppError;

/// Teto do buffer de um evento incompleto. Eventos reais têm poucas centenas de bytes.
const MAX_PENDING_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Parser incremental de Server-Sent Events. Recebe bytes em pedaços de qualquer tamanho
/// (inclusive cortando um evento ou um caractere UTF-8 ao meio) e devolve os eventos completos.
#[derive(Default)]
pub struct SseParser {
    pending: Vec<u8>,
}

impl SseParser {
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, AppError> {
        // CRLF e LF terminam linhas do mesmo jeito; descartar o CR simplifica a busca.
        self.pending
            .extend(chunk.iter().copied().filter(|byte| *byte != b'\r'));

        let mut events = Vec::new();
        while let Some(end) = find_blank_line(&self.pending) {
            let block: Vec<u8> = self.pending.drain(..end + 2).collect();
            // O corte é em `\n\n` (ASCII), então o bloco nunca termina no meio de um caractere.
            if let Some(event) = parse_block(&String::from_utf8_lossy(&block)) {
                events.push(event);
            }
        }

        if self.pending.len() > MAX_PENDING_BYTES {
            return Err(AppError::Upstream(200));
        }
        Ok(events)
    }
}

fn find_blank_line(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|pair| pair == b"\n\n")
}

fn parse_block(block: &str) -> Option<SseEvent> {
    let mut event = None;
    let mut data: Vec<&str> = Vec::new();

    for line in block.lines() {
        // Linhas começadas por `:` são comentários (usados como keep-alive).
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => event = Some(value.to_owned()),
            "data" => data.push(value),
            _ => {}
        }
    }

    if event.is_none() && data.is_empty() {
        return None;
    }
    Some(SseEvent {
        event,
        data: data.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, data: &str) -> SseEvent {
        SseEvent {
            event: Some(name.into()),
            data: data.into(),
        }
    }

    #[test]
    fn parses_a_complete_event() {
        let mut parser = SseParser::default();

        let events = parser.feed(b"event: ping\ndata: {\"a\":1}\n\n").unwrap();

        assert_eq!(events, vec![event("ping", "{\"a\":1}")]);
    }

    #[test]
    fn parses_several_events_in_one_chunk() {
        let mut parser = SseParser::default();

        let events = parser
            .feed(b"event: a\ndata: 1\n\nevent: b\ndata: 2\n\n")
            .unwrap();

        assert_eq!(events, vec![event("a", "1"), event("b", "2")]);
    }

    #[test]
    fn waits_for_the_rest_of_an_event_split_across_chunks() {
        let mut parser = SseParser::default();

        assert!(parser.feed(b"event: a\nda").unwrap().is_empty());
        assert!(parser.feed(b"ta: 12").unwrap().is_empty());
        let events = parser.feed(b"3\n\n").unwrap();

        assert_eq!(events, vec![event("a", "123")]);
    }

    #[test]
    fn keeps_multibyte_characters_split_across_chunks() {
        let mut parser = SseParser::default();
        let bytes = "event: a\ndata: ação\n\n".as_bytes();
        // Corta no meio do "ç" (2 bytes em UTF-8).
        let cut = "event: a\ndata: a".len() + 1;

        assert!(parser.feed(&bytes[..cut]).unwrap().is_empty());
        let events = parser.feed(&bytes[cut..]).unwrap();

        assert_eq!(events, vec![event("a", "ação")]);
    }

    #[test]
    fn accepts_crlf_line_endings() {
        let mut parser = SseParser::default();

        let events = parser.feed(b"event: a\r\ndata: 1\r\n\r\n").unwrap();

        assert_eq!(events, vec![event("a", "1")]);
    }

    #[test]
    fn ignores_comments_and_unknown_fields() {
        let mut parser = SseParser::default();

        let events = parser
            .feed(b": keep-alive\n\nid: 7\nretry: 10\nevent: a\ndata: 1\n\n")
            .unwrap();

        assert_eq!(events, vec![event("a", "1")]);
    }

    #[test]
    fn joins_multiple_data_lines_with_a_newline() {
        let mut parser = SseParser::default();

        let events = parser.feed(b"event: a\ndata: um\ndata: dois\n\n").unwrap();

        assert_eq!(events, vec![event("a", "um\ndois")]);
    }

    #[test]
    fn rejects_an_event_that_never_ends() {
        let mut parser = SseParser::default();
        let junk = vec![b'x'; MAX_PENDING_BYTES + 1];

        let error = parser.feed(&junk).unwrap_err();

        assert_eq!(error.code(), "upstream");
    }
}
