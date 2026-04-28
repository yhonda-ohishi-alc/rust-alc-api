//! ingest_keys 管理エンドポイント (テナント保護)
//!
//! Email Worker が使う ingest key の発行・一覧・削除。
//! plaintext key は発行レスポンスのみ返却され、DB には SHA-256 ハッシュで保存される。
//!
//! Phase 2.1: ingest_key 発行時に Cloudflare KV へ自動登録する
//! (Worker が `tenant-{slug}` ローカルパートでメールを受けた時に引ける)。

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
    /// テナント slug (KV key 名 = `tenant-{slug}`)
    pub tenant_slug: Option<String>,
    /// CF KV への自動登録結果
    pub kv_registered: bool,
    pub kv_message: Option<String>,
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

    // テナント slug を取得 (KV key 名に使う)
    let slug: Option<String> = sqlx::query_scalar("SELECT slug FROM tenants WHERE id = $1")
        .bind(tenant.0)
        .fetch_optional(&mut *tc.conn)
        .await
        .ok()
        .flatten();

    drop(tc);

    // CF KV に plaintext key を自動登録 (best-effort)
    let (kv_registered, kv_message) = if let Some(slug_str) = slug.as_deref() {
        register_kv(slug_str, &plain).await
    } else {
        (false, Some("テナント slug が見つかりません".to_string()))
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateIngestKeyResponse {
            id: row.0,
            name: input.name.trim().to_string(),
            key: plain,
            created_at: row.1,
            tenant_slug: slug,
            kv_registered,
            kv_message,
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

/// Cloudflare KV API へ `tenant-{slug}` = `<plaintext-key>` を PUT する。
/// 環境変数が無い場合や失敗時は (false, message) を返す (リクエスト全体は失敗させない)。
///
/// 必要な環境変数:
/// - `CLOUDFLARE_API_TOKEN` — Account:Workers KV Storage:Edit スコープ
/// - `CLOUDFLARE_ACCOUNT_ID`
/// - `NOTIFY_INGEST_KV_NAMESPACE_ID` — 受信側 Worker が bind している namespace ID
async fn register_kv(tenant_slug: &str, plaintext_key: &str) -> (bool, Option<String>) {
    let token = match std::env::var("CLOUDFLARE_API_TOKEN") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return (
                false,
                Some("CLOUDFLARE_API_TOKEN 未設定。手動で KV に登録してください。".to_string()),
            )
        }
    };
    let account_id = match std::env::var("CLOUDFLARE_ACCOUNT_ID") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return (
                false,
                Some("CLOUDFLARE_ACCOUNT_ID 未設定。手動で KV に登録してください。".to_string()),
            )
        }
    };
    let namespace_id = match std::env::var("NOTIFY_INGEST_KV_NAMESPACE_ID") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return (
                false,
                Some(
                    "NOTIFY_INGEST_KV_NAMESPACE_ID 未設定。手動で KV に登録してください。"
                        .to_string(),
                ),
            )
        }
    };

    let kv_key = format!("tenant-{}", tenant_slug);
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/storage/kv/namespaces/{}/values/{}",
        account_id, namespace_id, kv_key
    );

    let client = reqwest::Client::new();
    let res = match client
        .put(&url)
        .bearer_auth(&token)
        .header("Content-Type", "text/plain")
        .body(plaintext_key.to_string())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("CF KV register http error: {e}");
            return (false, Some(format!("KV API 接続エラー: {e}")));
        }
    };

    let status = res.status();
    if status.is_success() {
        tracing::info!("CF KV registered: tenant-{}", tenant_slug);
        (true, None)
    } else {
        let body = res.text().await.unwrap_or_default();
        tracing::warn!("CF KV register failed {}: {}", status, body);
        (false, Some(format!("KV API {} : {}", status, body)))
    }
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

    #[tokio::test]
    async fn register_kv_skips_when_env_missing() {
        // ENV 競合を避けるため、明示的に空にしてテスト
        let _l = ENV_LOCK.lock().await;
        std::env::remove_var("CLOUDFLARE_API_TOKEN");
        std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
        std::env::remove_var("NOTIFY_INGEST_KV_NAMESPACE_ID");
        let (ok, msg) = register_kv("acme", "plain").await;
        assert!(!ok);
        assert!(msg.unwrap().contains("CLOUDFLARE_API_TOKEN"));
    }

    #[tokio::test]
    async fn register_kv_skips_when_account_id_missing() {
        let _l = ENV_LOCK.lock().await;
        std::env::set_var("CLOUDFLARE_API_TOKEN", "tok");
        std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
        std::env::remove_var("NOTIFY_INGEST_KV_NAMESPACE_ID");
        let (ok, msg) = register_kv("acme", "plain").await;
        assert!(!ok);
        assert!(msg.unwrap().contains("CLOUDFLARE_ACCOUNT_ID"));
        std::env::remove_var("CLOUDFLARE_API_TOKEN");
    }

    #[tokio::test]
    async fn register_kv_skips_when_namespace_missing() {
        let _l = ENV_LOCK.lock().await;
        std::env::set_var("CLOUDFLARE_API_TOKEN", "tok");
        std::env::set_var("CLOUDFLARE_ACCOUNT_ID", "acc");
        std::env::remove_var("NOTIFY_INGEST_KV_NAMESPACE_ID");
        let (ok, msg) = register_kv("acme", "plain").await;
        assert!(!ok);
        assert!(msg.unwrap().contains("NOTIFY_INGEST_KV_NAMESPACE_ID"));
        std::env::remove_var("CLOUDFLARE_API_TOKEN");
        std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
    }

    use tokio::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::const_new(());
}
