use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use alc_core::tenant::TenantConn;

pub use alc_core::repository::dtako_drivers::*;

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
            "SELECT DISTINCT e.id, e.tenant_id, e.driver_cd, e.name AS driver_name \
             FROM alc_api.employees e \
             INNER JOIN alc_api.dtako_operations op ON op.driver_id = e.id \
             WHERE e.deleted_at IS NULL \
             ORDER BY e.driver_cd",
        )
        .fetch_all(&mut *tc.conn)
        .await
    }
}
