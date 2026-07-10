//! SSE event-order contract for chat streams (CAP-STREAM).
//!
//! - Offline: [`validate_chat_sse_event_order`] for full sequences (tests).
//! - Online: [`SseEventOrderTracker`] in the HTTP stream loop.

/// Validate ordered chat SSE event **names** (not payloads).
///
/// Returns `Ok(())` when:
/// - sequence is non-empty
/// - first event is `start`
/// - last event is `done` or `error` (terminal)
/// - terminal appears exactly once and is last
pub fn validate_chat_sse_event_order(event_names: &[&str]) -> Result<(), String> {
    if event_names.is_empty() {
        return Err("SSE sequence must not be empty".to_string());
    }
    if event_names[0] != "start" {
        return Err(format!(
            "first SSE event must be start, got {:?}",
            event_names[0]
        ));
    }
    let last = *event_names.last().expect("non-empty");
    if last != "done" && last != "error" {
        return Err(format!(
            "last SSE event must be done or error, got {last:?}"
        ));
    }
    let terminal_count = event_names
        .iter()
        .filter(|n| **n == "done" || **n == "error")
        .count();
    if terminal_count != 1 {
        return Err(format!(
            "terminal (done|error) must appear exactly once, got {terminal_count} in {event_names:?}"
        ));
    }
    Ok(())
}

/// Incremental tracker for live SSE streams (start → … → done|error).
#[derive(Debug, Default, Clone)]
pub struct SseEventOrderTracker {
    started: bool,
    terminal: bool,
    names: Vec<&'static str>,
}

impl SseEventOrderTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe one emitted event name. Returns `Err` if order is violated.
    /// After terminal, further events are rejected.
    pub fn observe(&mut self, event_name: &'static str) -> Result<(), String> {
        if self.terminal {
            return Err(format!(
                "SSE event after terminal: got {event_name:?}, prior={:?}",
                self.names
            ));
        }
        if !self.started {
            if event_name != "start" {
                return Err(format!(
                    "first SSE event must be start, got {event_name:?}"
                ));
            }
            self.started = true;
            self.names.push(event_name);
            return Ok(());
        }
        self.names.push(event_name);
        if event_name == "done" || event_name == "error" {
            self.terminal = true;
        }
        Ok(())
    }

    /// Call when the receiver ends. Requires start + terminal.
    pub fn finish(&self) -> Result<(), String> {
        if !self.started {
            return Err("SSE stream ended without start".to_string());
        }
        if !self.terminal {
            return Err(format!(
                "SSE stream ended without done/error, events={:?}",
                self.names
            ));
        }
        validate_chat_sse_event_order(&self.names)
    }

    pub fn event_names(&self) -> &[&'static str] {
        &self.names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S4 P-Stream (CAP-STREAM): happy path order.
    #[test]
    fn patho_stream_start_middle_done_ok() {
        assert!(validate_chat_sse_event_order(&["start", "token", "done"]).is_ok());
        assert!(validate_chat_sse_event_order(&["start", "done"]).is_ok());
        assert!(validate_chat_sse_event_order(&["start", "error"]).is_ok());
    }

    /// S4 P-Stream: missing start / missing done / trailing after done.
    #[test]
    fn patho_stream_rejects_bad_order() {
        assert!(validate_chat_sse_event_order(&[]).is_err());
        assert!(validate_chat_sse_event_order(&["token", "done"]).is_err());
        assert!(validate_chat_sse_event_order(&["start", "token"]).is_err());
        assert!(validate_chat_sse_event_order(&["start", "done", "token"]).is_err());
        assert!(validate_chat_sse_event_order(&["start", "done", "done"]).is_err());
    }

    #[test]
    fn patho_stream_tracker_live_sequence() {
        let mut t = SseEventOrderTracker::new();
        t.observe("start").unwrap();
        t.observe("token").unwrap();
        t.observe("done").unwrap();
        t.finish().unwrap();
        assert!(t.observe("token").is_err());
    }

    #[test]
    fn patho_stream_tracker_rejects_post_terminal() {
        let mut t = SseEventOrderTracker::new();
        t.observe("start").unwrap();
        t.observe("error").unwrap();
        assert!(t.observe("token").is_err());
        t.finish().unwrap();
    }
}
