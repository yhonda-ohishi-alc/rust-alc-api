use axum::{
    extract::Multipart,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Serialize;

use crate::db::DbPool;
use crate::middleware::auth::TenantId;

pub fn router() -> Router<DbPool> {
    Router::new().route("/upload/face-photo", post(upload_face_photo))
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub url: String,
    pub filename: String,
}

async fn upload_face_photo(
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

    // GCP Cloud Storage upload
    let bucket = std::env::var("GCS_BUCKET").unwrap_or_else(|_| "alc-face-photos".into());
    let object_path = format!("{tenant_id}/{filename}");

    // Use GCP metadata server token for authentication in Cloud Run
    let token = get_gcp_access_token()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let upload_url = format!(
        "https://storage.googleapis.com/upload/storage/v1/b/{bucket}/o?uploadType=media&name={object_path}"
    );

    let client = reqwest::Client::new();
    client
        .post(&upload_url)
        .bearer_auth(&token)
        .header("Content-Type", "image/jpeg")
        .body(data.to_vec())
        .send()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .error_for_status()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let public_url = format!("https://storage.googleapis.com/{bucket}/{object_path}");

    Ok(Json(UploadResponse {
        url: public_url,
        filename,
    }))
}

async fn get_gcp_access_token() -> anyhow::Result<String> {
    // In Cloud Run, use the metadata server
    let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(url)
        .header("Metadata-Flavor", "Google")
        .send()
        .await?
        .json()
        .await?;

    resp["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no access_token in metadata response"))
}
