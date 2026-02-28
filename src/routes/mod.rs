pub mod auth;
pub mod employees;
pub mod measurements;
pub mod upload;

use axum::{middleware as axum_middleware, Router};

use crate::AppState;
use crate::middleware::auth::{require_jwt, require_tenant};

pub fn router() -> Router<AppState> {
    // JWT 必須ルート (管理者のみ)
    let jwt_protected = Router::new()
        .merge(auth::protected_router())
        .merge(employees::jwt_router())
        .merge(upload::router())
        .layer(axum_middleware::from_fn(require_jwt));

    // キオスク対応ルート (JWT or X-Tenant-ID)
    let tenant_protected = Router::new()
        .merge(measurements::router())
        .merge(employees::tenant_router())
        .layer(axum_middleware::from_fn(require_tenant));

    // 公開ルート (認証不要)
    let public_routes = auth::public_router();

    Router::new()
        .merge(public_routes)
        .merge(jwt_protected)
        .merge(tenant_protected)
}
