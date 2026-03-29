use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use uuid::Uuid;

use crate::db::models::{DtakoEventClassification, UpdateDtakoClassification};
use crate::db::repository::dtako_event_classifications::{
    DtakoEventClassificationsRepository, PgDtakoEventClassificationsRepository,
};
use crate::middleware::auth::TenantId;
use crate::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new()
        .route("/event-classifications", get(list_event_classifications))
        .route("/event-classifications/{id}", put(update_classification))
}

async fn list_event_classifications(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
) -> Result<Json<Vec<DtakoEventClassification>>, StatusCode> {
    let tenant_id = tenant.0 .0;
    let repo = PgDtakoEventClassificationsRepository::new(state.pool.clone());

    let rows = repo
        .list(tenant_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows))
}

async fn update_classification(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDtakoClassification>,
) -> Result<Json<DtakoEventClassification>, (StatusCode, String)> {
    let valid = ["drive", "cargo", "rest_split", "break", "ignore"];
    if !valid.contains(&body.classification.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid classification '{}'. Must be one of: {}",
                body.classification,
                valid.join(", ")
            ),
        ));
    }

    let tenant_id = tenant.0 .0;
    let repo = PgDtakoEventClassificationsRepository::new(state.pool.clone());

    let row = repo
        .update(tenant_id, id, &body.classification)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok(Json(r)),
        None => Err((StatusCode::NOT_FOUND, "Not found".to_string())),
    }
}
