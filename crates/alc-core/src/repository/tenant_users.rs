use async_trait::async_trait;
use uuid::Uuid;

use crate::models::TenantAllowedEmail;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait TenantUsersRepository: Send + Sync {
    async fn list_users(&self, tenant_id: Uuid) -> Result<Vec<UserRow>, sqlx::Error>;

    async fn list_invitations(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<TenantAllowedEmail>, sqlx::Error>;

    async fn invite_user(
        &self,
        tenant_id: Uuid,
        email: &str,
        role: &str,
    ) -> Result<TenantAllowedEmail, sqlx::Error>;

    async fn delete_invitation(&self, tenant_id: Uuid, id: Uuid) -> Result<(), sqlx::Error>;

    async fn delete_user(&self, tenant_id: Uuid, id: Uuid) -> Result<(), sqlx::Error>;

    /// email を条件に users.role / tenant_allowed_emails.role を更新する。
    /// いずれかの行を更新できたら true、どちらも該当なしなら false。
    async fn update_role_by_email(
        &self,
        tenant_id: Uuid,
        email: &str,
        role: &str,
    ) -> Result<bool, sqlx::Error>;

    /// email を条件に users / tenant_allowed_emails から削除する。
    /// いずれかから削除できたら true、どちらも該当なしなら false。
    async fn delete_by_email(&self, tenant_id: Uuid, email: &str) -> Result<bool, sqlx::Error>;
}
