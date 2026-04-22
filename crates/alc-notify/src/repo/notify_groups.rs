use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use alc_core::repository::notify_groups::*;
use alc_core::repository::notify_recipients::NotifyRecipient;
use alc_core::tenant::TenantConn;

pub struct PgNotifyGroupRepository {
    pool: PgPool,
}

impl PgNotifyGroupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotifyGroupRepository for PgNotifyGroupRepository {
    async fn list(&self, tenant_id: Uuid) -> Result<Vec<NotifyGroup>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, NotifyGroup>("SELECT * FROM notify_groups ORDER BY name")
            .fetch_all(&mut *tc.conn)
            .await
    }

    async fn get(&self, tenant_id: Uuid, id: Uuid) -> Result<Option<NotifyGroup>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, NotifyGroup>("SELECT * FROM notify_groups WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tc.conn)
            .await
    }

    async fn create(
        &self,
        tenant_id: Uuid,
        input: &CreateNotifyGroup,
    ) -> Result<NotifyGroup, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, NotifyGroup>(
            r#"
            INSERT INTO notify_groups (tenant_id, name, description)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(tenant_id)
        .bind(&input.name)
        .bind(&input.description)
        .fetch_one(&mut *tc.conn)
        .await
    }

    async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: &UpdateNotifyGroup,
    ) -> Result<NotifyGroup, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, NotifyGroup>(
            r#"
            UPDATE notify_groups SET
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(id)
        .fetch_one(&mut *tc.conn)
        .await
    }

    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query("DELETE FROM notify_groups WHERE id = $1")
            .bind(id)
            .execute(&mut *tc.conn)
            .await?;
        Ok(())
    }

    async fn add_members(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
        recipient_ids: &[Uuid],
    ) -> Result<(), sqlx::Error> {
        if recipient_ids.is_empty() {
            return Ok(());
        }
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query(
            r#"
            INSERT INTO notify_recipient_groups (group_id, recipient_id)
            SELECT $1, UNNEST($2::uuid[])
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(group_id)
        .bind(recipient_ids)
        .execute(&mut *tc.conn)
        .await?;
        Ok(())
    }

    async fn remove_member(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
        recipient_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query(
            "DELETE FROM notify_recipient_groups WHERE group_id = $1 AND recipient_id = $2",
        )
        .bind(group_id)
        .bind(recipient_id)
        .execute(&mut *tc.conn)
        .await?;
        Ok(())
    }

    async fn list_members(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<NotifyRecipient>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, NotifyRecipient>(
            r#"
            SELECT r.*
            FROM notify_recipients r
            JOIN notify_recipient_groups rg ON rg.recipient_id = r.id
            WHERE rg.group_id = $1
            ORDER BY r.name
            "#,
        )
        .bind(group_id)
        .fetch_all(&mut *tc.conn)
        .await
    }

    async fn list_enabled_members(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<NotifyRecipient>, sqlx::Error> {
        let mut tc = TenantConn::acquire(&self.pool, &tenant_id.to_string()).await?;
        sqlx::query_as::<_, NotifyRecipient>(
            r#"
            SELECT r.*
            FROM notify_recipients r
            JOIN notify_recipient_groups rg ON rg.recipient_id = r.id
            WHERE rg.group_id = $1 AND r.enabled = TRUE
            ORDER BY r.name
            "#,
        )
        .bind(group_id)
        .fetch_all(&mut *tc.conn)
        .await
    }
}
