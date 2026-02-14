use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::db::models::{CreateMeasurement, Measurement, MeasurementFilter};
use crate::db::DbPool;
use crate::middleware::auth::TenantId;

pub fn router() -> Router<DbPool> {
    Router::new()
        .route("/measurements", post(create_measurement).get(list_measurements))
        .route("/measurements/{id}", get(get_measurement))
}

async fn create_measurement(
    State(pool): State<DbPool>,
    tenant: axum::Extension<TenantId>,
    Json(body): Json<CreateMeasurement>,
) -> Result<(StatusCode, Json<Measurement>), StatusCode> {
    let tenant_id = tenant.0 .0;

    let measurement = sqlx::query_as::<_, Measurement>(
        r#"
        INSERT INTO measurements (tenant_id, employee_id, alcohol_level, result, face_photo_url)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(tenant_id)
    .bind(body.employee_id)
    .bind(body.alcohol_level)
    .bind(&body.result)
    .bind(&body.face_photo_url)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(measurement)))
}

async fn list_measurements(
    State(pool): State<DbPool>,
    tenant: axum::Extension<TenantId>,
    Query(filter): Query<MeasurementFilter>,
) -> Result<Json<Vec<Measurement>>, StatusCode> {
    let tenant_id = tenant.0 .0;
    let limit = filter.limit.unwrap_or(50).min(100);
    let offset = filter.offset.unwrap_or(0);

    // Build dynamic query
    let mut conditions = vec!["m.tenant_id = $1".to_string()];
    let mut param_idx = 2u32;

    if filter.employee_id.is_some() {
        conditions.push(format!("m.employee_id = ${param_idx}"));
        param_idx += 1;
    }
    if filter.result.is_some() {
        conditions.push(format!("m.result = ${param_idx}"));
        param_idx += 1;
    }
    if filter.from.is_some() {
        conditions.push(format!("m.measured_at >= ${param_idx}"));
        param_idx += 1;
    }
    if filter.to.is_some() {
        conditions.push(format!("m.measured_at <= ${param_idx}"));
        param_idx += 1;
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT m.* FROM measurements m WHERE {where_clause} ORDER BY m.measured_at DESC LIMIT ${param_idx} OFFSET ${}",
        param_idx + 1
    );

    let mut query = sqlx::query_as::<_, Measurement>(&sql).bind(tenant_id);

    if let Some(employee_id) = filter.employee_id {
        query = query.bind(employee_id);
    }
    if let Some(ref result) = filter.result {
        query = query.bind(result);
    }
    if let Some(from) = filter.from {
        query = query.bind(from);
    }
    if let Some(to) = filter.to {
        query = query.bind(to);
    }

    query = query.bind(limit).bind(offset);

    let measurements = query
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(measurements))
}

async fn get_measurement(
    State(pool): State<DbPool>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Measurement>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let measurement = sqlx::query_as::<_, Measurement>(
        "SELECT * FROM measurements WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(measurement))
}
