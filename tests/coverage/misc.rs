/// dtako_operations month==12, middleware auth fallback, nfc_tags DBエラー, health_baselines DBエラー

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_calendar_december() {
    test_group!("カバレッジ 100% 補完");
    test_case!("12月のカレンダーで翌年1月1日を計算する", {
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "CalDec").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        let client = reqwest::Client::new();
        let res = client
            .get(format!(
                "{base_url}/api/operations/calendar?year=2026&month=12"
            ))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["year"], 2026);
        assert_eq!(body["month"], 12);
    });
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_auth_jwt_fail_fallback_to_tenant_id() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "不正 JWT + 有効な X-Tenant-ID でフォールバック成功する",
        {
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(&state.pool, "AuthFB").await;

            let client = reqwest::Client::new();
            let res = client
                .get(format!("{base_url}/api/employees"))
                .header("Authorization", "Bearer invalid-jwt-token")
                .header("X-Tenant-ID", tenant_id.to_string())
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
        }
    );
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_nfc_tag_register_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!("NFC タグ登録で DB エラー時に 500 を返す", {
        let state = crate::common::setup_app_state().await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "NFCErr").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");
        let base_url = crate::common::spawn_test_server(state.clone()).await;

        // pool を閉じて DB エラーを発生させる (他テストに影響しない)
        state.pool.close().await;

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/nfc-tags"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "nfc_uuid": "test-nfc-uuid-cov",
                "car_inspection_id": 99999
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);
    });
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_health_baseline_upsert_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "健康基準値 upsert で DB エラー時に 500 を返す",
        {
            let state = crate::common::setup_app_state().await;
            let tenant_id = crate::common::create_test_tenant(&state.pool, "HBErr").await;
            let jwt = crate::common::create_test_jwt(tenant_id, "admin");
            let base_url = crate::common::spawn_test_server(state.clone()).await;

            // pool を閉じて DB エラーを発生させる (他テストに影響しない)
            state.pool.close().await;

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{base_url}/api/tenko/health-baselines"))
                .header("Authorization", format!("Bearer {jwt}"))
                .json(&serde_json::json!({
                    "employee_id": "00000000-0000-0000-0000-000000000099",
                    "baseline_systolic": 120,
                    "baseline_diastolic": 80,
                    "baseline_temperature": 36.5
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 500);
        }
    );
}
