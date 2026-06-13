use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Tracks in-flight chat streams for cancellation (C2).
#[derive(Default)]
pub struct ChatStreamRegistry {
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ChatStreamRegistry {
    pub fn register(&self, request_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .expect("chat registry lock")
            .insert(request_id.to_string(), Arc::clone(&flag));
        flag
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        let mut guard = self.cancellations.lock().expect("chat registry lock");
        if let Some(flag) = guard.get(request_id) {
            flag.store(true, Ordering::SeqCst);
            guard.remove(request_id);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, request_id: &str) {
        self.cancellations
            .lock()
            .expect("chat registry lock")
            .remove(request_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_marks_registered_stream_and_is_idempotent() {
        let registry = ChatStreamRegistry::default();
        let flag = registry.register("req-1");

        assert!(registry.cancel("req-1"));
        assert!(flag.load(Ordering::SeqCst));
        assert!(!registry.cancel("req-1"));
    }

    #[test]
    fn remove_drops_registration_without_setting_cancel_flag() {
        let registry = ChatStreamRegistry::default();
        let flag = registry.register("req-2");

        registry.remove("req-2");

        assert!(!flag.load(Ordering::SeqCst));
        assert!(!registry.cancel("req-2"));
    }
}
