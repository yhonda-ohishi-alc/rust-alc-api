use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use alc_core::auth_middleware::TenantId;
use alc_core::models::{CreateItem, Item, ItemFile, UpdateItem};
use alc_core::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/items", get(list_items).post(create_item))
        .route("/items/search", get(search_by_barcode))
        .route(
            "/items/{id}",
            get(get_item).put(update_item).delete(delete_item),
        )
        .route("/items/{id}/move", axum::routing::post(move_item))
        .route(
            "/items/{id}/ownership",
            axum::routing::post(change_ownership),
        )
        .route("/items/{id}/convert", axum::routing::post(convert_type))
        .route("/item-files", axum::routing::post(upload_file))
        .route("/item-files/{id}", get(download_file))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    parent_id: Option<Uuid>,
    owner_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    barcode: String,
}

#[derive(Debug, Deserialize)]
struct MoveBody {
    parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct OwnershipBody {
    owner_type: String,
}

#[derive(Debug, Deserialize)]
struct ConvertBody {
    item_type: String,
}

#[derive(Debug, Serialize)]
struct ConvertResponse {
    item: Item,
    children_moved: i64,
}

async fn list_items(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Item>>, StatusCode> {
    let tenant_id = tenant.0 .0;
    let owner_type = query.owner_type.as_deref().unwrap_or("org");

    let items = state
        .items
        .list(tenant_id, query.parent_id, owner_type)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(items))
}

async fn get_item(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Item>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let item = state
        .items
        .get(tenant_id, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(item))
}

async fn create_item(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Json(body): Json<CreateItem>,
) -> Result<(StatusCode, Json<Item>), StatusCode> {
    let tenant_id = tenant.0 .0;

    let item = state
        .items
        .create(tenant_id, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(item)))
}

async fn update_item(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateItem>,
) -> Result<Json<Item>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let item = state
        .items
        .update(tenant_id, id, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(item))
}

async fn delete_item(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let tenant_id = tenant.0 .0;

    let deleted = state
        .items
        .delete(tenant_id, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn move_item(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<MoveBody>,
) -> Result<Json<Item>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let item = state
        .items
        .move_item(tenant_id, id, body.parent_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(item))
}

async fn change_ownership(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<OwnershipBody>,
) -> Result<Json<Item>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let item = state
        .items
        .change_ownership(tenant_id, id, &body.owner_type)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(item))
}

async fn search_by_barcode(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Item>>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let items = state
        .items
        .search_by_barcode(tenant_id, &query.barcode)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(items))
}

async fn convert_type(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<ConvertBody>,
) -> Result<Json<ConvertResponse>, StatusCode> {
    let tenant_id = tenant.0 .0;

    let (item, children_moved) = state
        .items
        .convert_type(tenant_id, id, &body.item_type)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ConvertResponse {
        item,
        children_moved,
    }))
}

async fn upload_file(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ItemFile>), StatusCode> {
    let tenant_id = tenant.0 .0;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| {
            tracing::error!("multipart error: {:?}", e);
            StatusCode::BAD_REQUEST
        })?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let filename = field.file_name().unwrap_or("file").to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let data = field.bytes().await.map_err(|e| {
        tracing::error!("multipart read error: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;
    let size_bytes = data.len() as i64;

    let item_file = state
        .item_files
        .create(tenant_id, &filename, &content_type, size_bytes)
        .await
        .map_err(|e| {
            tracing::error!("item_files create error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let key = format!("items/{}/{}", tenant_id, item_file.id);
    state
        .storage
        .upload(&key, &data, &content_type)
        .await
        .map_err(|e| {
            tracing::error!("storage upload error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::CREATED, Json(item_file)))
}

async fn download_file(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let tenant_id = tenant.0 .0;

    let item_file = state
        .item_files
        .get(tenant_id, id)
        .await
        .map_err(|e| {
            tracing::error!("item_files get error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let key = format!("items/{}/{}", tenant_id, id);
    let data = state.storage.download(&key).await.map_err(|e| {
        tracing::error!("storage download error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                item_file.content_type.clone(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!(
                    "inline; filename=\"{}\"",
                    item_file.filename.replace('"', "\\\"")
                ),
            ),
        ],
        data,
    ))
}
