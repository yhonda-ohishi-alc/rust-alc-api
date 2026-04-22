use async_trait::async_trait;
use uuid::Uuid;

use super::notify_recipients::NotifyRecipient;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NotifyGroup {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateNotifyGroup {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateNotifyGroup {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[async_trait]
pub trait NotifyGroupRepository: Send + Sync {
    async fn list(&self, tenant_id: Uuid) -> Result<Vec<NotifyGroup>, sqlx::Error>;

    async fn get(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<NotifyGroup>, sqlx::Error>;

    async fn create(
        &self,
        tenant_id: Uuid,
        input: &CreateNotifyGroup,
    ) -> Result<NotifyGroup, sqlx::Error>;

    async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: &UpdateNotifyGroup,
    ) -> Result<NotifyGroup, sqlx::Error>;

    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<(), sqlx::Error>;

    /// Add one or more recipients to the group. Idempotent (ON CONFLICT DO NOTHING).
    async fn add_members(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
        recipient_ids: &[Uuid],
    ) -> Result<(), sqlx::Error>;

    async fn remove_member(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
        recipient_id: Uuid,
    ) -> Result<(), sqlx::Error>;

    /// Recipients belonging to a group (joined with notify_recipients).
    async fn list_members(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<NotifyRecipient>, sqlx::Error>;

    /// Recipients belonging to a group AND enabled. Used by distribute target=group_id.
    async fn list_enabled_members(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<NotifyRecipient>, sqlx::Error>;
}
