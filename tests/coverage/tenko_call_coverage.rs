use crate::common;

// ============================================================
// tenko_call register — DB エラー (trigger で INSERT 拒否)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_register_db_error() {
    test_group!("tenko_call カバレッジ");
    test_case!(
        "register: driver upsert 失敗 → 500 + register_err カバー",
        {
            let state = common::setup_app_state().await;
            let base_url = common::spawn_test_server(state.clone()).await;

            // pool を閉じて DB エラーを発生させる (他テストに影響しない)
            state.pool.close().await;

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{base_url}/api/tenko-call/register"))
                .json(&serde_json::json!({
                    "phone_number": "090-0000-err1",
                    "driver_name": "エラーテスト",
                    "call_number": "nonexistent"
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 500);
            let body: serde_json::Value = res.json().await.unwrap();
            assert_eq!(body["success"], false);
            assert!(body["error"].as_str().unwrap().contains("internal error"));
        }
    );
}

// ============================================================
// tenko_call tenko — DB エラー (trigger で tenko_call_logs INSERT 拒否)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_tenko_db_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("tenko: log insert 失敗 → 500", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;

        // pool を閉じて DB エラーを発生させる (他テストに影響しない)
        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko-call/tenko"))
            .json(&serde_json::json!({
                "phone_number": "090-err-test",
                "driver_name": "テンコエラー",
                "latitude": 35.0,
                "longitude": 139.0
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

// ============================================================
// register — master lookup DB エラー (RENAME tenko_call_numbers)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_register_master_lookup_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("register: master lookup 失敗 → 500", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;

        // pool を閉じて DB エラーを発生させる (他テストに影響しない)
        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko-call/register"))
            .json(&serde_json::json!({
                "phone_number": "090-master-err",
                "driver_name": "マスタエラー",
                "call_number": "nonexistent"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

// ============================================================
// tenko — driver lookup DB エラー (RENAME tenko_call_drivers)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_tenko_driver_lookup_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("tenko: driver lookup 失敗 → 500", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;

        // pool を閉じて DB エラーを発生させる (他テストに影響しない)
        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko-call/tenko"))
            .json(&serde_json::json!({
                "phone_number": "090-drv-err",
                "driver_name": "ドライバーエラー",
                "latitude": 35.0,
                "longitude": 139.0
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

// ============================================================
// list_numbers / create_number / list_drivers — DB エラー (RENAME)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_list_numbers_db_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("list_numbers: DB エラー → 500", {
        let state = common::setup_app_state().await;
        let tenant_id = common::create_test_tenant(&state.pool, "TkListNumErr").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let base_url = common::spawn_test_server(state.clone()).await;

        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{base_url}/api/tenko-call/numbers"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_create_number_db_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("create_number: DB エラー → 500", {
        let state = common::setup_app_state().await;
        let tenant_id = common::create_test_tenant(&state.pool, "TkCreateNumErr").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let base_url = common::spawn_test_server(state.clone()).await;

        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko-call/numbers"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "call_number": "err-create-001",
                "label": "test"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_delete_number_db_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("delete_number: DB エラー → 500", {
        let state = common::setup_app_state().await;
        let tenant_id = common::create_test_tenant(&state.pool, "TkDelNumErr").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let base_url = common::spawn_test_server(state.clone()).await;

        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .delete(format!("{base_url}/api/tenko-call/numbers/99999"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_call_list_drivers_db_error() {
    test_group!("tenko_call カバレッジ");
    test_case!("list_drivers: DB エラー → 500", {
        let state = common::setup_app_state().await;
        let tenant_id = common::create_test_tenant(&state.pool, "TkListDrvErr").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let base_url = common::spawn_test_server(state.clone()).await;

        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{base_url}/api/tenko-call/drivers"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}
