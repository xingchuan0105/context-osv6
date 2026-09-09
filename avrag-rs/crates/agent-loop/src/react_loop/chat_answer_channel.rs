//! Explicit answer framing keeps tool drafts out of progressive chat delivery.
use super::host_markers::{CHAT_ANSWER_CLOSE as CLOSE, CHAT_ANSWER_OPEN as OPEN};

#[derive(Default)]
pub(super) struct AnswerChannel {
    raw: String,
    pub emitted: usize,
}

impl AnswerChannel {
    pub fn push(&mut self, delta: &str) -> Result<String, &'static str> {
        self.raw.push_str(delta);
        let Some(body) = self.body()? else {
            return Ok(String::new());
        };
        // Retain enough bytes to detect any forbidden marker split over chunks
        // before any portion of it reaches the user. UTF-8 boundaries stay intact.
        let reserve = forbidden()
            .map(str::len)
            .max()
            .unwrap_or(0)
            .max(CLOSE.len());
        let mut end = body.len().saturating_sub(reserve).max(self.emitted);
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        let delta = body[self.emitted..end].to_string();
        self.emitted = end;
        Ok(delta)
    }

    fn body(&self) -> Result<Option<&str>, &'static str> {
        let Some(body) = self.raw.trim_start().strip_prefix(OPEN) else {
            return Ok(None);
        };
        let body = if let Some((body, tail)) = body.split_once(CLOSE) {
            if !tail.trim().is_empty() {
                return Err("chat_answer_trailing_content");
            }
            body
        } else {
            body
        };
        // An incomplete closing delimiter is retained, not treated as payload.
        let body = if let Some(pos) = body.rfind('<') {
            if CLOSE.starts_with(&body[pos..]) {
                &body[..pos]
            } else {
                body
            }
        } else {
            body
        };
        if forbidden().any(|tag| body[self.emitted.min(body.len())..].contains(tag)) {
            return Err("chat_answer_protocol_leak");
        }
        Ok(Some(body))
    }

    pub fn finish(&self) -> Result<Option<String>, &'static str> {
        let Some(body) = self.body()? else {
            return Ok(None);
        };
        if !self.raw.trim_end().ends_with(CLOSE) {
            return Err("chat_answer_unclosed");
        }
        if body.trim().is_empty() {
            return Err("chat_answer_empty");
        }
        Ok(Some(body.to_string()))
    }
}

fn forbidden() -> impl Iterator<Item = &'static str> {
    super::host_markers::forbidden_in_final_tags().chain(["DSML", "<code"])
}

pub(super) fn decode(raw: &str) -> Result<Option<String>, &'static str> {
    let mut channel = AnswerChannel::default();
    channel.push(raw)?;
    channel.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_boundaries_and_unicode_stream_before_completion() {
        let text = "这是一段面向用户的回答。".repeat(30);
        let raw = format!("{OPEN}{text}{CLOSE}");
        let mut channel = AnswerChannel::default();
        let mut visible = String::new();
        for c in raw.chars() {
            visible.push_str(&channel.push(&c.to_string()).unwrap());
        }
        assert!(visible.len() > text.len() / 2);
        let full = channel.finish().unwrap().unwrap();
        visible.push_str(&full[channel.emitted..]);
        assert_eq!(visible, text);
    }

    #[test]
    fn tool_and_skill_drafts_never_open_answer_channel() {
        for raw in [
            "<code language=\"python\">print(1)</code>",
            "{\"skill_request\":[\"memory\"]}",
            "draft prose",
        ] {
            let mut channel = AnswerChannel::default();
            assert_eq!(channel.push(raw).unwrap(), "");
            assert_eq!(channel.finish().unwrap(), None);
        }
    }

    #[test]
    fn protocol_fragments_are_held_even_after_prose() {
        for tag in forbidden().filter(|tag| *tag != CLOSE) {
            let mut channel = AnswerChannel::default();
            let mut visible = channel
                .push(&format!("{OPEN}{}", "ordinary prose ".repeat(12)))
                .unwrap();
            let mut failed = false;
            for c in tag.chars() {
                match channel.push(&c.to_string()) {
                    Ok(delta) => visible.push_str(&delta),
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            assert!(failed, "{tag}");
            assert!(!visible.contains(tag), "{tag}");
        }
    }

    #[test]
    fn rejects_unclosed_empty_or_trailing_answer() {
        for raw in [
            format!("{OPEN}unfinished"),
            format!("{OPEN} {CLOSE}"),
            format!("{OPEN}done{CLOSE}extra"),
        ] {
            assert!(decode(&raw).is_err());
        }
    }
}
