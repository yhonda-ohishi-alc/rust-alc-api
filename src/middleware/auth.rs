use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Extract tenant_id from X-Tenant-ID header.
/// In production, this should validate a JWT and extract tenant_id from claims.
pub async fn require_tenant(
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let tenant_id = req
        .headers()
        .get("X-Tenant-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(TenantId(tenant_id));
    Ok(next.run(req).await)
}

#[derive(Debug, Clone, Copy)]
pub struct TenantId(pub Uuid);
