//! Email Worker からの ingest エンドポイント (X-Ingest-Key 認証)
//!
//! Cloudflare Email Worker が受信したメールを JSON で POST してくる。
//! 添付ファイルは base64 で送られ、各ファイルは notify_documents の 1 行に分解される。
//! 同一メールに含まれる添付は同じ email_message_id でグルーピングされる。

use axum::{extract::State, http::StatusCode, Json, Router};
use base64::Engine;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use alc_core::tenant::set_current_tenant;
use alc_core::AppState;

pub fn public_router() -> Router<AppState> {
    Router::new().route("/notify/ingest", axum::routing::post(handle_ingest))
}

#[derive(serde::Deserialize)]
pub struct IngestRequest {
    pub from: Option<String>,
    pub subject: Option<String>,
    pub body_text: Option<String>,
    #[allow(dead_code)]
    pub body_html: Option<String>,
    pub received_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(serde::Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    #[allow(dead_code)]
    pub size: Option<i64>,
    pub content_base64: String,
}

#[derive(serde::Serialize)]
pub struct IngestResponse {
    pub email_message_id: Uuid,
    pub document_ids: Vec<Uuid>,
    pub count: usize,
}

const MAX_ATTACHMENTS: usize = 20;
const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;

async fn handle_ingest(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<IngestRequest>,
) -> Result<(StatusCode, Json<IngestResponse>), StatusCode> {
    // 1. ingest key 検証 → tenant_id 取得
    let raw_key = headers
        .get("x-ingest-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let key_hash = hex_sha256(raw_key);

    let pool = state.pool();
    let tenant_id: Option<Uuid> = sqlx::query_scalar("SELECT alc_api.verify_ingest_key($1)")
        .bind(&key_hash)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("verify_ingest_key: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let tenant_id = tenant_id.ok_or(StatusCode::UNAUTHORIZED)?;

    // 2. 受信制約
    if payload.attachments.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if payload.attachments.len() > MAX_ATTACHMENTS {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // 3. 添付を base64 デコード + サイズ計算
    let storage = state.notify_storage.as_ref().ok_or_else(|| {
        tracing::error!("notify_storage not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let email_message_id = Uuid::new_v4();
    let received_at = payload.received_at.unwrap_or_else(chrono::Utc::now);

    let mut decoded: Vec<(String, String, Vec<u8>)> = Vec::with_capacity(payload.attachments.len());
    let mut total: usize = 0;
    for a in &payload.attachments {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&a.content_base64)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        total = total.saturating_add(bytes.len());
        if total > MAX_TOTAL_BYTES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        decoded.push((a.filename.clone(), a.content_type.clone(), bytes));
    }

    // 4. R2 に保存
    let mut keys_with_meta: Vec<(String, String, i64, String)> = Vec::with_capacity(decoded.len());
    for (filename, content_type, bytes) in &decoded {
        let key = format!(
            "{}/email/{}/{}",
            tenant_id,
            email_message_id,
            sanitize_filename(filename)
        );
        storage
            .upload(&key, bytes, content_type)
            .await
            .map_err(|e| {
                tracing::error!("notify_storage.upload: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        keys_with_meta.push((
            key,
            filename.clone(),
            bytes.len() as i64,
            content_type.clone(),
        ));
    }

    // 5. notify_documents に INSERT (RLS コンテキスト設定後)
    let mut conn = pool.acquire().await.map_err(|e| {
        tracing::error!("pool acquire: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    set_current_tenant(&mut conn, &tenant_id.to_string())
        .await
        .map_err(|e| {
            tracing::error!("set_current_tenant: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut document_ids: Vec<Uuid> = Vec::with_capacity(keys_with_meta.len());
    for (r2_key, file_name, size, _ct) in &keys_with_meta {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO notify_documents (
                tenant_id, source_type, source_sender, source_subject,
                r2_key, file_name, file_size_bytes,
                email_message_id, source_body_text, source_received_at,
                extraction_status, distribution_status
            )
            VALUES ($1, 'email', $2, $3, $4, $5, $6, $7, $8, $9, 'pending', 'pending')
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(payload.from.as_deref())
        .bind(payload.subject.as_deref())
        .bind(r2_key)
        .bind(file_name)
        .bind(size)
        .bind(email_message_id)
        .bind(payload.body_text.as_deref())
        .bind(received_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| {
            tracing::error!("insert notify_document: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        document_ids.push(id);
    }

    let count = document_ids.len();
    Ok((
        StatusCode::CREATED,
        Json(IngestResponse {
            email_message_id,
            document_ids,
            count,
        }),
    ))
}

fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let bytes = hasher.finalize();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 危険なパス文字を除去 (/, .. を含むファイル名で R2 にぶら下げない)
fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "attachment".to_string();
    }
    let cleaned: String = trimmed
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if cleaned.contains("..") {
        cleaned.replace("..", "_")
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_sha256_known_vector() {
        assert_eq!(
            hex_sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sanitize_filename_strips_slashes() {
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_filename("../etc/passwd"), "__etc_passwd");
        assert_eq!(sanitize_filename("normal.pdf"), "normal.pdf");
        assert_eq!(sanitize_filename(""), "attachment");
        assert_eq!(sanitize_filename("   "), "attachment");
    }
}
