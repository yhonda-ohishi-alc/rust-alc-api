//! LINE WORKS Bot のチャネル/グループ管理。
//!
//! Bot 公式 API には「既存トークルームに Bot を追加する」エンドポイントが無いため、
//! ユーザーが LINE WORKS アプリ上で手動で Bot を招待 → join webhook で channel_id を保存
//! という運用にしている。本モジュールはその webhook と、登録済み channel の CRUD を担う。

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use alc_core::auth_lineworks::decrypt_secret;
use alc_core::auth_middleware::TenantId;
use alc_core::repository::lineworks_channels::LineworksChannel;
use alc_core::AppState;

use crate::clients::lineworks::{LineworksBotClient, LineworksBotConfig};

type HmacSha256 = Hmac<Sha256>;

/// Admin (require_tenant) ルート群。
pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/notify/lineworks/channels", get(list_channels))
        .route("/notify/lineworks/channels/{id}", delete(delete_channel))
        .route(
            "/notify/lineworks/channels/{id}/test-send",
            post(test_send_channel),
        )
}

/// Public (認証なし) ルート群。LINE WORKS Developers Console から callback URL として登録される。
///
/// **段階移行中**: 新しい callback は auth-worker 側 (`https://auth.ippoan.org/lineworks/webhook/{bot_id}`)
/// で受け、HMAC 検証 + 復号 + イベント抽出を行ってから [`internal_router`] にイベントだけを転送する。
/// 旧 callback URL を切替えるまでは両方が生きている。
pub fn public_router() -> Router<AppState> {
    Router::new().route(
        "/notify/lineworks/webhook/{bot_id}",
        post(handle_webhook_route),
    )
}

/// Internal (auth-worker 専用) ルート群。`require_internal_jwt` 配下に nest される想定。
///
/// auth-worker (Cloudflare Workers) が LINE WORKS webhook を edge で受け、
/// HMAC 検証 + 復号 + イベント抽出を済ませた後、本ルートに転送する。
///
/// - `GET  /api/internal/lineworks/bot-secret/{bot_id}` — bot_secret_encrypted を返す (復号は auth-worker)
/// - `POST /api/internal/lineworks/event` — 検証済みイベントを受け取って upsert/mark_left
pub fn internal_router() -> Router<AppState> {
    Router::new()
        .route(
            "/internal/lineworks/bot-secret/{bot_id}",
            get(get_bot_secret_internal),
        )
        .route("/internal/lineworks/event", post(receive_event_internal))
}

fn internal_error(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": "internal_error", "message": msg})),
    )
}

fn encryption_key() -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    std::env::var("SSO_ENCRYPTION_KEY")
        .or_else(|_| std::env::var("JWT_SECRET"))
        .map_err(|_| {
            tracing::error!("SSO_ENCRYPTION_KEY or JWT_SECRET not set");
            internal_error("encryption_key_missing")
        })
}

// ---------- admin: list ----------

async fn list_channels(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
) -> Result<Json<Vec<LineworksChannel>>, (StatusCode, Json<serde_json::Value>)> {
    let rows = state
        .lineworks_channels
        .list_active(tenant.0)
        .await
        .map_err(|e| {
            tracing::error!("list_active lineworks_channels: {e}");
            internal_error("list_failed")
        })?;
    Ok(Json(rows))
}

// ---------- admin: delete ----------

async fn delete_channel(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    state
        .lineworks_channels
        .delete(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("delete lineworks_channel: {e}");
            internal_error("delete_failed")
        })?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- admin: test-send ----------

#[derive(Debug, Deserialize)]
pub struct TestSendBody {
    pub text: String,
}

async fn test_send_channel(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<TestSendBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let row = state
        .lineworks_channels
        .get(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("get lineworks_channel: {e}");
            internal_error("get_failed")
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "channel_not_found"})),
            )
        })?;

    let full = state
        .bot_admin
        .get_config_with_secrets(tenant.0, row.bot_config_id)
        .await
        .map_err(|e| {
            tracing::error!("get_config_with_secrets: {e}");
            internal_error("get_bot_config_failed")
        })?
        .ok_or_else(|| internal_error("bot_config_not_found"))?;

    let key = encryption_key()?;
    let client_secret = decrypt_secret(&full.client_secret_encrypted, &key).map_err(|e| {
        tracing::error!("decrypt client_secret: {e}");
        internal_error("decrypt_failed")
    })?;
    let private_key =
        alc_core::auth_lineworks::decrypt_pem_secret(&full.private_key_encrypted, &key).map_err(
            |e| {
                tracing::error!("decrypt private_key: {e}");
                internal_error("decrypt_failed")
            },
        )?;

    let config = LineworksBotConfig {
        client_id: full.client_id.clone(),
        client_secret,
        service_account: full.service_account.clone(),
        private_key,
        bot_id: full.bot_id.clone(),
    };

    let client = LineworksBotClient::new();
    client
        .send_text_to_channel(full.id, &config, &row.channel_id, &body.text)
        .await
        .map_err(|e| {
            tracing::error!("send_text_to_channel: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": "upstream_error", "message": e.to_string()})),
            )
        })?;

    Ok(Json(serde_json::json!({"ok": true})))
}

