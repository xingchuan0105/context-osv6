use async_trait::async_trait;
use common::{AppError, CreateWorkspaceRequest};
use contracts::workspaces::Workspace;

#[async_trait]
pub trait WorkspaceStore: Send + Sync {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, AppError>;
    async fn create_workspace(&self, req: CreateWorkspaceRequest) -> Result<Workspace, AppError>;
}
