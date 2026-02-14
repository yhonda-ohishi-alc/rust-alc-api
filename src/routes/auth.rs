use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::models::Tenant;
use crate::db::DbPool;

pub fn router() -> Router<DbPool> {
    Router::new().route("/auth/tenants", post(create_tenant))
}

#[derive(Debug, Deserialize)]
pub struct CreateTenant {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: Uuid,
    pub name: String,
    pub api_key: String,
}

async fn create_tenant(
    State(pool): State<DbPool>,
    Json(body): Json<CreateTenant>,
) -> Result<(StatusCode, Json<TenantResponse>), StatusCode> {
    let tenant = sqlx::query_as::<_, Tenant>(
        "INSERT INTO tenants (name) VALUES ($1) RETURNING *",
    )
    .bind(&body.name)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // TODO: Implement proper API key generation and storage
    let api_key = format!("alc_{}", Uuid::new_v4().simple());

    Ok((
        StatusCode::CREATED,
        Json(TenantResponse {
            id: tenant.id,
            name: tenant.name,
            api_key,
        }),
    ))
}
