use axum::{extract::State, http::StatusCode, routing::get, Json, Router};

use crate::db::models::DtakoVehicle;
use crate::db::repository::dtako_vehicles::{DtakoVehiclesRepository, PgDtakoVehiclesRepository};
use crate::middleware::auth::TenantId;
use crate::AppState;

pub fn tenant_router() -> Router<AppState> {
    Router::new().route("/vehicles", get(list_vehicles))
}

async fn list_vehicles(
    State(state): State<AppState>,
    tenant: axum::Extension<TenantId>,
) -> Result<Json<Vec<DtakoVehicle>>, StatusCode> {
    let tenant_id = tenant.0 .0;
    let repo = PgDtakoVehiclesRepository::new(state.pool.clone());

    let vehicles = repo
        .list(tenant_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(vehicles))
}
