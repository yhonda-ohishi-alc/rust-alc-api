pub mod auth;
pub mod employees;
pub mod measurements;
pub mod upload;

use axum::{middleware as axum_middleware, Router};

use crate::db::DbPool;
use crate::middleware::auth::require_tenant;

pub fn router() -> Router<DbPool> {
    let protected = Router::new()
        .merge(employees::router())
        .merge(measurements::router())
        .merge(upload::router())
        .layer(axum_middleware::from_fn(require_tenant));

    Router::new()
        .merge(auth::router())
        .merge(protected)
}