// ---------- webhook (public) ----------

#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub source: Option<WebhookSource>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookSource {
    #[serde(rename = "channelId")]
    pub channel_id: Option<String>,
    #[serde(rename = "channelType")]
    pub channel_type: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub ok: bool,
}

async fn handle_webhook_route(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<serde_json::Value>)> {
    handle_webhook(&state, &bot_id, &headers, &body).await
}

/// Public testable core. handler は HeaderMap/Body をこの関数に委譲する。
pub async fn handle_webhook(
    state: &AppState,
    bot_id: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Json<WebhookResponse>, (StatusCode, Json<serde_json::Value>)> {
    let cfg = state
        .lineworks_channels
        .lookup_bot_config_for_webhook(bot_id)
        .await
        .map_err(|e| {
            tracing::error!("lookup_bot_config_for_webhook: {e}");
            internal_error("lookup_failed")
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "bot_not_found"})),
        ))?;

    let bot_secret_encrypted = cfg.bot_secret_encrypted.ok_or((
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "bot_secret_not_configured"})),
    ))?;

    let key = encryption_key()?;
    let bot_secret = decrypt_secret(&bot_secret_encrypted, &key).map_err(|e| {
        tracing::error!("decrypt bot_secret: {e}");
        internal_error("decrypt_failed")
    })?;

    // X-WORKS-Signature: base64(HMAC-SHA256(bot_secret, raw_body))
    let signature_b64 = headers
        .get("x-works-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "missing_signature"})),
        ))?;

    if !verify_signature(&bot_secret, body, signature_b64) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "signature_mismatch"})),
        ));
    }

    let event: WebhookEvent = serde_json::from_slice(body).map_err(|e| {
        tracing::error!("parse webhook event: {e}");
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_payload"})),
        )
    })?;

    let source = event.source.unwrap_or(WebhookSource {
        channel_id: None,
        channel_type: None,
        title: None,
    });
    let channel_id = match source.channel_id {
        Some(c) => c,
        // Source 無しイベント (e.g. bot メッセージ) は無視
        None => return Ok(Json(WebhookResponse { ok: true })),
    };

    match event.event_type.as_str() {
        "join" | "joined" => {
            state
                .lineworks_channels
                .upsert_joined(
                    cfg.tenant_id,
                    cfg.id,
                    &channel_id,
                    source.channel_type.as_deref(),
                    source.title.as_deref(),
                )
                .await
                .map_err(|e| {
                    tracing::error!("upsert_joined: {e}");
                    internal_error("upsert_failed")
                })?;
        }
        "leave" | "left" => {
            state
                .lineworks_channels
                .mark_left(cfg.tenant_id, cfg.id, &channel_id)
                .await
                .map_err(|e| {
                    tracing::error!("mark_left: {e}");
                    internal_error("mark_left_failed")
                })?;
        }
        // message 等のその他イベントは無視 (200 OK)
        _ => {}
    }

    Ok(Json(WebhookResponse { ok: true }))
}

// ---------- internal: GET bot-secret ----------

#[derive(Debug, Serialize)]
pub struct BotSecretEncryptedResponse {
    pub bot_secret_encrypted: String,
}

async fn get_bot_secret_internal(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<Json<BotSecretEncryptedResponse>, (StatusCode, Json<serde_json::Value>)> {
    let cfg = state
        .lineworks_channels
        .lookup_bot_config_for_webhook(&bot_id)
        .await
        .map_err(|e| {
            tracing::error!("lookup_bot_config_for_webhook (internal): {e}");
            internal_error("lookup_failed")
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "bot_not_found"})),
        ))?;

    let bot_secret_encrypted = cfg.bot_secret_encrypted.ok_or((
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "bot_secret_not_configured"})),
    ))?;

    Ok(Json(BotSecretEncryptedResponse {
        bot_secret_encrypted,
    }))
}

