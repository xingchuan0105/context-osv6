use crate::schema::LlmError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Framing {
    #[default]
    Sse,
}

#[derive(Debug, Default)]
pub struct SseFramer {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
}

impl SseFramer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed_chunk(&mut self, chunk: &[u8]) -> Result<Vec<String>, LlmError> {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();

        while let Some(line) = self.take_line()? {
            if line.is_empty() {
                if let Some(frame) = self.flush_data_lines() {
                    frames.push(frame);
                }
                continue;
            }

            if line.starts_with(':') {
                continue;
            }

            if let Some(value) = line.strip_prefix("data:") {
                self.data_lines.push(value.trim_start().to_string());
            }
        }

        Ok(frames)
    }

    pub fn finish(&mut self) -> Result<Vec<String>, LlmError> {
        let mut frames = Vec::new();

        if !self.buffer.is_empty() {
            let line = String::from_utf8(std::mem::take(&mut self.buffer))
                .map_err(|e| LlmError::parse(format!("invalid UTF-8 in SSE trailing buffer: {e}")))?;
            let normalized = line.trim_end_matches('\r');
            if let Some(value) = normalized.strip_prefix("data:") {
                self.data_lines.push(value.trim_start().to_string());
            }
        }

        if let Some(frame) = self.flush_data_lines() {
            frames.push(frame);
        }

        Ok(frames)
    }

    fn flush_data_lines(&mut self) -> Option<String> {
        if self.data_lines.is_empty() {
            return None;
        }
        let payload = self.data_lines.join("\n");
        self.data_lines.clear();
        Some(payload)
    }

    fn take_line(&mut self) -> Result<Option<String>, LlmError> {
        let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') else {
            return Ok(None);
        };

        let mut raw_line = self.buffer.drain(..=index).collect::<Vec<_>>();
        raw_line.pop();
        if raw_line.last() == Some(&b'\r') {
            raw_line.pop();
        }

        let line = String::from_utf8(raw_line)
            .map_err(|e| LlmError::parse(format!("invalid UTF-8 in SSE line: {e}")))?;
        Ok(Some(line))
    }
}

#[cfg(test)]
mod tests {
    use super::SseFramer;

    #[test]
    fn sse_framer_emits_complete_data_payloads() {
        let mut framer = SseFramer::new();
        let frames = framer
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"Hel"}}]}

"#,
            )
            .unwrap();
        assert_eq!(frames.len(), 1);

        let mut frames = framer
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"lo"}}]}

data: [DONE]

"#,
            )
            .unwrap();
        frames.extend(framer.finish().unwrap());
        assert!(frames.iter().any(|f| f.contains(r#""content":"lo""#)));
        assert!(frames.iter().any(|f| f.trim() == "[DONE]"));
    }
}
