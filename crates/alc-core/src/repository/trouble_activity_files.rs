use async_trait::async_trait;
use uuid::Uuid;

use crate::models::TroubleActivityFile;

#[async_trait]
pub trait TroubleActivityFilesRepository: Send + Sync {
    async fn create(
        &self,
        tenant_id: Uuid,
        activity_id: Uuid,
        filename: &str,
        content_type: &str,
        size_bytes: i64,
        storage_key: &str,
    ) -> Result<TroubleActivityFile, sqlx::Error>;

    async fn list_by_activity(
        &self,
        tenant_id: Uuid,
        activity_id: Uuid,
    ) -> Result<Vec<TroubleActivityFile>, sqlx::Error>;

    async fn get(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<TroubleActivityFile>, sqlx::Error>;

    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error>;
}
