use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use alc_core::tenant::TenantConn;

pub use alc_core::repository::api_tokens::*;

pub struct PgApiTokensRepository {
    pool: PgPool,
}

impl PgApiTokensRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ApiTokensRepository for PgApiTokensRepository {
    async fn list(&self, tenant_id: Uuid) -> Result<Vec<ApiTokenRow>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, ApiTokenRow>(
            "SELECT id, name, token_prefix, expires_at, revoked_at, last_used_at, created_at
             FROM api_tokens
             ORDER BY created_at DESC",
        )
        .fetch_all(&mut *tc.conn)
        .await
    }

    async fn create(
        &self,
        tenant_id: Uuid,
        name: &str,
        token_hash: &str,
        token_prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiTokenRow, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, ApiTokenRow>(
            "INSERT INTO api_tokens (tenant_id, name, token_hash, token_prefix, expires_at)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, name, token_prefix, expires_at, revoked_at, last_used_at, created_at",
        )
        .bind(tenant_id)
        .bind(name)
        .bind(token_hash)
        .bind(token_prefix)
        .bind(expires_at)
        .fetch_one(&mut *tc.conn)
        .await
    }

    async fn revoke(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        let affected = sqlx::query(
            "UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&mut *tc.conn)
        .await?
        .rows_affected();
        Ok(affected > 0)
    }
}
