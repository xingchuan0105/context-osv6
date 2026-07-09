#[derive(Debug, Clone, Copy)]
pub struct ActivationInputs {
    pub created_workspace: bool,
    pub uploaded_document: bool,
    pub completed_chat: bool,
}

pub fn is_activated(inputs: &ActivationInputs) -> bool {
    inputs.created_workspace && inputs.uploaded_document && inputs.completed_chat
}
