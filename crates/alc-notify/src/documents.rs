use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Extension, Json, Router,
};
use uuid::Uuid;

use alc_core::auth_middleware::TenantId;
use alc_core::tenant::set_current_tenant;
use alc_core::AppState;

use crate::ingest::sanitize_filename;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/notify/documents", axum::routing::get(list))
        .route("/notify/documents/search", axum::routing::get(search))
        .route("/notify/documents/upload", axum::routing::post(upload))
        .route("/notify/documents/{id}", axum::routing::get(get))
}

#[derive(serde::Deserialize)]
struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let docs = state
        .notify_documents
        .list(tenant.0, q.limit.unwrap_or(50), q.offset.unwrap_or(0))
        .await
        .map_err(|e| {
            tracing::error!("list notify_documents: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::to_value(docs).unwrap()))
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Query(sq): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let docs = state
        .notify_documents
        .search(tenant.0, &sq.q)
        .await
        .map_err(|e| {
            tracing::error!("search notify_documents: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::to_value(docs).unwrap()))
}

async fn get(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let doc = state
        .notify_documents
        .get(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("get notify_document: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let deliveries = state
        .notify_deliveries
        .list_by_document(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("list deliveries for document: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({
        "document": doc,
        "deliveries": deliveries,
    })))
}

const MAX_UPLOAD_FILES: usize = 20;
const MAX_UPLOAD_TOTAL_BYTES: usize = 25 * 1024 * 1024;
const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "docx", "xlsx", "png", "jpg", "jpeg"];

#[derive(serde::Serialize)]
struct UploadResponse {
    document_ids: Vec<Uuid>,
    count: usize,
}

async fn upload(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResponse>), StatusCode> {
    let storage = state.notify_storage.as_ref().ok_or_else(|| {
        tracing::error!("notify_storage not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut files: Vec<(String, String, Vec<u8>)> = Vec::new();
    let mut total: usize = 0;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("multipart read: {e}");
        StatusCode::BAD_REQUEST
    })? {
        // file part 以外 (テキストフィールド等) は無視
        let Some(filename) = field.file_name().map(|s| s.to_string()) else {
            continue;
        };
        let content_type = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        if !is_allowed_extension(&filename) {
            return Err(StatusCode::BAD_REQUEST);
        }

        let bytes = field.bytes().await.map_err(|e| {
            tracing::warn!("multipart bytes: {e}");
            StatusCode::BAD_REQUEST
        })?;

        total = total.saturating_add(bytes.len());
        if total > MAX_UPLOAD_TOTAL_BYTES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        files.push((filename, content_type, bytes.to_vec()));
        if files.len() > MAX_UPLOAD_FILES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
    }

    if files.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let upload_batch_id = Uuid::new_v4();
    let mut keys_with_meta: Vec<(String, String, i64, String)> = Vec::with_capacity(files.len());
    for (filename, content_type, bytes) in &files {
        let key = format!(
            "{}/manual/{}/{}",
            tenant.0,
            upload_batch_id,
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

    let pool = state.pool();
    let mut conn = pool.acquire().await.map_err(|e| {
        tracing::error!("pool acquire: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    set_current_tenant(&mut conn, &tenant.0.to_string())
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
                tenant_id, source_type,
                r2_key, file_name, file_size_bytes,
                source_received_at,
                extraction_status, distribution_status
            )
            VALUES ($1, 'manual', $2, $3, $4, NOW(), 'pending', 'pending')
            RETURNING id
            "#,
        )
        .bind(tenant.0)
        .bind(r2_key)
        .bind(file_name)
        .bind(size)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| {
            tracing::error!("insert notify_document (manual): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        document_ids.push(id);
    }

    let count = document_ids.len();
    Ok((
        StatusCode::CREATED,
        Json(UploadResponse {
            document_ids,
            count,
        }),
    ))
}

fn is_allowed_extension(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    ALLOWED_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_extensions_basic() {
        assert!(is_allowed_extension("a.pdf"));
        assert!(is_allowed_extension("A.PDF"));
        assert!(is_allowed_extension("foo.docx"));
        assert!(is_allowed_extension("foo.bar.xlsx"));
        assert!(is_allowed_extension("photo.jpeg"));
        assert!(is_allowed_extension("photo.JPG"));
        assert!(is_allowed_extension("img.png"));
    }

    #[test]
    fn rejected_extensions() {
        assert!(!is_allowed_extension("a.exe"));
        assert!(!is_allowed_extension("a"));
        assert!(!is_allowed_extension(""));
        assert!(!is_allowed_extension(".pdf.exe"));
        assert!(!is_allowed_extension("pdf"));
    }

    #[test]
    fn upload_limits_constants() {
        assert_eq!(MAX_UPLOAD_FILES, 20);
        assert_eq!(MAX_UPLOAD_TOTAL_BYTES, 25 * 1024 * 1024);
    }
}
