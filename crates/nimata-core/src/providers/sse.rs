//! Incremental parser for Server-Sent Events (the `text/event-stream`
//! format providers use to stream replies).
//!
//! Bytes arrive in arbitrary chunks, so a line, or a multi-byte character,
//! can be split across chunks; the parser buffers until a full event is seen.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` field, if the server sent one.
    pub event: Option<String>,
    /// The `data:` lines joined with newlines.
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    event: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds bytes and returns every event they complete.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|&b| b == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line).into_owned();
            if let Some(event) = self.line(&line) {
                events.push(event);
            }
        }
        events
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            if self.data.is_empty() {
                self.event = None;
                return None;
            }
            return Some(SseEvent {
                event: self.event.take(),
                data: std::mem::take(&mut self.data).join("\n"),
            });
        }
        if line.starts_with(':') {
            return None; // comment, used by servers as a keep-alive
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => self.data.push(value.to_string()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_chunks() {
        let mut parser = SseParser::new();
        assert!(parser.push(b"event: response.out").is_empty());
        assert!(parser.push(b"put_text.delta\ndata: {\"delta\":").is_empty());
        let events = parser.push(b"\"Hi\"}\n\n");
        assert_eq!(
            events,
            vec![SseEvent {
                event: Some("response.output_text.delta".into()),
                data: "{\"delta\":\"Hi\"}".into()
            }]
        );
    }

    #[test]
    fn handles_crlf_comments_and_multi_line_data() {
        let mut parser = SseParser::new();
        let events =
            parser.push(b": keep-alive\r\n\r\ndata: one\r\ndata: two\r\n\r\ndata:three\n\n");
        let data: Vec<_> = events.iter().map(|e| e.data.as_str()).collect();
        assert_eq!(data, vec!["one\ntwo", "three"]);
    }

    #[test]
    fn keeps_multi_byte_characters_split_across_chunks() {
        let mut parser = SseParser::new();
        let text = "data: νήματα\n\n".as_bytes();
        let (a, b) = text.split_at(9); // inside the second Greek letter
        assert!(parser.push(a).is_empty());
        assert_eq!(parser.push(b)[0].data, "νήματα");
    }
}
