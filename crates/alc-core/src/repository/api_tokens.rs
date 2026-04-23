//! nuxt-dtako-admin /api/api-tokens 用の Repository。
//!
//! 平文トークンは DB に保存せず、SHA-256 ハッシュで照合する。
//! revoke はソフトデリート (revoked_at を埋める)。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiTokenRow {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait ApiTokensRepository: Send + Sync {
    async fn list(&self, tenant_id: Uuid) -> Result<Vec<ApiTokenRow>, sqlx::Error>;

    async fn create(
        &self,
        tenant_id: Uuid,
        name: &str,
        token_hash: &str,
        token_prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiTokenRow, sqlx::Error>;

    /// revoked_at = now() を立てる。該当行があれば true、なければ false。
    async fn revoke(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error>;
}
