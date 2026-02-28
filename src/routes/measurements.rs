use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::db::models::{CreateMeasurement, Measurement, MeasurementFilter, MeasurementsResponse};
use crate::db::tenant::set_current_tenant;
use crate::AppState;
use crate::middleware::auth::TenantId;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/measurements", post(create_measurement).get(list_measurements))
        .route("/measurements/{id}", get(get_measurement))
}

async fn create_measurement(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    raw_body: String,
) -> Result<(StatusCode, Json<Measurement>), StatusCode> {
    let body: CreateMeasurement = match serde_json::from_str(&raw_body) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("create_measurement deserialize error: {e}, body: {raw_body}");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    };
    let tenant_id = tenant.0 .0;

    let valid_results = ["pass", "fail", "normal", "over", "error"];
    if !valid_results.contains(&body.result_type.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut conn = state.pool.acquire().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    set_current_tenant(&mut conn, &tenant_id.to_string()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let measurement = sqlx::query_as::<_, Measurement>(
        r#"
        INSERT INTO measurements (tenant_id, employee_id, alcohol_level, result, face_photo_url, measured_at, device_use_count)
        VALUES ($1, $2, $3, $4, $5, COALESCE($6, NOW()), COALESCE($7, 0))
        RETURNING *
        "#,
    )
    .bind(tenant_id)
    .bind(body.employee_id)
    .bind(body.alcohol_value)
    .bind(&body.result_type)
    .bind(&body.face_photo_url)
    .bind(body.measured_at)
    .bind(body.device_use_count)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        tracing::error!("create_measurement DB error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(measurement)))
}

async fn list_measurements(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Query(filter): Query<MeasurementFilter>,
) -> Result<Json<MeasurementsResponse>, StatusCode> {
    let tenant_id = tenant.0 .0;
    let per_page = filter.per_page.unwrap_or(50).min(100);
    let page = filter.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    let mut conn = state.pool.acquire().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    set_current_tenant(&mut conn, &tenant_id.to_string()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build dynamic query
    let mut conditions = vec!["m.tenant_id = $1".to_string()];
    let mut param_idx = 2u32;

    if filter.employee_id.is_some() {
        conditions.push(format!("m.employee_id = ${param_idx}"));
        param_idx += 1;
    }
    if filter.result_type.is_some() {
        conditions.push(format!("m.result = ${param_idx}"));
        param_idx += 1;
    }
    if filter.date_from.is_some() {
        conditions.push(format!("m.measured_at >= ${param_idx}"));
        param_idx += 1;
    }
    if filter.date_to.is_some() {
        conditions.push(format!("m.measured_at <= ${param_idx}"));
        param_idx += 1;
    }

    let where_clause = conditions.join(" AND ");

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM measurements m WHERE {where_clause}");
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(tenant_id);
    if let Some(employee_id) = filter.employee_id {
        count_query = count_query.bind(employee_id);
    }
    if let Some(ref result_type) = filter.result_type {
        count_query = count_query.bind(result_type);
    }
    if let Some(date_from) = filter.date_from {
        count_query = count_query.bind(date_from);
    }
    if let Some(date_to) = filter.date_to {
        count_query = count_query.bind(date_to);
    }
    let total = count_query
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Data query
    let sql = format!(
        "SELECT m.* FROM measurements m WHERE {where_clause} ORDER BY m.measured_at DESC LIMIT ${param_idx} OFFSET ${}",
        param_idx + 1
    );

    let mut query = sqlx::query_as::<_, Measurement>(&sql).bind(tenant_id);

    if let Some(employee_id) = filter.employee_id {
        query = query.bind(employee_id);
    }
    if let Some(ref result_type) = filter.result_type {
        query = query.bind(result_type);
    }
    if let Some(date_from) = filter.date_from {
        query = query.bind(date_from);
    }
    if let Some(date_to) = filter.date_to {
        query = query.bind(date_to);
    }

    query = query.bind(per_page).bind(offset);

    let measurements = query
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(MeasurementsResponse {
        measurements,
        total,
        page,
        per_page,
    }))
}

async fn get_measurement(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Measurement>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let mut conn = state.pool.acquire().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    set_current_tenant(&mut conn, &tenant_id.to_string()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let measurement = sqlx::query_as::<_, Measurement>(
        "SELECT * FROM measurements WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(measurement))
}
