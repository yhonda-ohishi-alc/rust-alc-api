//! ingest_keys 管理エンドポイント (テナント保護)
//!
//! Email Worker が使う ingest key の発行・一覧・削除。
//! plaintext key は発行レスポンスのみ返却され、DB には SHA-256 ハッシュで保存される。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json, Router,
};
use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use alc_core::auth_middleware::TenantId;
use alc_core::tenant::TenantConn;
use alc_core::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/notify/ingest-keys", axum::routing::get(list))
        .route("/notify/ingest-keys", axum::routing::post(create))
        .route(
            "/notify/ingest-keys/{id}",
            axum::routing::delete(delete_key),
        )
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct IngestKeyInfo {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
pub struct CreateIngestKey {
    pub name: String,
}

#[derive(serde::Serialize)]
pub struct CreateIngestKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
) -> Result<Json<Vec<IngestKeyInfo>>, StatusCode> {
    let mut tc = TenantConn::acquire(state.pool(), &tenant.0.to_string())
        .await
        .map_err(|e| {
            tracing::error!("acquire tenant conn: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let rows: Vec<IngestKeyInfo> = sqlx::query_as(
        "SELECT id, name, enabled, created_at FROM notify_ingest_keys ORDER BY created_at DESC",
    )
    .fetch_all(&mut *tc.conn)
    .await
    .map_err(|e| {
        tracing::error!("list ingest_keys: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Json(input): Json<CreateIngestKey>,
) -> Result<(StatusCode, Json<CreateIngestKeyResponse>), StatusCode> {
    if input.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 32-byte 乱数 → base64url (パディング無し) → SHA-256 ハッシュ
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let plain = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let key_hash = hex_sha256(&plain);

    let mut tc = TenantConn::acquire(state.pool(), &tenant.0.to_string())
        .await
        .map_err(|e| {
            tracing::error!("acquire tenant conn: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let row: (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        INSERT INTO notify_ingest_keys (tenant_id, key_hash, name)
        VALUES ($1, $2, $3)
        RETURNING id, created_at
        "#,
    )
    .bind(tenant.0)
    .bind(&key_hash)
    .bind(input.name.trim())
    .fetch_one(&mut *tc.conn)
    .await
    .map_err(|e| {
        tracing::error!("insert ingest_key: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateIngestKeyResponse {
            id: row.0,
            name: input.name.trim().to_string(),
            key: plain,
            created_at: row.1,
        }),
    ))
}

async fn delete_key(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut tc = TenantConn::acquire(state.pool(), &tenant.0.to_string())
        .await
        .map_err(|e| {
            tracing::error!("acquire tenant conn: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let result = sqlx::query("DELETE FROM notify_ingest_keys WHERE id = $1")
        .bind(id)
        .execute(&mut *tc.conn)
        .await
        .map_err(|e| {
            tracing::error!("delete ingest_key: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let bytes = hasher.finalize();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_sha256_matches_ingest_module() {
        // ingest.rs の hex_sha256 と整合性を保つ
        assert_eq!(
            hex_sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn random_key_is_long_enough_url_safe() {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let key = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        // 32バイトを base64url(no pad) で符号化 → 約43文字
        assert!(key.len() >= 40);
        // URL-safe: + / は含まれない
        assert!(!key.contains('+'));
        assert!(!key.contains('/'));
        assert!(!key.contains('='));
    }
}
