//! Notify group CRUD + membership management.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use alc_core::auth_middleware::TenantId;
use alc_core::repository::notify_groups::{CreateNotifyGroup, UpdateNotifyGroup};
use alc_core::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/notify/groups", axum::routing::get(list))
        .route("/notify/groups", axum::routing::post(create))
        .route("/notify/groups/{id}", axum::routing::get(get))
        .route("/notify/groups/{id}", axum::routing::put(update))
        .route("/notify/groups/{id}", axum::routing::delete(delete))
        .route(
            "/notify/groups/{id}/members",
            axum::routing::get(list_members),
        )
        .route(
            "/notify/groups/{id}/members",
            axum::routing::post(add_members),
        )
        .route(
            "/notify/groups/{id}/members/{recipient_id}",
            axum::routing::delete(remove_member),
        )
}

#[derive(Debug, Deserialize)]
pub struct AddMembersInput {
    pub recipient_ids: Vec<Uuid>,
}

async fn list(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let groups = state.notify_groups.list(tenant.0).await.map_err(|e| {
        tracing::error!("list notify_groups: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(serde_json::to_value(groups).unwrap()))
}

async fn get(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let group = state
        .notify_groups
        .get(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("get notify_group: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::to_value(group).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Json(input): Json<CreateNotifyGroup>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let group = state
        .notify_groups
        .create(tenant.0, &input)
        .await
        .map_err(|e| {
            tracing::error!("create notify_group: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(group).unwrap()),
    ))
}

async fn update(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateNotifyGroup>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let group = state
        .notify_groups
        .update(tenant.0, id, &input)
        .await
        .map_err(|e| {
            tracing::error!("update notify_group: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::to_value(group).unwrap()))
}

async fn delete(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    state
        .notify_groups
        .delete(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("delete notify_group: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let members = state
        .notify_groups
        .list_members(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("list notify_group members: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::to_value(members).unwrap()))
}

async fn add_members(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddMembersInput>,
) -> Result<StatusCode, StatusCode> {
    state
        .notify_groups
        .add_members(tenant.0, id, &input.recipient_ids)
        .await
        .map_err(|e| {
            tracing::error!("add notify_group members: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path((id, recipient_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    state
        .notify_groups
        .remove_member(tenant.0, id, recipient_id)
        .await
        .map_err(|e| {
            tracing::error!("remove notify_group member: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}
