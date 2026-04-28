//! 受信メール一覧 / 添付ファイルダウンロード / 削除エンドポイント (テナント保護)
//!
//! `notify_documents` のうち `email_message_id` でグルーピングされた行を「メール」として扱う。
//! 1メール = N添付 (N行) なので、一覧は GROUP BY、詳細は同 message_id の行を返す。

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json, Router,
};
use uuid::Uuid;

use alc_core::auth_middleware::TenantId;
use alc_core::tenant::TenantConn;
use alc_core::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/notify/emails", axum::routing::get(list_emails))
        .route("/notify/emails/{message_id}", axum::routing::get(get_email))
        .route(
            "/notify/documents/{id}/download",
            axum::routing::get(download_document),
        )
        .route(
            "/notify/documents/{id}",
            axum::routing::delete(delete_document),
        )
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct EmailSummary {
    pub email_message_id: Uuid,
    pub source_sender: Option<String>,
    pub source_subject: Option<String>,
    pub source_received_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attachment_count: i64,
    pub total_size_bytes: Option<i64>,
    pub distribution_status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct EmailDocument {
    pub id: Uuid,
    pub file_name: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub r2_key: String,
    pub extraction_status: String,
    pub distribution_status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
pub struct EmailDetail {
    pub email_message_id: Uuid,
    pub source_sender: Option<String>,
    pub source_subject: Option<String>,
    pub source_body_text: Option<String>,
    pub source_received_at: Option<chrono::DateTime<chrono::Utc>>,
    pub documents: Vec<EmailDocument>,
}

async fn list_emails(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
) -> Result<Json<Vec<EmailSummary>>, StatusCode> {
    let mut tc = TenantConn::acquire(state.pool(), &tenant.0.to_string())
        .await
        .map_err(|e| {
            tracing::error!("acquire tenant conn: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let rows: Vec<EmailSummary> = sqlx::query_as(
        r#"
        SELECT
            email_message_id,
            MAX(source_sender) AS source_sender,
            MAX(source_subject) AS source_subject,
            MAX(source_received_at) AS source_received_at,
            COUNT(*)::BIGINT AS attachment_count,
            SUM(file_size_bytes)::BIGINT AS total_size_bytes,
            CASE
                WHEN BOOL_AND(distribution_status = 'completed') THEN 'completed'
                WHEN BOOL_OR(distribution_status = 'in_progress') THEN 'in_progress'
                WHEN BOOL_OR(distribution_status = 'failed') THEN 'failed'
                ELSE 'pending'
            END AS distribution_status,
            MAX(created_at) AS created_at
        FROM notify_documents
        WHERE email_message_id IS NOT NULL
        GROUP BY email_message_id
        ORDER BY MAX(created_at) DESC
        LIMIT 200
        "#,
    )
    .fetch_all(&mut *tc.conn)
    .await
    .map_err(|e| {
        tracing::error!("list emails: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rows))
}

async fn get_email(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(message_id): Path<Uuid>,
) -> Result<Json<EmailDetail>, StatusCode> {
    let mut tc = TenantConn::acquire(state.pool(), &tenant.0.to_string())
        .await
        .map_err(|e| {
            tracing::error!("acquire tenant conn: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    #[derive(sqlx::FromRow)]
    struct HeaderRow {
        source_sender: Option<String>,
        source_subject: Option<String>,
        source_body_text: Option<String>,
        source_received_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let header_row: Option<HeaderRow> = sqlx::query_as(
        r#"
        SELECT source_sender, source_subject, source_body_text, source_received_at
        FROM notify_documents
        WHERE email_message_id = $1
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .bind(message_id)
    .fetch_optional(&mut *tc.conn)
    .await
    .map_err(|e| {
        tracing::error!("get email header: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let header_row = header_row.ok_or(StatusCode::NOT_FOUND)?;

    let documents: Vec<EmailDocument> = sqlx::query_as(
        r#"
        SELECT id, file_name, file_size_bytes, r2_key,
               extraction_status, distribution_status, created_at
        FROM notify_documents
        WHERE email_message_id = $1
        ORDER BY created_at ASC, file_name ASC
        "#,
    )
    .bind(message_id)
    .fetch_all(&mut *tc.conn)
    .await
    .map_err(|e| {
        tracing::error!("get email documents: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(EmailDetail {
        email_message_id: message_id,
        source_sender: header_row.source_sender,
        source_subject: header_row.source_subject,
        source_body_text: header_row.source_body_text,
        source_received_at: header_row.source_received_at,
        documents,
    }))
}

async fn download_document(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<axum::response::Response, StatusCode> {
    let mut tc = TenantConn::acquire(state.pool(), &tenant.0.to_string())
        .await
        .map_err(|e| {
            tracing::error!("acquire tenant conn: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT r2_key, file_name FROM notify_documents WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tc.conn)
            .await
            .map_err(|e| {
                tracing::error!("lookup document: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let (r2_key, file_name) = row.ok_or(StatusCode::NOT_FOUND)?;
    drop(tc);

    let storage = state.notify_storage.as_ref().ok_or_else(|| {
        tracing::error!("notify_storage not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let bytes = storage.download(&r2_key).await.map_err(|e| {
        tracing::error!("notify_storage.download: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let display_name = file_name.unwrap_or_else(|| "attachment".to_string());
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    // RFC 5987 形式の filename* で UTF-8 ファイル名を安全にエンコード
    let encoded = urlencoding::encode(&display_name);
    let cd = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        display_name.replace('"', "_"),
        encoded
    );
    if let Ok(v) = cd.parse() {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }

    Ok((StatusCode::OK, headers, bytes).into_response())
}

async fn delete_document(
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

    // 先に r2_key を取得
    let row: Option<(String,)> =
        sqlx::query_as("SELECT r2_key FROM notify_documents WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tc.conn)
            .await
            .map_err(|e| {
                tracing::error!("lookup document: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let r2_key = row.ok_or(StatusCode::NOT_FOUND)?.0;

    // DB から削除 (deliveries は ON DELETE CASCADE)
    sqlx::query("DELETE FROM notify_documents WHERE id = $1")
        .bind(id)
        .execute(&mut *tc.conn)
        .await
        .map_err(|e| {
            tracing::error!("delete document: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    drop(tc);

    // R2 からも削除 (best-effort)
    if let Some(storage) = state.notify_storage.as_ref() {
        if let Err(e) = storage.delete(&r2_key).await {
            tracing::warn!("notify_storage.delete (orphan ok): {e}");
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
