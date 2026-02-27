use sqlx::PgConnection;

/// Set the current tenant for RLS policies.
/// Must be called before any tenant-scoped query.
pub async fn set_current_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_current_tenant($1)")
        .bind(tenant_id)
        .execute(conn)
        .await?;
    Ok(())
}
