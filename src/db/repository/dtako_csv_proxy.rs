use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use super::TenantConn;

#[async_trait]
pub trait DtakoCsvProxyRepository: Send + Sync {
    async fn get_r2_key_prefix(
        &self,
        tenant_id: Uuid,
        unko_no: &str,
    ) -> Result<Option<String>, sqlx::Error>;
}

pub struct PgDtakoCsvProxyRepository {
    pool: PgPool,
}

impl PgDtakoCsvProxyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DtakoCsvProxyRepository for PgDtakoCsvProxyRepository {
    async fn get_r2_key_prefix(
        &self,
        tenant_id: Uuid,
        unko_no: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_scalar(
            "SELECT r2_key_prefix FROM alc_api.dtako_operations WHERE tenant_id = $1 AND unko_no = $2 LIMIT 1",
        )
        .bind(tenant_id)
        .bind(unko_no)
        .fetch_optional(&mut *tc.conn)
        .await
    }
}
