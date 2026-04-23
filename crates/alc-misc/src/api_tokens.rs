//! nuxt-dtako-admin 向け `/api/api-tokens` エンドポイント。
//!
//! Machine-to-machine 用の API トークン (平文は発行時のみ返却、
//! DB には SHA-256 ハッシュで保存)。revoke はソフトデリート。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Extension, Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use alc_core::auth_middleware::AuthUser;
use alc_core::repository::api_tokens::ApiTokenRow;
use alc_core::AppState;

const TOKEN_PREFIX_PUBLIC: &str = "alc_";
/// 平文トークン中、テーブルの token_prefix に保存する先頭文字数。
/// 管理画面で「alc_abcdefgh...」のような形でマスク表示に使う。
const TOKEN_PREFIX_VISIBLE_LEN: usize = 12;

#[derive(Debug, Serialize)]
struct ApiTokenListItem {
    id: Uuid,
    name: String,
    token_prefix: String,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<ApiTokenRow> for ApiTokenListItem {
    fn from(row: ApiTokenRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            token_prefix: row.token_prefix,
            expires_at: row.expires_at,
            revoked_at: row.revoked_at,
            last_used_at: row.last_used_at,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateRequest {
    name: String,
    /// None または null で無期限。
    expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateResponse {
    id: Uuid,
    name: String,
    /// 発行された平文トークン。作成直後のみ返却し、DB には保存しない。
    token: String,
    token_prefix: String,
}

/// 平文トークンを生成し、(plaintext, sha256 hex, visible prefix) を返す。
///
/// plaintext は `alc_` + UUID.simple() の 32 hex 文字で 36 文字。十分なエントロピー。
/// hash は plaintext の SHA-256 hex (DB 照合用)。
/// visible prefix は plaintext の先頭 12 文字 (管理画面表示用マスク)。
fn generate_token() -> (String, String, String) {
    // 32 bytes of randomness (Uuid::new_v4() は OsRng ベース)
    let body = URL_SAFE_NO_PAD.encode(Uuid::new_v4().as_bytes());
    let plaintext = format!("{TOKEN_PREFIX_PUBLIC}{body}");

    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    let prefix = plaintext
        .chars()
        .take(TOKEN_PREFIX_VISIBLE_LEN)
        .collect::<String>();

    (plaintext, hash, prefix)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api-tokens", get(list_tokens).post(create_token))
        .route("/api-tokens/{id}", axum::routing::delete(revoke_token))
}

/// GET /api-tokens — 当テナントのトークン一覧 (平文は返却しない)。
async fn list_tokens(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<ApiTokenListItem>>, StatusCode> {
    if auth_user.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let rows = state
        .api_tokens
        .list(auth_user.tenant_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list api tokens: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(rows.into_iter().map(ApiTokenListItem::from).collect()))
}

/// POST /api-tokens — 新規トークン発行。平文は本レスポンスでのみ返却される。
async fn create_token(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<CreateRequest>,
) -> Result<Json<CreateResponse>, StatusCode> {
    if auth_user.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let name = body.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let expires_at = match body.expires_in_days {
        Some(days) if days <= 0 => return Err(StatusCode::BAD_REQUEST),
        Some(days) => Some(Utc::now() + Duration::days(days)),
        None => None,
    };

    let (plaintext, token_hash, token_prefix) = generate_token();

    let row = state
        .api_tokens
        .create(
            auth_user.tenant_id,
            name,
            &token_hash,
            &token_prefix,
            expires_at,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create api token: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(CreateResponse {
        id: row.id,
        name: row.name,
        token: plaintext,
        token_prefix: row.token_prefix,
    }))
}

/// DELETE /api-tokens/{id} — ソフトデリート (revoked_at を埋める)。
async fn revoke_token(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    if auth_user.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let found = state
        .api_tokens
        .revoke(auth_user.tenant_id, id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to revoke api token: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !found {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_has_expected_shape() {
        let (plaintext, hash, prefix) = generate_token();
        assert!(plaintext.starts_with("alc_"));
        assert!(plaintext.len() > TOKEN_PREFIX_VISIBLE_LEN);
        assert_eq!(prefix.len(), TOKEN_PREFIX_VISIBLE_LEN);
        assert!(plaintext.starts_with(&prefix));
        // SHA-256 hex is 64 chars
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_returns_unique_values() {
        let (a, _, _) = generate_token();
        let (b, _, _) = generate_token();
        assert_ne!(a, b);
    }

    #[test]
    fn generate_token_hash_matches_plaintext() {
        let (plaintext, hash, _) = generate_token();
        let mut hasher = Sha256::new();
        hasher.update(plaintext.as_bytes());
        assert_eq!(hash, format!("{:x}", hasher.finalize()));
    }
}
