use async_trait::async_trait;
use uuid::Uuid;

use crate::models::{CreateTroubleTask, TroubleTask, UpdateTroubleTask};

#[async_trait]
pub trait TroubleTasksRepository: Send + Sync {
    async fn create(
        &self,
        tenant_id: Uuid,
        ticket_id: Uuid,
        created_by: Option<Uuid>,
        input: &CreateTroubleTask,
    ) -> Result<TroubleTask, sqlx::Error>;

    async fn list_by_ticket(
        &self,
        tenant_id: Uuid,
        ticket_id: Uuid,
    ) -> Result<Vec<TroubleTask>, sqlx::Error>;

    async fn get(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<TroubleTask>, sqlx::Error>;

    async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: &UpdateTroubleTask,
    ) -> Result<Option<TroubleTask>, sqlx::Error>;

    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error>;
}
