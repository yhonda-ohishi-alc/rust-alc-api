use async_trait::async_trait;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::TenantConn;

/// daiun-salary 互換の Driver レスポンス
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Driver {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub driver_cd: Option<String>,
    #[sqlx(rename = "name")]
    #[serde(rename = "driver_name")]
    pub driver_name: String,
}

#[async_trait]
pub trait DtakoDriversRepository: Send + Sync {
    async fn list(&self, tenant_id: Uuid) -> Result<Vec<Driver>, sqlx::Error>;
}

pub struct PgDtakoDriversRepository {
    pool: PgPool,
}

impl PgDtakoDriversRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DtakoDriversRepository for PgDtakoDriversRepository {
    async fn list(&self, tenant_id: Uuid) -> Result<Vec<Driver>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, Driver>(
            "SELECT id, tenant_id, driver_cd, name FROM alc_api.employees WHERE driver_cd IS NOT NULL AND deleted_at IS NULL ORDER BY driver_cd",
        )
        .fetch_all(&mut *tc.conn)
        .await
    }
}
