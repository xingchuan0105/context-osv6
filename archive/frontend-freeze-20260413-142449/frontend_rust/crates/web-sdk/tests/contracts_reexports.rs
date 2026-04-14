use contracts::{
    auth::{
        AuthEnvelope as ContractAuthEnvelope, AuthPayload as ContractAuthPayload,
        AuthUserDto as ContractAuthUserDto,
    },
    chat::{
        AnswerBlock as ContractAnswerBlock, ChatRequest as ContractChatRequest,
        ChatResponse as ContractChatResponse, ChatTurnInput as ContractChatTurnInput,
        Citation as ContractCitation, DegradeTraceItem as ContractDegradeTraceItem,
        GuardReport as ContractGuardReport, ModeDebug as ContractModeDebug,
        PlannerOutput as ContractPlannerOutput, SourceRef as ContractSourceRef,
        TraceInfo as ContractTraceInfo,
    },
    documents::{
        CreateDocumentUploadResponse as ContractCreateDocumentUploadResponse,
        DocumentStatusResponse as ContractDocumentStatusResponse,
    },
    notebooks::{
        Notebook as ContractNotebook, NotebookListResponse as ContractNotebookListResponse,
        NotebookResponse as ContractNotebookResponse,
    },
};
use web_sdk::dtos::{
    AnswerBlock, AuthEnvelope, AuthPayload, AuthUserDto, ChatRequest, ChatResponse, ChatTurnInput,
    Citation, CreateDocumentUploadResponse, DegradeTraceItem, DocumentStatusResponse, GuardReport,
    ModeDebug, Notebook, NotebookListResponse, NotebookResponse, PlannerOutput, SourceRef,
    TraceInfo,
};

fn same_type<T>(_left: Option<T>, _right: Option<T>) {}

#[test]
fn dtos_reexport_selected_contract_types() {
    same_type::<ContractAuthUserDto>(None, None::<AuthUserDto>);
    same_type::<ContractAuthPayload>(None, None::<AuthPayload>);
    same_type::<ContractAuthEnvelope>(None, None::<AuthEnvelope>);
    same_type::<ContractNotebook>(None, None::<Notebook>);
    same_type::<ContractNotebookListResponse>(None, None::<NotebookListResponse>);
    same_type::<ContractNotebookResponse>(None, None::<NotebookResponse>);
    same_type::<ContractCreateDocumentUploadResponse>(None, None::<CreateDocumentUploadResponse>);
    same_type::<ContractDocumentStatusResponse>(None, None::<DocumentStatusResponse>);
}

#[test]
fn auth_envelope_deserializes_when_backend_omits_full_name() {
    let envelope: AuthEnvelope = serde_json::from_value(serde_json::json!({
        "success": true,
        "data": {
            "token": "token-123",
            "user": {
                "id": "user-1",
                "email": "user@example.com"
            }
        },
        "error": null
    }))
    .expect("backend auth payload without full_name should deserialize");

    let payload = envelope.data.expect("expected auth payload");
    assert_eq!(payload.user.full_name, "");
    assert_eq!(payload.user.email, "user@example.com");
}

#[test]
fn chat_request_matches_shared_contract_shape() {
    let request = ChatRequest {
        query: "hello".to_string(),
        notebook_id: Some("nb-1".to_string()),
        session_id: None,
        agent_type: "rag".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: vec!["doc-1".to_string()],
        messages: vec![ChatTurnInput {
            role: "user".to_string(),
            content: "hello".to_string(),
        }],
        stream: true,
    };

    let value = serde_json::to_value(&request).expect("chat request should serialize");
    assert!(
        value.get("request_id").is_none(),
        "chat request should not expose transport-only request_id"
    );

    let contract_request: ContractChatRequest =
        serde_json::from_value(value.clone()).expect("contract chat request should deserialize");
    assert_eq!(contract_request.query, request.query);
    assert_eq!(contract_request.doc_scope, request.doc_scope);

    let contract_turn: ContractChatTurnInput =
        serde_json::from_value(serde_json::json!({"role": "user", "content": "hello"}))
            .expect("contract chat turn should deserialize");
    assert_eq!(contract_turn.role, "user");
}

#[test]
fn chat_response_and_nested_types_reexport_the_shared_contract() {
    same_type::<ContractChatResponse>(None, None::<ChatResponse>);
    same_type::<ContractCitation>(None, None::<Citation>);
    same_type::<ContractAnswerBlock>(None, None::<AnswerBlock>);
    same_type::<ContractSourceRef>(None, None::<SourceRef>);
    same_type::<ContractTraceInfo>(None, None::<TraceInfo>);
    same_type::<ContractDegradeTraceItem>(None, None::<DegradeTraceItem>);
    same_type::<ContractPlannerOutput>(None, None::<PlannerOutput>);
    same_type::<ContractModeDebug>(None, None::<ModeDebug>);
    same_type::<ContractGuardReport>(None, None::<GuardReport>);
}
