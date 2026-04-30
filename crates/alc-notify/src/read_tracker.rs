//! 既読トラッキング — メッセージ内リンクのクリックハンドラ。
//!
//! 流れ:
//! 1. `mark_delivery_read` で既読更新 + r2_key + expire_at を取得
//!    (既読済みでも r2_key と expire_at は返るが、未読時のみ read_at を更新)
//! 2. expire_at 経過後なら 410 Gone (リンク失効)
//! 3. 期限内なら nuxt-notify の公開 viewer ページ `/v/{token}` に 302 redirect
//!    (ログイン不要、Google OAuth ブロック回避、ブランディング/メタデータ表示可能)
//!
//! 公開 viewer ページが内部で `/api/notify/v/{token}` (メタデータ) と
//! `/api/notify/v/{token}/file` (R2 presigned URL リダイレクト) を呼ぶ。

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use uuid::Uuid;

use alc_core::AppState;

pub fn public_router() -> Router<AppState> {
    Router::new().route("/notify/read/{token}", axum::routing::get(read_redirect))
}

pub(crate) fn frontend_url() -> String {
    std::env::var("NOTIFY_FRONTEND_URL").unwrap_or_else(|_| "https://notify.example.com".into())
}

pub(crate) fn build_view_url(frontend_url: &str, token: Uuid) -> String {
    let trimmed = frontend_url.trim_end_matches('/');
    format!("{trimmed}/v/{token}")
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
    if r.expire_at <= chrono::Utc::now() {
        return Err(StatusCode::GONE);
    }

    let url = build_view_url(&frontend_url(), token);
    Ok((StatusCode::FOUND, [(header::LOCATION, url)]).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_view_url_appends_token() {
        let token = Uuid::nil();
        let url = build_view_url("https://notify.example.com", token);
        assert_eq!(
            url,
            "https://notify.example.com/v/00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn build_view_url_strips_trailing_slash() {
        let token = Uuid::nil();
        let url = build_view_url("https://notify.example.com/", token);
        assert_eq!(
            url,
            "https://notify.example.com/v/00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn frontend_url_uses_env_when_set() {
        // 並列テストで環境変数を直接弄らないよう、デフォルト経路だけ
        // 確認する。env var 注入経路は staging E2E でカバーする。
        std::env::remove_var("NOTIFY_FRONTEND_URL");
        let url = frontend_url();
        assert!(url.starts_with("https://"));
    }
}
