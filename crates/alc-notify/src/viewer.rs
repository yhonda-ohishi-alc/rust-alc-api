//! 公開 viewer (ログイン不要) — nuxt-notify の `/v/{token}` ページから呼ばれる。
//!
//! 既読化はしない (それは `/api/notify/read/{token}` の責務)。
//! トークンが有効である限り何度でも閲覧できる。
//!
//! - GET /api/notify/v/{token}      → メタデータ JSON (件名 / 送信者 / ファイル名 / 受信日時 / 期限)
//! - GET /api/notify/v/{token}/file → R2 presigned URL (inline) に 302 redirect

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use uuid::Uuid;

use alc_core::repository::notify_deliveries::DeliveryViewInfo;
use alc_core::AppState;

/// presigned URL の最大有効期間 (秒)。delivery.expire_at までの残期間と
/// この値の小さい方を採用する。
pub(crate) const PRESIGN_DEFAULT_SECS: i64 = 3600;
/// presigned URL の最低秒数 (R2/S3 の最小値に対する安全マージン)
pub(crate) const PRESIGN_MIN_SECS: i64 = 60;

pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/notify/v/{token}", axum::routing::get(view_metadata))
        .route("/notify/v/{token}/file", axum::routing::get(view_file))
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct ViewMetadata {
    pub file_name: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub source_subject: Option<String>,
    pub source_sender: Option<String>,
    pub source_received_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expire_at: chrono::DateTime<chrono::Utc>,
}

pub(crate) fn build_metadata(info: &DeliveryViewInfo) -> ViewMetadata {
    ViewMetadata {
        file_name: info.file_name.clone(),
        file_size_bytes: info.file_size_bytes,
        source_subject: info.source_subject.clone(),
        source_sender: info.source_sender.clone(),
        source_received_at: info.source_received_at,
        expire_at: info.expire_at,
    }
}

/// 期限を判定する純関数。期限切れなら 410 Gone、有効なら使う秒数を返す。
pub(crate) fn presign_secs_or_gone(
    expire_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<u32, StatusCode> {
    if expire_at <= now {
        return Err(StatusCode::GONE);
    }
    let remaining = (expire_at - now).num_seconds();
    Ok(remaining.clamp(PRESIGN_MIN_SECS, PRESIGN_DEFAULT_SECS) as u32)
}

async fn view_metadata(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
) -> Result<Json<ViewMetadata>, StatusCode> {
    let info = state
        .notify_deliveries
        .get_for_view(token)
        .await
        .map_err(|e| {
            tracing::error!("get_for_view: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    presign_secs_or_gone(info.expire_at, chrono::Utc::now())?;
    Ok(Json(build_metadata(&info)))
}

async fn view_file(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let info = state
        .notify_deliveries
        .get_for_view(token)
        .await
        .map_err(|e| {
            tracing::error!("get_for_view: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let secs = presign_secs_or_gone(info.expire_at, chrono::Utc::now())?;

    let storage = state.notify_storage.as_ref().ok_or_else(|| {
        tracing::error!("notify_storage not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let url = storage.presign_get(&info.r2_key, secs).await.map_err(|e| {
        tracing::error!("presign_get: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::FOUND, [(header::LOCATION, url)]).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info(expire_in_hours: i64) -> DeliveryViewInfo {
        DeliveryViewInfo {
            document_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            r2_key: "tenant/email/msg/file.pdf".into(),
            file_name: Some("file.pdf".into()),
            file_size_bytes: Some(2048),
            source_subject: Some("件名".into()),
            source_sender: Some("from@example.com".into()),
            source_received_at: Some(chrono::Utc::now()),
            expire_at: chrono::Utc::now() + chrono::Duration::hours(expire_in_hours),
        }
    }

    #[test]
    fn build_metadata_copies_all_fields() {
        let info = sample_info(24);
        let m = build_metadata(&info);
        assert_eq!(m.file_name, info.file_name);
        assert_eq!(m.file_size_bytes, info.file_size_bytes);
        assert_eq!(m.source_subject, info.source_subject);
        assert_eq!(m.source_sender, info.source_sender);
        assert_eq!(m.source_received_at, info.source_received_at);
        assert_eq!(m.expire_at, info.expire_at);
        // r2_key と document_id / tenant_id は外に漏らさない
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains("r2_key"));
        assert!(!json.contains("document_id"));
        assert!(!json.contains("tenant_id"));
    }

    #[test]
    fn presign_secs_clamps_to_default_when_remaining_is_long() {
        let now = chrono::Utc::now();
        let expire = now + chrono::Duration::days(7);
        let secs = presign_secs_or_gone(expire, now).unwrap();
        assert_eq!(secs as i64, PRESIGN_DEFAULT_SECS);
    }

    #[test]
    fn presign_secs_clamps_to_min_when_remaining_is_short() {
        let now = chrono::Utc::now();
        let expire = now + chrono::Duration::seconds(10);
        let secs = presign_secs_or_gone(expire, now).unwrap();
        assert_eq!(secs as i64, PRESIGN_MIN_SECS);
    }

    #[test]
    fn presign_secs_uses_remaining_in_window() {
        let now = chrono::Utc::now();
        let expire = now + chrono::Duration::seconds(900);
        let secs = presign_secs_or_gone(expire, now).unwrap();
        assert_eq!(secs, 900);
    }

    #[test]
    fn presign_secs_returns_gone_when_expired() {
        let now = chrono::Utc::now();
        let expire = now - chrono::Duration::seconds(1);
        let err = presign_secs_or_gone(expire, now).unwrap_err();
        assert_eq!(err, StatusCode::GONE);
    }

    #[test]
    fn presign_secs_returns_gone_at_exact_boundary() {
        let now = chrono::Utc::now();
        let err = presign_secs_or_gone(now, now).unwrap_err();
        assert_eq!(err, StatusCode::GONE);
    }
}
