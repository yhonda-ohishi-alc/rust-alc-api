use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// 公開ルート (認証不要)
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/tenko-call/register", post(register))
        .route("/tenko-call/tenko", post(tenko))
}

// --- ドライバー登録 ---

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    phone_number: String,
    driver_name: String,
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    success: bool,
    driver_id: i32,
    call_number: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, StatusCode> {
    let row = sqlx::query_as::<_, (i32, Option<String>)>(
        r#"
        INSERT INTO tenko_call_drivers (phone_number, driver_name, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (phone_number) DO UPDATE SET driver_name = $2, updated_at = now()
        RETURNING id, call_number
        "#,
    )
    .bind(&body.phone_number)
    .bind(&body.driver_name)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("tenko_call register error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(RegisterResponse {
        success: true,
        driver_id: row.0,
        call_number: row.1,
    }))
}

// --- 点呼送信 ---

#[derive(Debug, Deserialize)]
struct TenkoRequest {
    phone_number: String,
    driver_name: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Serialize)]
struct TenkoResponse {
    success: bool,
    call_number: Option<String>,
}

async fn tenko(
    State(state): State<AppState>,
    Json(body): Json<TenkoRequest>,
) -> Result<Json<TenkoResponse>, StatusCode> {
    // ドライバーを検索 (なければ自動登録)
    let driver = sqlx::query_as::<_, (i32, Option<String>)>(
        r#"
        INSERT INTO tenko_call_drivers (phone_number, driver_name, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (phone_number) DO UPDATE SET driver_name = $2, updated_at = now()
        RETURNING id, call_number
        "#,
    )
    .bind(&body.phone_number)
    .bind(&body.driver_name)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("tenko_call tenko driver upsert error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 位置情報ログを保存
    sqlx::query(
        r#"
        INSERT INTO tenko_call_logs (driver_id, phone_number, driver_name, latitude, longitude)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(driver.0)
    .bind(&body.phone_number)
    .bind(&body.driver_name)
    .bind(body.latitude)
    .bind(body.longitude)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("tenko_call tenko log insert error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(TenkoResponse {
        success: true,
        call_number: driver.1,
    }))
}
