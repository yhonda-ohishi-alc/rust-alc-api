use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use alc_core::auth_middleware::TenantId;
use alc_core::repository::notify_recipients::{CreateNotifyRecipient, UpdateNotifyRecipient};
use alc_core::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/notify/recipients", axum::routing::get(list))
        .route("/notify/recipients", axum::routing::post(create))
        .route("/notify/recipients/bulk", axum::routing::post(bulk_upsert))
        .route("/notify/recipients/{id}", axum::routing::get(get))
        .route("/notify/recipients/{id}", axum::routing::put(update))
        .route("/notify/recipients/{id}", axum::routing::delete(delete))
}

#[derive(Debug, Deserialize)]
pub struct BulkUpsertInput {
    pub recipients: Vec<CreateNotifyRecipient>,
    /// If provided, each created/updated recipient is also added to these groups.
    #[serde(default)]
    pub group_ids: Vec<Uuid>,
}

async fn bulk_upsert(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Json(input): Json<BulkUpsertInput>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    let mut touched_ids: Vec<Uuid> = Vec::new();

    // Build existing lineworks_user_id → id map once (avoids N+1 list calls).
    let existing = state.notify_recipients.list(tenant.0).await.map_err(|e| {
        tracing::error!("bulk list notify_recipients: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let existing_by_lw: std::collections::HashMap<String, Uuid> = existing
        .into_iter()
        .filter_map(|r| r.lineworks_user_id.map(|id| (id, r.id)))
        .collect();

    for (idx, r) in input.recipients.iter().enumerate() {
        let Some(lw_id) = r.lineworks_user_id.as_deref() else {
            skipped.push(serde_json::json!({
                "index": idx,
                "reason": "missing_lineworks_user_id",
            }));
            continue;
        };

        let pre_existing_id = existing_by_lw.get(lw_id).copied();

        let result = if let Some(id) = pre_existing_id {
            state
                .notify_recipients
                .update(
                    tenant.0,
                    id,
                    &UpdateNotifyRecipient {
                        name: Some(r.name.clone()),
                        provider: Some(r.provider.clone()),
                        lineworks_user_id: Some(lw_id.to_string()),
                        line_user_id: None,
                        phone_number: r.phone_number.clone(),
                        email: r.email.clone(),
                        enabled: None,
                    },
                )
                .await
        } else {
            state.notify_recipients.create(tenant.0, r).await
        };

        match result {
            Ok(rec) => {
                if pre_existing_id.is_some() {
                    updated += 1;
                } else {
                    created += 1;
                }
                touched_ids.push(rec.id);
            }
            Err(e) => {
                tracing::error!("bulk upsert notify_recipient: {e}");
                skipped.push(serde_json::json!({
                    "index": idx,
                    "reason": "db_error",
                }));
            }
        }
    }

    // Attach to groups (best-effort; errors are logged but don't fail the whole batch).
    for group_id in &input.group_ids {
        if let Err(e) = state
            .notify_groups
            .add_members(tenant.0, *group_id, &touched_ids)
            .await
        {
            tracing::error!("bulk add_members to group {group_id}: {e}");
        }
    }

    Ok(Json(serde_json::json!({
        "created": created,
        "updated": updated,
        "skipped": skipped,
        "touched_recipient_ids": touched_ids,
    })))
}

async fn list(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let recipients = state.notify_recipients.list(tenant.0).await.map_err(|e| {
        tracing::error!("list notify_recipients: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(serde_json::to_value(recipients).unwrap()))
}

async fn get(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let recipient = state
        .notify_recipients
        .get(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("get notify_recipient: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::to_value(recipient).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Json(input): Json<CreateNotifyRecipient>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let recipient = state
        .notify_recipients
        .create(tenant.0, &input)
        .await
        .map_err(|e| {
            tracing::error!("create notify_recipient: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(recipient).unwrap()),
    ))
}

async fn update(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<uuid::Uuid>,
    Json(input): Json<UpdateNotifyRecipient>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let recipient = state
        .notify_recipients
        .update(tenant.0, id, &input)
        .await
        .map_err(|e| {
            tracing::error!("update notify_recipient: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::to_value(recipient).unwrap()))
}

async fn delete(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantId>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    state
        .notify_recipients
        .delete(tenant.0, id)
        .await
        .map_err(|e| {
            tracing::error!("delete notify_recipient: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}
