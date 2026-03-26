mod common;

use serde_json::Value;

// ============================================================
// 既存テスト (ミドルウェア)
// ============================================================

/// JWT なし + X-Tenant-ID ��し → 401
#[tokio::test]
async fn test_no_auth_returns_unauthorized() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base_url}/api/employees"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "No auth should return 401");
}

/// 無効な JWT → 401 (JWT 必須ルート)
#[tokio::test]
async fn test_invalid_jwt_returns_unauthorized() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state).await;

    let client = reqwest::Client::new();

    // JWT 必須の管理者ルートを使用
    let res = client
        .get(format!("{base_url}/api/auth/me"))
        .header("Authorization", "Bearer invalid-token-here")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "Invalid JWT should return 401");
}

/// 有効な JWT で認証成功
#[tokio::test]
async fn test_valid_jwt_succeeds() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;

    let tenant_id = common::create_test_tenant(&state.pool, "Auth Test Tenant").await;
    let jwt = common::create_test_jwt(tenant_id, "admin");

    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base_url}/api/employees"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "Valid JWT should return 200");
}

/// 不正な UUID の X-Tenant-ID → 401
#[tokio::test]
async fn test_invalid_tenant_id_returns_unauthorized() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base_url}/api/employees"))
        .header("X-Tenant-ID", "not-a-uuid")
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        401,
        "Invalid UUID in X-Tenant-ID should return 401"
    );
}

// ============================================================
// auth/me
// ============================================================

#[tokio::test]
async fn test_me_returns_user_info() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "Me Test").await;

    let (user_id, _) =
        common::create_test_user_in_db(&state.pool, tenant_id, "me@test.com", "admin").await;
    let jwt = common::create_test_jwt_for_user(user_id, tenant_id, "me@test.com", "admin");

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{base_url}/api/auth/me"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["id"], user_id.to_string());
    assert_eq!(body["email"], "me@test.com");
    assert_eq!(body["role"], "admin");
    assert_eq!(body["tenant_id"], tenant_id.to_string());
}

#[tokio::test]
async fn test_me_without_auth() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state).await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base_url}/api/auth/me"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ============================================================
// Refresh Token
// ============================================================

#[tokio::test]
async fn test_refresh_token_success() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "Refresh OK").await;

    let (_user_id, raw_token) =
        common::create_test_user_in_db(&state.pool, tenant_id, "refresh@test.com", "admin").await;

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{base_url}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": raw_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["access_token"].as_str().is_some());
    assert!(body["expires_in"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_refresh_token_invalid() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state).await;
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{base_url}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": "rt_garbage_token" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn test_refresh_token_expired() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "Refresh Exp").await;

    let (_user_id, raw_token) =
        common::create_test_user_in_db(&state.pool, tenant_id, "expired@test.com", "admin").await;

    // 有効期限を過去に更新
    sqlx::query("UPDATE users SET refresh_token_expires_at = NOW() - INTERVAL '1 day' WHERE tenant_id = $1 AND email = 'expired@test.com'")
        .bind(tenant_id)
        .execute(&state.pool)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{base_url}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": raw_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ============================================================
// Logout
// ============================================================

#[tokio::test]
async fn test_logout_clears_refresh() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "Logout Test").await;

    let (user_id, raw_token) =
        common::create_test_user_in_db(&state.pool, tenant_id, "logout@test.com", "admin").await;
    let jwt = common::create_test_jwt_for_user(user_id, tenant_id, "logout@test.com", "admin");

    let client = reqwest::Client::new();

    // Logout
    let res = client
        .post(format!("{base_url}/api/auth/logout"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // Refresh should fail now
    let res = client
        .post(format!("{base_url}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": raw_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ============================================================
// テナント作成 + my-orgs
// ============================================================

#[tokio::test]
async fn test_create_tenant_endpoint() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state).await;
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{base_url}/api/auth/tenants"))
        .json(&serde_json::json!({ "name": "New Tenant" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["name"], "New Tenant");
    assert!(body["id"].as_str().is_some());
}

#[tokio::test]
async fn test_my_orgs() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "MyOrgs Test").await;

    let (user_id, _) =
        common::create_test_user_in_db(&state.pool, tenant_id, "orgs@test.com", "admin").await;
    let jwt = common::create_test_jwt_for_user(user_id, tenant_id, "orgs@test.com", "admin");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{base_url}/api/my-orgs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let orgs = body["organizations"].as_array().unwrap();
    assert_eq!(orgs.len(), 1);
    assert_eq!(orgs[0]["id"], tenant_id.to_string());
}
