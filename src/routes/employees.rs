use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{post, put},
    Json, Router,
};
use uuid::Uuid;

use crate::db::models::{CreateEmployee, Employee, UpdateFace};
use crate::db::DbPool;
use crate::middleware::auth::TenantId;

pub fn router() -> Router<DbPool> {
    Router::new()
        .route("/employees", post(create_employee).get(list_employees))
        .route("/employees/{id}/face", put(update_face))
}

async fn create_employee(
    State(pool): State<DbPool>,
    tenant: axum::Extension<TenantId>,
    Json(body): Json<CreateEmployee>,
) -> Result<(StatusCode, Json<Employee>), StatusCode> {
    let tenant_id = tenant.0 .0;

    let employee = sqlx::query_as::<_, Employee>(
        r#"
        INSERT INTO employees (tenant_id, nfc_id, name)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(tenant_id)
    .bind(&body.nfc_id)
    .bind(&body.name)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(employee)))
}

async fn list_employees(
    State(pool): State<DbPool>,
    tenant: axum::Extension<TenantId>,
) -> Result<Json<Vec<Employee>>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let employees = sqlx::query_as::<_, Employee>(
        "SELECT * FROM employees WHERE tenant_id = $1 ORDER BY name",
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(employees))
}

async fn update_face(
    State(pool): State<DbPool>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateFace>,
) -> Result<Json<Employee>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let employee = sqlx::query_as::<_, Employee>(
        r#"
        UPDATE employees SET face_photo_url = $1, updated_at = NOW()
        WHERE id = $2 AND tenant_id = $3
        RETURNING *
        "#,
    )
    .bind(&body.face_photo_url)
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(employee))
}
