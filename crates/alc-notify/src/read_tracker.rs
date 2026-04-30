//! 既読トラッキング + 公開ファイル配信
//!
//! メッセージ内リンク `/api/notify/read/{token}` がクリックされると:
//! 1. mark_delivery_read で既読更新 + r2_key + expire_at を取得
//! 2. expire_at 経過後なら 410 Gone (リンク失効)
//! 3. 期限内なら R2 の presigned URL (短期、最大 1 時間) を発行 → 302 redirect
//!
//! ログイン不要で LINE WORKS in-app browser から見られる (Google OAuth ブロック回避)。

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use uuid::Uuid;

use alc_core::AppState;

/// presigned URL の有効期間 (秒)。delivery.expire_at までの残期間と
/// この値の小さい方を採用する。LINE トークから複数回タップしても
/// 都度新規発行されるので、単一 URL の漏洩耐性を上げる目的。
const PRESIGN_DEFAULT_SECS: i64 = 3600;
/// presigned URL の最低秒数 (R2/S3 の最小値に対する安全マージン)
const PRESIGN_MIN_SECS: i64 = 60;

pub fn public_router() -> Router<AppState> {
    Router::new().route("/notify/read/{token}", axum::routing::get(read_redirect))
}

async fn read_redirect(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let result = state
        .notify_deliveries
        .mark_read(token)
        .await
        .map_err(|e| {
            tracing::error!("mark_read: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let r = result.ok_or(StatusCode::NOT_FOUND)?;

    // 配信単位の絶対期限 (= notify_deliveries.expire_at) を超えたら 410
    let now = chrono::Utc::now();
    if r.expire_at <= now {
        return Err(StatusCode::GONE);
    }

    // 残期間と PRESIGN_DEFAULT_SECS の小さい方を採用 (PRESIGN_MIN_SECS で下限ガード)
    let remaining = (r.expire_at - now).num_seconds();
    let presign_secs = remaining.clamp(PRESIGN_MIN_SECS, PRESIGN_DEFAULT_SECS) as u32;

    let storage = state.notify_storage.as_ref().ok_or_else(|| {
        tracing::error!("notify_storage not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let presigned_url = storage
        .presign_get(&r.r2_key, presign_secs)
        .await
        .map_err(|e| {
            tracing::error!("presign_get: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::FOUND, [(header::LOCATION, presigned_url)]).into_response())
}
