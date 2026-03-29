use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};

use crate::db::repository::dtako_work_times::{
    DtakoWorkTimesRepository, PgDtakoWorkTimesRepository, WorkTimesFilter, WorkTimesResponse,
};
use crate::middleware::auth::TenantId;
use crate::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new().route("/work-times", get(list_work_times))
}

async fn list_work_times(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
    Query(filter): Query<WorkTimesFilter>,
) -> Result<Json<WorkTimesResponse>, StatusCode> {
    let tenant_id = tenant.0 .0;
    let page = filter.page.unwrap_or(1).max(1);
    let per_page = filter.per_page.unwrap_or(50).min(200);
    let offset = (page - 1) * per_page;

    let repo = PgDtakoWorkTimesRepository::new(state.pool.clone());

    let total = repo
        .count(
            tenant_id,
            filter.driver_id,
            filter.date_from,
            filter.date_to,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let items = repo
        .list(
            tenant_id,
            filter.driver_id,
            filter.date_from,
            filter.date_to,
            per_page,
            offset,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(WorkTimesResponse {
        items,
        total,
        page,
        per_page,
    }))
}