// ---------- internal: POST event ----------

#[derive(Debug, Deserialize)]
pub struct InternalEventBody {
    pub bot_id: String,
    pub event_type: String,
    pub channel_id: Option<String>,
    pub channel_type: Option<String>,
    pub title: Option<String>,
}

async fn receive_event_internal(
    State(state): State<AppState>,
    Json(body): Json<InternalEventBody>,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<serde_json::Value>)> {
    process_internal_event(&state, body).await
}

/// Public testable core。`receive_event_internal` から委譲される。
pub async fn process_internal_event(
    state: &AppState,
    body: InternalEventBody,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<serde_json::Value>)> {
    let cfg = state
        .lineworks_channels
        .lookup_bot_config_for_webhook(&body.bot_id)
        .await
        .map_err(|e| {
            tracing::error!("lookup_bot_config_for_webhook (event): {e}");
            internal_error("lookup_failed")
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "bot_not_found"})),
        ))?;

    let channel_id = match body.channel_id {
        Some(c) => c,
        None => return Ok(Json(WebhookResponse { ok: true })),
    };

    match body.event_type.as_str() {
        "join" | "joined" => {
            state
                .lineworks_channels
                .upsert_joined(
                    cfg.tenant_id,
                    cfg.id,
                    &channel_id,
                    body.channel_type.as_deref(),
                    body.title.as_deref(),
                )
                .await
                .map_err(|e| {
                    tracing::error!("upsert_joined (internal): {e}");
                    internal_error("upsert_failed")
                })?;
        }
        "leave" | "left" => {
            state
                .lineworks_channels
                .mark_left(cfg.tenant_id, cfg.id, &channel_id)
                .await
                .map_err(|e| {
                    tracing::error!("mark_left (internal): {e}");
                    internal_error("mark_left_failed")
                })?;
        }
        _ => {}
    }

    Ok(Json(WebhookResponse { ok: true }))
}

/// HMAC-SHA256(bot_secret, body) の base64 と signature_b64 が一致するか定数時間比較。
pub(crate) fn verify_signature(bot_secret: &str, body: &[u8], signature_b64: &str) -> bool {
    let mut mac = match HmacSha256::new_from_slice(bot_secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    let provided = match BASE64.decode(signature_b64) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if provided.len() != expected.len() {
        return false;
    }
    // 定数時間比較
    let mut diff: u8 = 0;
    for (a, b) in expected.iter().zip(provided.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &str, body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        BASE64.encode(mac.finalize().into_bytes())
    }

    #[test]
    fn verify_signature_accepts_matching_hmac() {
        let secret = "topsecret";
        let body = br#"{"hello":"world"}"#;
        let sig = sign(secret, body);
        assert!(verify_signature(secret, body, &sig));
    }

    #[test]
    fn verify_signature_rejects_wrong_secret() {
        let body = br#"{"hello":"world"}"#;
        let sig = sign("right", body);
        assert!(!verify_signature("wrong", body, &sig));
    }

    #[test]
    fn verify_signature_rejects_tampered_body() {
        let secret = "topsecret";
        let original = br#"{"hello":"world"}"#;
        let tampered = br#"{"hello":"WORLD"}"#;
        let sig = sign(secret, original);
        assert!(!verify_signature(secret, tampered, &sig));
    }

    #[test]
    fn verify_signature_rejects_invalid_base64() {
        let secret = "topsecret";
        let body = b"x";
        assert!(!verify_signature(secret, body, "!!!not-base64!!!"));
    }

    #[test]
    fn verify_signature_rejects_wrong_length_signature() {
        let secret = "topsecret";
        let body = b"x";
        // valid base64 but decoded to 3 bytes (HMAC is 32)
        assert!(!verify_signature(secret, body, "AAAA"));
    }

    #[test]
    fn webhook_event_parses_join_with_source() {
        let json =
            r#"{"type":"joined","source":{"channelId":"ch1","channelType":"group","title":"T"}}"#;
        let ev: WebhookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.event_type, "joined");
        let s = ev.source.unwrap();
        assert_eq!(s.channel_id.as_deref(), Some("ch1"));
        assert_eq!(s.channel_type.as_deref(), Some("group"));
        assert_eq!(s.title.as_deref(), Some("T"));
    }

    #[test]
    fn webhook_event_accepts_missing_source() {
        let json = r#"{"type":"message"}"#;
        let ev: WebhookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.event_type, "message");
        assert!(ev.source.is_none());
    }
}
