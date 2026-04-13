use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::TroubleState;
use alc_core::auth_middleware::TenantId;
use alc_core::models::{CreateTroubleTask, TroubleFile, TroubleTask, UpdateTroubleTask};

const VALID_STATUSES: &[&str] = &["open", "in_progress", "done"];

pub fn tenant_router<S>() -> Router<S>
where
    TroubleState: axum::extract::FromRef<S>,
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/trouble/tickets/{ticket_id}/tasks",
            post(create_task).get(list_tasks),
        )
        .route(
            "/trouble/tasks/{task_id}",
            axum::routing::put(update_task).delete(delete_task),
        )
        .route(
            "/trouble/tasks/{task_id}/files",
            post(upload_task_file).get(list_task_files),
        )
        .route(
            "/trouble/task-files/{file_id}/download",
            get(download_task_file),
        )
        .route("/trouble/task-files/{file_id}", delete(delete_task_file))
}

async fn create_task(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(ticket_id): Path<Uuid>,
    Json(body): Json<CreateTroubleTask>,
) -> Result<(StatusCode, Json<TroubleTask>), StatusCode> {
    if body.title.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let tenant_id = tenant.0 .0;

    let task = state
        .trouble_tasks
        .create(tenant_id, ticket_id, None, &body)
        .await
        .map_err(|e| {
            tracing::error!("create_task error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // タスクアサイン通知
    if let Some(assigned_to_id) = body.assigned_to {
        if let Some(notifier) = &state.notifier {
            if let Ok(Some(pref)) = state
                .trouble_notification_prefs
                .find_enabled(tenant_id, "task_assigned", "lineworks")
                .await
            {
                let ticket_no = state
                    .trouble_tickets
                    .get(tenant_id, ticket_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|t| t.ticket_no)
                    .unwrap_or(0);
                let emp_name = if let Some(ref employees) = state.employees {
                    employees
                        .get(tenant_id, assigned_to_id)
                        .await
                        .ok()
                        .flatten()
                        .map(|e| e.name)
                } else {
                    None
                };
                let msg = format!(
                    "タスクアサイン: #{} {} → {}",
                    ticket_no,
                    task.title,
                    emp_name.as_deref().unwrap_or("不明"),
                );
                notifier
                    .notify(tenant_id, "task_assigned", &msg, &pref.lineworks_user_ids)
                    .await;
            }
        }
    }

    Ok((StatusCode::CREATED, Json(task)))
}

async fn list_tasks(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<Vec<TroubleTask>>, StatusCode> {
    let tasks = state
        .trouble_tasks
        .list_by_ticket(tenant.0 .0, ticket_id)
        .await
        .map_err(|e| {
            tracing::error!("list_tasks error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(tasks))
}

async fn update_task(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<UpdateTroubleTask>,
) -> Result<Json<TroubleTask>, StatusCode> {
    // Validate status if provided
    if let Some(ref status) = body.status {
        if !VALID_STATUSES.contains(&status.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let tenant_id = tenant.0 .0;

    // assigned_to が提供され、かつ Some(uuid) の場合に通知対象
    let has_assigned_to = matches!(body.assigned_to, Some(Some(_)));

    let task = state
        .trouble_tasks
        .update(tenant_id, task_id, &body)
        .await
        .map_err(|e| {
            tracing::error!("update_task error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // タスクアサイン通知
    if has_assigned_to {
        if let Some(assigned_to_id) = task.assigned_to {
            if let Some(notifier) = &state.notifier {
                if let Ok(Some(pref)) = state
                    .trouble_notification_prefs
                    .find_enabled(tenant_id, "task_assigned", "lineworks")
                    .await
                {
                    let ticket_no = state
                        .trouble_tickets
                        .get(tenant_id, task.ticket_id)
                        .await
                        .ok()
                        .flatten()
                        .map(|t| t.ticket_no)
                        .unwrap_or(0);
                    let emp_name = if let Some(ref employees) = state.employees {
                        employees
                            .get(tenant_id, assigned_to_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|e| e.name)
                    } else {
                        None
                    };
                    let msg = format!(
                        "タスクアサイン: #{} {} → {}",
                        ticket_no,
                        task.title,
                        emp_name.as_deref().unwrap_or("不明"),
                    );
                    notifier
                        .notify(tenant_id, "task_assigned", &msg, &pref.lineworks_user_ids)
                        .await;
                }
            }
        }
    }

    Ok(Json(task))
}

async fn delete_task(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(task_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = state
        .trouble_tasks
        .delete(tenant.0 .0, task_id)
        .await
        .map_err(|e| {
            tracing::error!("delete_task error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// --- Task File Handlers ---

async fn upload_task_file(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(task_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<TroubleFile>), StatusCode> {
    let tenant_id = tenant.0 .0;

    // タスクの存在確認 + ticket_id 取得
    let task = state
        .trouble_tasks
        .get(tenant_id, task_id)
        .await
        .map_err(|e| {
            tracing::error!("upload_task_file: get task error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let storage = state
        .trouble_storage
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let filename = field.file_name().unwrap_or("unknown").to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
    let size_bytes = data.len() as i64;

    let file_uuid = Uuid::new_v4();
    let ext = filename.rsplit('.').next().unwrap_or("bin");
    let storage_key = format!("{tenant_id}/trouble/tasks/{task_id}/{file_uuid}.{ext}");

    storage
        .upload(&storage_key, &data, &content_type)
        .await
        .map_err(|e| {
            tracing::error!("storage upload error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let file = state
        .trouble_files
        .create_for_task(
            tenant_id,
            task.ticket_id,
            task_id,
            &filename,
            &content_type,
            size_bytes,
            &storage_key,
        )
        .await
        .map_err(|e| {
            tracing::error!("create_task_file DB error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((StatusCode::CREATED, Json(file)))
}

async fn list_task_files(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<TroubleFile>>, StatusCode> {
    let files = state
        .trouble_files
        .list_by_task(tenant.0 .0, task_id)
        .await
        .map_err(|e| {
            tracing::error!("list_task_files error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(files))
}

async fn download_task_file(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(file_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let file = state
        .trouble_files
        .get(tenant.0 .0, file_id)
        .await
        .map_err(|e| {
            tracing::error!("download_task_file error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let storage = state
        .trouble_storage
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let data = storage.download(&file.storage_key).await.map_err(|e| {
        tracing::error!("storage download error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, file.content_type.clone()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    file.filename.replace('"', "_")
                ),
            ),
        ],
        data,
    ))
}

async fn delete_task_file(
    State(state): State<TroubleState>,
    tenant: axum::Extension<TenantId>,
    Path(file_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = state
        .trouble_files
        .delete(tenant.0 .0, file_id)
        .await
        .map_err(|e| {
            tracing::error!("delete_task_file error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
