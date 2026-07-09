//! Contract tests for the HeavyTail `write` mode surface (no live LLM).

use app::agents::AgentKind;
use app::agents::capability::{CapabilityRegistry, write_mode_schema};

#[test]
fn write_mode_schema_requires_internet_and_web_search() {
    let schema = write_mode_schema();
    assert_eq!(schema.id, "write");
    assert!(schema.requires_internet);
    assert!(
        schema
            .external_tools_used
            .iter()
            .any(|t| t == "web_search"),
        "write mode should declare web_search dependency"
    );
}

#[test]
fn write_mode_registered_in_capability_registry() {
    let registry = CapabilityRegistry::standard_cached();
    let mode = registry
        .mode("write")
        .expect("write mode must be registered");
    assert!(mode.requires_internet);
}

#[test]
fn agent_kind_write_round_trip() {
    assert_eq!(AgentKind::Write.as_canonical_str(), "write");
    assert_eq!(AgentKind::parse("write"), Some(AgentKind::Write));
    assert_eq!(AgentKind::parse("WRITE"), Some(AgentKind::Write));
}
