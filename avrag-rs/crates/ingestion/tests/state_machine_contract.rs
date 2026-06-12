use common::DocumentStatus;
use ingestion::{
    DocumentStateMachine, IngestionError, Transition,
    parser::{LocalParseKind, ParsePlan, ParseRoute, ParseRouter, RouteReason},
};

#[test]
fn document_state_machine_allows_queued_to_processing() {
    DocumentStateMachine::validate(&Transition {
        from: DocumentStatus::Queued,
        to: DocumentStatus::Processing,
    })
    .expect("queued -> processing is part of the ingest lifecycle");
}

#[test]
fn document_state_machine_rejects_completed_to_processing() {
    let error = DocumentStateMachine::validate(&Transition {
        from: DocumentStatus::Completed,
        to: DocumentStatus::Processing,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        IngestionError::InvalidStateTransition { .. }
    ));
}

#[test]
fn parse_router_routes_plain_text_to_local_fast_path() {
    let decision = ParseRouter::route(b"hello world", "notes.txt", "text/plain").unwrap();

    assert_eq!(decision.route, ParseRoute::Local);
    assert_eq!(decision.reason, RouteReason::TextFile);
    assert!(matches!(
        decision.plan,
        ParsePlan::Local(plan) if plan.kind == LocalParseKind::Text
    ));
}
