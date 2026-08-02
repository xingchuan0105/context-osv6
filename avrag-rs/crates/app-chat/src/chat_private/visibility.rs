use crate::context::ChatContext;

impl ChatContext {
    pub(crate) fn current_owner_user_id(&self) -> String {
        self.auth.user_id().to_string()
    }
}
