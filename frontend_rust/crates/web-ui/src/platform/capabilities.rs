#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiCapabilities {
    pub profile_edit: bool,
    pub password_reset: bool,
    pub shared_kb: bool,
    pub document_upload: bool,
    pub long_text_virtualization: bool,
    pub pretext_prediction: bool,
}

pub const UI_CAPABILITIES: UiCapabilities = UiCapabilities {
    profile_edit: true,
    password_reset: true,
    shared_kb: true,
    document_upload: true,
    long_text_virtualization: true,
    pretext_prediction: true,
};

pub const fn ui_capabilities() -> &'static UiCapabilities {
    &UI_CAPABILITIES
}
