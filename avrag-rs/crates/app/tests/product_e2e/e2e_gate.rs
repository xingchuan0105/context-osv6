//! Runtime suite gating via `E2E_MODE`.

/// Product E2E suite tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum E2eSuite {
    /// PR smoke (`smoke::`, top-level routing tests).
    Smoke,
    /// Main-branch integration (`integration::`, `failure::`, `tenants::`).
    Integration,
    /// Nightly real LLM (`llm_real::`, ignored).
    Nightly,
}

/// Read `E2E_MODE` (defaults to `integration` so local `cargo test` stays full-suite).
pub fn current_mode() -> String {
    std::env::var("E2E_MODE").unwrap_or_else(|_| "integration".to_string())
}

/// Whether this suite should execute under the current `E2E_MODE`.
pub fn suite_enabled(suite: E2eSuite) -> bool {
    match current_mode().to_ascii_lowercase().as_str() {
        "smoke" => matches!(suite, E2eSuite::Smoke),
        "integration" => matches!(suite, E2eSuite::Smoke | E2eSuite::Integration),
        "nightly" | "llm_real" => matches!(suite, E2eSuite::Nightly),
        other => {
            eprintln!(
                "[product_e2e] WARN: unknown E2E_MODE={other:?}, treating as integration"
            );
            matches!(suite, E2eSuite::Smoke | E2eSuite::Integration)
        }
    }
}

/// Panics when the smoke suite is disabled for the current `E2E_MODE`.
pub fn require_smoke_suite() {
    assert_suite_enabled(E2eSuite::Smoke);
}

/// Panics when the integration suite is disabled for the current `E2E_MODE`.
pub fn require_integration_suite() {
    assert_suite_enabled(E2eSuite::Integration);
}

/// Panics when the nightly (`llm_real`) suite is disabled for the current `E2E_MODE`.
pub fn require_nightly_suite() {
    assert_suite_enabled(E2eSuite::Nightly);
}

/// Panics when the requested suite is disabled (avoids silent pass in `E2E_MODE=smoke`).
pub fn assert_suite_enabled(suite: E2eSuite) {
    if suite_enabled(suite) {
        return;
    }
    panic!(
        "suite {suite:?} disabled by E2E_MODE={}; adjust E2E_MODE or narrow the test filter",
        current_mode()
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static E2E_MODE_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct E2eModeGuard(Option<String>);

    impl E2eModeGuard {
        fn set(mode: &str) -> Self {
            let previous = std::env::var("E2E_MODE").ok();
            unsafe {
                std::env::set_var("E2E_MODE", mode);
            }
            Self(previous)
        }
    }

    impl Drop for E2eModeGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.0 {
                    Some(value) => std::env::set_var("E2E_MODE", value),
                    None => std::env::remove_var("E2E_MODE"),
                }
            }
        }
    }

    #[test]
    fn smoke_mode_excludes_integration_only_suite() {
        let _lock = E2E_MODE_TEST_LOCK.lock().unwrap();
        let _guard = E2eModeGuard::set("smoke");
        assert!(suite_enabled(E2eSuite::Smoke));
        assert!(!suite_enabled(E2eSuite::Integration));
        assert!(!suite_enabled(E2eSuite::Nightly));
    }

    #[test]
    fn integration_mode_includes_smoke_and_integration() {
        let _lock = E2E_MODE_TEST_LOCK.lock().unwrap();
        let _guard = E2eModeGuard::set("integration");
        assert!(suite_enabled(E2eSuite::Smoke));
        assert!(suite_enabled(E2eSuite::Integration));
        assert!(!suite_enabled(E2eSuite::Nightly));
    }

    #[test]
    fn nightly_mode_only_enables_llm_real_suite() {
        let _lock = E2E_MODE_TEST_LOCK.lock().unwrap();
        let _guard = E2eModeGuard::set("nightly");
        assert!(!suite_enabled(E2eSuite::Smoke));
        assert!(!suite_enabled(E2eSuite::Integration));
        assert!(suite_enabled(E2eSuite::Nightly));
    }

    #[test]
    fn unknown_mode_treats_as_integration() {
        let _lock = E2E_MODE_TEST_LOCK.lock().unwrap();
        let _guard = E2eModeGuard::set("typo");
        assert!(suite_enabled(E2eSuite::Smoke));
        assert!(suite_enabled(E2eSuite::Integration));
        assert!(!suite_enabled(E2eSuite::Nightly));
    }
}
