mod common;

use serde_json::Value;

/// JWT付きadminユーザーセットアップ
async fn setup_admin() -> (rust_alc_api::AppState, String, uuid::Uuid, String, reqwest::Client) {
    // SSO暗号化に必要
    std::env::set_var("JWT_SECRET", common::TEST_JWT_SECRET);
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, &format!("Admin{}", uuid::Uuid::new_v4().simple())).await;
    let (user_id, _) = common::create_test_user_in_db(&state.pool, tenant_id, "admin@test.com", "admin").await;
    let jwt = common::create_test_jwt_for_user(user_id, tenant_id, "admin@test.com", "admin");
    let client = reqwest::Client::new();
    (state, base_url, tenant_id, jwt, client)
}

// ============================================================
// Tenant Users
// ============================================================

#[tokio::test]
async fn test_list_users() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    let res = client
        .get(format!("{base_url}/api/admin/users"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["users"].as_array().is_some());
}

#[tokio::test]
async fn test_list_invitations() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    let res = client
        .get(format!("{base_url}/api/admin/users/invitations"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["invitations"].as_array().is_some());
}

#[tokio::test]
async fn test_invite_user() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    let res = client
        .post(format!("{base_url}/api/admin/users/invite"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({
            "email": "newuser@example.com",
            "role": "viewer"
        }))
        .send().await.unwrap();
    assert!(res.status() == 200 || res.status() == 201, "invite: {}", res.status());

    // 招待一覧に表示
    let res = client
        .get(format!("{base_url}/api/admin/users/invitations"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let invitations = body["invitations"].as_array().unwrap();
    assert!(invitations.iter().any(|i| i["email"] == "newuser@example.com"));
}

#[tokio::test]
async fn test_invite_and_delete_invitation() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    let res = client
        .post(format!("{base_url}/api/admin/users/invite"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({ "email": "delete-me@example.com" }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let inv_id = body["id"].as_str().unwrap();

    let res = client
        .delete(format!("{base_url}/api/admin/users/invite/{inv_id}"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 204);
}

#[tokio::test]
async fn test_delete_user() {
    let (state, base_url, tenant_id, jwt, client) = setup_admin().await;

    // 削除用ユーザーを作成
    let (target_id, _) = common::create_test_user_in_db(&state.pool, tenant_id, "target@test.com", "viewer").await;

    let res = client
        .delete(format!("{base_url}/api/admin/users/{target_id}"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 204);
}

#[tokio::test]
async fn test_users_forbidden_for_viewer() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "ViewerForbid").await;
    let (user_id, _) = common::create_test_user_in_db(&state.pool, tenant_id, "viewer@test.com", "viewer").await;
    let jwt = common::create_test_jwt_for_user(user_id, tenant_id, "viewer@test.com", "viewer");
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base_url}/api/admin/users"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 403);
}

// ============================================================
// SSO Admin
// ============================================================

#[tokio::test]
async fn test_sso_list_configs() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    let res = client
        .get(format!("{base_url}/api/admin/sso/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["configs"].as_array().is_some());
}

#[tokio::test]
async fn test_sso_upsert_and_delete_config() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    // Upsert
    let res = client
        .post(format!("{base_url}/api/admin/sso/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({
            "provider": "lineworks",
            "client_id": "test-client-id",
            "client_secret": "test-secret",
            "external_org_id": "test-org",
            "woff_id": "test-woff"
        }))
        .send().await.unwrap();
    assert!(res.status() == 200 || res.status() == 201, "sso upsert: {}", res.status());

    // List to verify
    let res = client
        .get(format!("{base_url}/api/admin/sso/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    assert!(!body["configs"].as_array().unwrap().is_empty());

    // Delete
    let res = client
        .delete(format!("{base_url}/api/admin/sso/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({ "provider": "lineworks" }))
        .send().await.unwrap();
    assert_eq!(res.status(), 204);
}

// ============================================================
// Bot Admin
// ============================================================

#[tokio::test]
async fn test_bot_list_configs() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    let res = client
        .get(format!("{base_url}/api/admin/bot/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["configs"].as_array().is_some());
}

#[tokio::test]
async fn test_bot_upsert_and_delete_config() {
    let (_state, base_url, _tenant_id, jwt, client) = setup_admin().await;

    // Upsert (create)
    let res = client
        .post(format!("{base_url}/api/admin/bot/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({
            "name": "Test Bot",
            "client_id": "bot-client-id",
            "client_secret": "bot-secret",
            "service_account": "bot@service",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----",
            "bot_id": "bot-123"
        }))
        .send().await.unwrap();
    assert!(res.status() == 200 || res.status() == 201, "bot upsert: {}", res.status());
    let body: Value = res.json().await.unwrap();
    let bot_id = body["id"].as_str().unwrap();

    // Delete
    let res = client
        .delete(format!("{base_url}/api/admin/bot/configs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({ "id": bot_id }))
        .send().await.unwrap();
    assert_eq!(res.status(), 204);
}
