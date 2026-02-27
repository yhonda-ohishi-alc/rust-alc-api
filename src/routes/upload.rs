use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Serialize;

use crate::AppState;
use crate::middleware::auth::TenantId;

pub fn router() -> Router<AppState> {
    Router::new().route("/upload/face-photo", post(upload_face_photo))
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub url: String,
    pub filename: String,
}

async fn upload_face_photo(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}.jpg", uuid::Uuid::new_v4()));

    let data = field
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let object_path = format!("{tenant_id}/{filename}");

    let url = state
        .storage
        .upload(&object_path, &data, "image/jpeg")
        .await
        .map_err(|e| {
            tracing::error!("Storage upload failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(UploadResponse { url, filename }))
}
