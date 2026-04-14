use common::{
    AnswerBlock, ChatRequest, ChatResponse, ChatTurnInput, Citation, DegradeTraceItem,
    DocumentStatusResponse, GuardReport, ModeDebug, PlannerOutput, SourceRef, TraceInfo,
};
use contracts::chat::{
    AnswerBlock as ContractAnswerBlock, ChatRequest as ContractChatRequest,
    ChatResponse as ContractChatResponse, ChatTurnInput as ContractChatTurnInput,
    Citation as ContractCitation, DegradeTraceItem as ContractDegradeTraceItem,
    GuardReport as ContractGuardReport, ModeDebug as ContractModeDebug,
    PlannerOutput as ContractPlannerOutput, SourceRef as ContractSourceRef,
    TraceInfo as ContractTraceInfo,
};
use contracts::documents::DocumentStatusResponse as ContractDocumentStatusResponse;

fn same_type<T>(_left: Option<T>, _right: Option<T>) {}

#[test]
fn common_chat_request_reuses_the_shared_contract_type() {
    same_type::<ContractChatRequest>(None, None::<ChatRequest>);
}

#[test]
fn common_chat_turn_input_reuses_the_shared_contract_type() {
    same_type::<ContractChatTurnInput>(None, None::<ChatTurnInput>);
}

#[test]
fn common_chat_response_reuses_the_shared_contract_type() {
    same_type::<ContractChatResponse>(None, None::<ChatResponse>);
}

#[test]
fn common_chat_nested_types_reuse_the_shared_contract_type() {
    same_type::<ContractCitation>(None, None::<Citation>);
    same_type::<ContractAnswerBlock>(None, None::<AnswerBlock>);
    same_type::<ContractSourceRef>(None, None::<SourceRef>);
    same_type::<ContractTraceInfo>(None, None::<TraceInfo>);
    same_type::<ContractDegradeTraceItem>(None, None::<DegradeTraceItem>);
    same_type::<ContractPlannerOutput>(None, None::<PlannerOutput>);
    same_type::<ContractModeDebug>(None, None::<ModeDebug>);
    same_type::<ContractGuardReport>(None, None::<GuardReport>);
    same_type::<ContractDocumentStatusResponse>(None, None::<DocumentStatusResponse>);
}
