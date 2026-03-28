/// dtako_operations month==12, middleware auth fallback, nfc_tags DBエラー, health_baselines DBエラー
/// car_inspection get_by_id 成功, employees update_employee ISE, communication_items list DBエラー, sso_admin client_secret None

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
            let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
            let _flock = crate::common::db_rename_flock();
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
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "NFCErr").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        sqlx::query(
            r#"CREATE OR REPLACE FUNCTION alc_api.reject_nfc_insert() RETURNS trigger AS $$
               BEGIN RAISE EXCEPTION 'test: nfc insert blocked'; END;
               $$ LANGUAGE plpgsql"#,
        )
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_nfc_insert BEFORE INSERT ON alc_api.car_inspection_nfc_tags \
             FOR EACH ROW EXECUTE FUNCTION alc_api.reject_nfc_insert()",
        )
        .execute(&state.pool)
        .await
        .unwrap();

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

        sqlx::query("DROP TRIGGER reject_nfc_insert ON alc_api.car_inspection_nfc_tags")
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DROP FUNCTION alc_api.reject_nfc_insert()")
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_health_baseline_upsert_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "健康基準値 upsert で DB エラー時に 500 を返す",
        {
            let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
            let _flock = crate::common::db_rename_flock();
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(&state.pool, "HBErr").await;
            let jwt = crate::common::create_test_jwt(tenant_id, "admin");

            sqlx::query(
                r#"CREATE OR REPLACE FUNCTION alc_api.reject_hb_insert() RETURNS trigger AS $$
               BEGIN RAISE EXCEPTION 'test: health baseline insert blocked'; END;
               $$ LANGUAGE plpgsql"#,
            )
            .execute(&state.pool)
            .await
            .unwrap();
            sqlx::query(
                "CREATE TRIGGER reject_hb_insert BEFORE INSERT ON alc_api.employee_health_baselines \
                 FOR EACH ROW EXECUTE FUNCTION alc_api.reject_hb_insert()",
            )
            .execute(&state.pool)
            .await
            .unwrap();

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

            sqlx::query("DROP TRIGGER reject_hb_insert ON alc_api.employee_health_baselines")
                .execute(&state.pool)
                .await
                .unwrap();
            sqlx::query("DROP FUNCTION alc_api.reject_hb_insert()")
                .execute(&state.pool)
                .await
                .unwrap();
        }
    );
}

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_carrying_items_empty_tenant() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "carrying_items が空のテナントで一覧取得 → 空配列",
        {
            let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
            let _flock = crate::common::db_rename_flock();
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(&state.pool, "CarryEmpty").await;
            let jwt = crate::common::create_test_jwt(tenant_id, "admin");

            let client = reqwest::Client::new();
            let res = client
                .get(format!("{base_url}/api/carrying-items"))
                .header("Authorization", format!("Bearer {jwt}"))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
            let body: Vec<serde_json::Value> = res.json().await.unwrap();
            assert!(body.is_empty(), "New tenant should have no carrying items");
        }
    );
}

// ============================================================
// 1. car_inspections get_by_id success path (line 114)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_car_inspection_get_by_id_success() {
    test_group!("カバレッジ 100% 補完");
    test_case!("car_inspection get_by_id 成功パス", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "CarInsGet").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        // INSERT a minimal car_inspection row with all 96 NOT NULL columns
        let row = sqlx::query_scalar::<_, i32>(
            r#"INSERT INTO alc_api.car_inspection (
                tenant_id,
                "CertInfoImportFileVersion", "Acceptoutputno", "FormType", "ElectCertMgNo",
                "CarId", "ElectCertPublishdateE", "ElectCertPublishdateY",
                "ElectCertPublishdateM", "ElectCertPublishdateD",
                "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD",
                "TranspotationBureauchiefName", "EntryNoCarNo",
                "ReggrantdateE", "ReggrantdateY", "ReggrantdateM", "ReggrantdateD",
                "FirstregistdateE", "FirstregistdateY", "FirstregistdateM",
                "CarName", "CarNameCode", "CarNo", "Model", "EngineModel",
                "OwnernameLowLevelChar", "OwnernameHighLevelChar",
                "OwnerAddressChar", "OwnerAddressNumValue", "OwnerAddressCode",
                "UsernameLowLevelChar", "UsernameHighLevelChar",
                "UserAddressChar", "UserAddressNumValue", "UserAddressCode",
                "UseheadqrterChar", "UseheadqrterNumValue", "UseheadqrterCode",
                "CarKind", "Use", "PrivateBusiness", "CarShape", "CarShapeCode",
                "NoteCap", "Cap", "NoteMaxloadage", "Maxloadage",
                "NoteCarWgt", "CarWgt", "NoteCarTotalWgt", "CarTotalWgt",
                "NoteLength", "Length", "NoteWidth", "Width",
                "NoteHeight", "Height",
                "FfAxWgt", "FrAxWgt", "RfAxWgt", "RrAxWgt",
                "Displacement", "FuelClass", "ModelSpecifyNo", "ClassifyAroundNo",
                "ValidPeriodExpirdateE", "ValidPeriodExpirdateY",
                "ValidPeriodExpirdateM", "ValidPeriodExpirdateD", "NoteInfo",
                "TwodimensionCodeInfoEntryNoCarNo", "TwodimensionCodeInfoCarNo",
                "TwodimensionCodeInfoValidPeriodExpirdate",
                "TwodimensionCodeInfoModel",
                "TwodimensionCodeInfoModelSpecifyNoClassifyAroundNo",
                "TwodimensionCodeInfoCharInfo", "TwodimensionCodeInfoEngineModel",
                "TwodimensionCodeInfoCarNoStampPlace",
                "TwodimensionCodeInfoFirstregistdate",
                "TwodimensionCodeInfoFfAxWgt", "TwodimensionCodeInfoFrAxWgt",
                "TwodimensionCodeInfoRfAxWgt", "TwodimensionCodeInfoRrAxWgt",
                "TwodimensionCodeInfoNoiseReg", "TwodimensionCodeInfoNearNoiseReg",
                "TwodimensionCodeInfoDriveMethod",
                "TwodimensionCodeInfoOpacimeterMeasCar",
                "TwodimensionCodeInfoNoxPmMeasMode",
                "TwodimensionCodeInfoNoxValue", "TwodimensionCodeInfoPmValue",
                "TwodimensionCodeInfoSafeStdDate",
                "TwodimensionCodeInfoFuelClassCode",
                "RegistCarLightCar"
            ) VALUES (
                $1,
                '1','1','1','MGNO1',
                'CID1','1','26','01','01',
                '1','26','01','01',
                'Bureau','ENT1',
                '1','26','01','01',
                '1','26','01',
                'TestCar','TC','1234','MDL','ENG',
                'Owner','Owner',
                'Addr','1','100',
                'User','User',
                'UAddr','1','200',
                'HQ','1','300',
                'Kind','Use','Priv','Shape','SC',
                '','0','','0',
                '','0','','0',
                '','0','','0',
                '','0',
                '0','0','0','0',
                '0','Gas','MSN','CAN',
                '1','28','01','01','',
                'ENT1','1234',
                '280101',
                'MDL',
                'MSCN',
                'CHR','ENG',
                'STP',
                '2601',
                '0','0','0','0',
                '','',
                '',
                '',
                '',
                '','',
                '',
                '',
                '1'
            ) RETURNING id"#,
        )
        .bind(tenant_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{base_url}/api/car-inspections/{row}"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["CarId"], "CID1");

        // Cleanup
        sqlx::query("DELETE FROM alc_api.car_inspection WHERE id = $1")
            .bind(row)
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

// ============================================================
// 2. employees update_employee non-CONFLICT DB error (line 223)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_update_employee_non_conflict_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!("update_employee: 非CONFLICT DBエラー → 500", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "EmpUpdISE").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");
        let auth = format!("Bearer {jwt}");
        let client = reqwest::Client::new();

        // Create an employee first
        let emp =
            crate::common::create_test_employee(&client, &base_url, &auth, "UpdISEEmp", "UI01")
                .await;
        let id = emp["id"].as_str().unwrap();

        // Create a trigger that raises a generic (non-constraint) exception on UPDATE
        sqlx::query(
            r#"CREATE OR REPLACE FUNCTION alc_api.reject_emp_update_ise() RETURNS trigger AS $$
               BEGIN RAISE EXCEPTION 'test: generic update error'; END;
               $$ LANGUAGE plpgsql"#,
        )
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_emp_update_ise BEFORE UPDATE ON alc_api.employees \
             FOR EACH ROW EXECUTE FUNCTION alc_api.reject_emp_update_ise()",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let res = client
            .put(format!("{base_url}/api/employees/{id}"))
            .header("Authorization", &auth)
            .json(&serde_json::json!({
                "name": "Updated",
                "code": "UI02"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);

        // Cleanup
        sqlx::query("DROP TRIGGER reject_emp_update_ise ON alc_api.employees")
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DROP FUNCTION alc_api.reject_emp_update_ise()")
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

// ============================================================
// 3. communication_items list DB error (lines 106-108)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_communication_items_list_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!("communication_items list: RENAME → 500", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "CommListErr").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        // Rename employees table so the JOIN in the second query fails,
        // but the COUNT query (no join) succeeds.
        sqlx::query("ALTER TABLE alc_api.employees RENAME TO employees_bak")
            .execute(&state.pool)
            .await
            .unwrap();

        let client = reqwest::Client::new();
        let res = client
            .get(format!("{base_url}/api/communication-items"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);

        // Restore
        sqlx::query("ALTER TABLE alc_api.employees_bak RENAME TO employees")
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

// ============================================================
// 4. sso_admin upsert client_secret None path (line 113)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_sso_upsert_no_client_secret() {
    test_group!("カバレッジ 100% 補完");
    test_case!("sso upsert: client_secret省略 → None分岐", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        std::env::set_var("JWT_SECRET", crate::common::TEST_JWT_SECRET);
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(
            &state.pool,
            &format!("SSONoSec{}", uuid::Uuid::new_v4().simple()),
        )
        .await;
        let (user_id, _) = crate::common::create_test_user_in_db(
            &state.pool,
            tenant_id,
            "ssonosec@test.com",
            "admin",
        )
        .await;
        let jwt = crate::common::create_test_jwt_for_user(
            user_id,
            tenant_id,
            "ssonosec@test.com",
            "admin",
        );
        let client = reqwest::Client::new();

        // Send upsert request WITHOUT client_secret field at all.
        // This hits line 113 (the `None` branch when client_secret is absent).
        // The DB INSERT will fail because client_secret_encrypted is NOT NULL,
        // but the None path (line 113) is covered before the DB call.
        let res = client
            .post(format!("{base_url}/api/admin/sso/configs"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "provider": "lineworks",
                "client_id": "no-secret-test",
                "external_org_id": "no-secret-org"
            }))
            .send()
            .await
            .unwrap();
        // 500 because DB column client_secret_encrypted is NOT NULL
        assert_eq!(res.status(), 500);
    });
}

// ============================================================
// 5. bot_admin: UPDATE with non-empty client_secret & private_key (lines 134, 148)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_bot_admin_update_with_secrets() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "bot_admin: UPDATE で client_secret/private_key を更新する",
        {
            let _env = crate::common::ENV_LOCK.lock().unwrap();
            std::env::set_var("JWT_SECRET", crate::common::TEST_JWT_SECRET);
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(
                &state.pool,
                &format!("BotUpd{}", uuid::Uuid::new_v4().simple()),
            )
            .await;
            let (user_id, _) = crate::common::create_test_user_in_db(
                &state.pool,
                tenant_id,
                "botupd@test.com",
                "admin",
            )
            .await;
            let jwt = crate::common::create_test_jwt_for_user(
                user_id,
                tenant_id,
                "botupd@test.com",
                "admin",
            );
            let client = reqwest::Client::new();

            // Step 1: CREATE a bot config (no id → INSERT path)
            let res = client
                .post(format!("{base_url}/api/admin/bot/configs"))
                .header("Authorization", format!("Bearer {jwt}"))
                .json(&serde_json::json!({
                    "name": "TestBot",
                    "client_id": "bot-client-id",
                    "client_secret": "initial-secret",
                    "service_account": "sa@test.com",
                    "private_key": "initial-pk",
                    "bot_id": "bot-123"
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
            let created: serde_json::Value = res.json().await.unwrap();
            let config_id = created["id"].as_str().unwrap().to_string();

            // Step 2: UPDATE with non-empty client_secret and private_key (lines 134, 148)
            let res = client
                .post(format!("{base_url}/api/admin/bot/configs"))
                .header("Authorization", format!("Bearer {jwt}"))
                .json(&serde_json::json!({
                    "id": config_id,
                    "name": "TestBotUpdated",
                    "client_id": "bot-client-id",
                    "client_secret": "updated-secret",
                    "service_account": "sa@test.com",
                    "private_key": "updated-pk",
                    "bot_id": "bot-123"
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
            let updated: serde_json::Value = res.json().await.unwrap();
            assert_eq!(updated["name"], "TestBotUpdated");

            // Cleanup
            sqlx::query("DELETE FROM alc_api.bot_configs WHERE id = $1::uuid")
                .bind(&config_id)
                .execute(&state.pool)
                .await
                .unwrap();
        }
    );
}

// ============================================================
// 5b. bot_admin: update WITHOUT secrets → None path (lines 132, 144)
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_bot_admin_update_without_secrets() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "bot_admin update: client_secret/private_key省略 → None分岐",
        {
            let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
            let _flock = crate::common::db_rename_flock();
            let _env = crate::common::ENV_LOCK.lock().unwrap();
            std::env::set_var("JWT_SECRET", crate::common::TEST_JWT_SECRET);
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(
                &state.pool,
                &format!("BotNone{}", uuid::Uuid::new_v4().simple()),
            )
            .await;
            let (user_id, _) = crate::common::create_test_user_in_db(
                &state.pool,
                tenant_id,
                "botnone@test.com",
                "admin",
            )
            .await;
            let jwt = crate::common::create_test_jwt_for_user(
                user_id,
                tenant_id,
                "botnone@test.com",
                "admin",
            );
            let client = reqwest::Client::new();

            // CREATE
            let res = client
                .post(format!("{base_url}/api/admin/bot/configs"))
                .header("Authorization", format!("Bearer {jwt}"))
                .json(&serde_json::json!({
                    "name": "BotNone",
                    "client_id": "none-client",
                    "client_secret": "init-secret",
                    "service_account": "sa@test.com",
                    "private_key": "init-pk",
                    "bot_id": "bot-none"
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
            let created: serde_json::Value = res.json().await.unwrap();
            let config_id = created["id"].as_str().unwrap().to_string();

            // UPDATE without client_secret and private_key → None path
            let res = client
                .post(format!("{base_url}/api/admin/bot/configs"))
                .header("Authorization", format!("Bearer {jwt}"))
                .json(&serde_json::json!({
                    "id": config_id,
                    "name": "BotNoneUpd",
                    "client_id": "none-client",
                    "service_account": "sa@test.com",
                    "bot_id": "bot-none"
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);

            // Cleanup
            sqlx::query("DELETE FROM alc_api.bot_configs WHERE id = $1::uuid")
                .bind(&config_id)
                .execute(&state.pool)
                .await
                .unwrap();
        }
    );
}

// ============================================================
// 6. tenko_schedules: batch empty array → 400
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_schedules_batch_empty() {
    test_group!("カバレッジ 100% 補完");
    test_case!("tenko_schedules batch_create: 空配列 → 400", {
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "TSchEmpty").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko/schedules/batch"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({ "schedules": [] }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400);
    });
}

// ============================================================
// 7. tenko_schedules: pre_operation without instruction → 400
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_schedules_pre_op_no_instruction() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "tenko_schedules: pre_operation without instruction → 400",
        {
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(&state.pool, "TSchNoIns").await;
            let jwt = crate::common::create_test_jwt(tenant_id, "admin");

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{base_url}/api/tenko/schedules/batch"))
                .header("Authorization", format!("Bearer {jwt}"))
                .json(&serde_json::json!({
                    "schedules": [{
                        "employee_id": "00000000-0000-0000-0000-000000000001",
                        "tenko_type": "pre_operation",
                        "responsible_manager_name": "Manager",
                        "scheduled_at": "2026-04-01T09:00:00Z"
                    }]
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 400);
        }
    );
}

// ============================================================
// 8. tenko_schedules: list with employee_id + tenko_type filters
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_schedules_list_with_filters() {
    test_group!("カバレッジ 100% 補完");
    test_case!(
        "tenko_schedules list: employee_id + tenko_type フィルタ",
        {
            let state = crate::common::setup_app_state().await;
            let base_url = crate::common::spawn_test_server(state.clone()).await;
            let tenant_id = crate::common::create_test_tenant(&state.pool, "TSchFilt").await;
            let jwt = crate::common::create_test_jwt(tenant_id, "admin");

            let emp_id = "00000000-0000-0000-0000-000000000099";
            let client = reqwest::Client::new();
            let res = client
                .get(format!(
                    "{base_url}/api/tenko/schedules?employee_id={emp_id}&tenko_type=pre_operation"
                ))
                .header("Authorization", format!("Bearer {jwt}"))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
            let body: serde_json::Value = res.json().await.unwrap();
            assert_eq!(body["total"], 0);
        }
    );
}

// ============================================================
// 9. tenko_schedules: batch_create DB error
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_schedules_batch_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!("tenko_schedules batch_create: DB エラー → 500", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "TSchBErr").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        // Create trigger to block INSERT
        sqlx::query(
            r#"CREATE OR REPLACE FUNCTION alc_api.reject_sched_insert() RETURNS trigger AS $$
               BEGIN RAISE EXCEPTION 'test: schedule insert blocked'; END;
               $$ LANGUAGE plpgsql"#,
        )
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_sched_insert BEFORE INSERT ON alc_api.tenko_schedules \
             FOR EACH ROW EXECUTE FUNCTION alc_api.reject_sched_insert()",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko/schedules/batch"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "schedules": [{
                    "employee_id": "00000000-0000-0000-0000-000000000001",
                    "tenko_type": "post_operation",
                    "responsible_manager_name": "Manager",
                    "scheduled_at": "2026-04-01T09:00:00Z"
                }]
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);

        // Cleanup
        sqlx::query("DROP TRIGGER reject_sched_insert ON alc_api.tenko_schedules")
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DROP FUNCTION alc_api.reject_sched_insert()")
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

// ============================================================
// 10. equipment_failures: list with session_id + date filters
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_equipment_failures_list_with_filters() {
    test_group!("カバレッジ 100% 補完");
    test_case!("equipment_failures list: session_id + date フィルタ", {
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "EqFltFilt").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        let session_id = "00000000-0000-0000-0000-000000000099";
        let client = reqwest::Client::new();
        let res = client
            .get(format!(
                "{base_url}/api/tenko/equipment-failures?session_id={session_id}&date_from=2026-01-01T00:00:00Z&date_to=2026-12-31T23:59:59Z"
            ))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["total"], 0);
    });
}

// ============================================================
// 11. equipment_failures: CSV export with date filters
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_equipment_failures_csv_with_date_filters() {
    test_group!("カバレッジ 100% 補完");
    test_case!("equipment_failures CSV: date フィルタ", {
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "EqCsvFlt").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        let client = reqwest::Client::new();
        let res = client
            .get(format!(
                "{base_url}/api/tenko/equipment-failures/csv?date_from=2026-01-01T00:00:00Z&date_to=2026-12-31T23:59:59Z"
            ))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(content_type.contains("text/csv"));
    });
}

// ============================================================
// 12. tenko_webhooks: invalid event_type → 400
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_webhooks_invalid_event_type() {
    test_group!("カバレッジ 100% 補完");
    test_case!("tenko_webhooks: invalid event_type → 400", {
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "WHInvEvt").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko/webhooks"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "event_type": "invalid_event",
                "url": "https://example.com/webhook"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400);
    });
}

// ============================================================
// 13. tenko_webhooks: get_webhook success + delete not found
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_webhooks_get_and_delete_not_found() {
    test_group!("カバレッジ 100% 補完");
    test_case!("tenko_webhooks: GET成功 + DELETE not found → 404", {
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "WHGetDel").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");
        let client = reqwest::Client::new();

        // Create a webhook config first
        let res = client
            .post(format!("{base_url}/api/tenko/webhooks"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "event_type": "alcohol_detected",
                "url": "https://example.com/webhook",
                "secret": "test-secret"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
        let created: serde_json::Value = res.json().await.unwrap();
        let webhook_id = created["id"].as_str().unwrap();

        // GET the webhook (covers lines 107-134)
        let res = client
            .get(format!("{base_url}/api/tenko/webhooks/{webhook_id}"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);

        // DELETE non-existent webhook → 404 (line 163)
        let fake_id = "00000000-0000-0000-0000-000000000099";
        let res = client
            .delete(format!("{base_url}/api/tenko/webhooks/{fake_id}"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 404);

        // Cleanup: delete the actual webhook
        sqlx::query("DELETE FROM alc_api.webhook_configs WHERE id = $1::uuid")
            .bind(webhook_id)
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

// ============================================================
// 14. tenko_webhooks: upsert DB error
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_webhooks_upsert_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!("tenko_webhooks upsert: DB エラー → 500", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "WHUpsErr").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        // Create trigger to block INSERT on webhook_configs
        sqlx::query(
            r#"CREATE OR REPLACE FUNCTION alc_api.reject_wh_insert() RETURNS trigger AS $$
               BEGIN RAISE EXCEPTION 'test: webhook insert blocked'; END;
               $$ LANGUAGE plpgsql"#,
        )
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_wh_insert BEFORE INSERT ON alc_api.webhook_configs \
             FOR EACH ROW EXECUTE FUNCTION alc_api.reject_wh_insert()",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let client = reqwest::Client::new();
        let res = client
            .post(format!("{base_url}/api/tenko/webhooks"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({
                "event_type": "alcohol_detected",
                "url": "https://example.com/webhook"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);

        // Cleanup
        sqlx::query("DROP TRIGGER reject_wh_insert ON alc_api.webhook_configs")
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DROP FUNCTION alc_api.reject_wh_insert()")
            .execute(&state.pool)
            .await
            .unwrap();
    });
}

// ============================================================
// 15. tenko_webhooks: delete DB error
// ============================================================

#[cfg_attr(not(coverage), ignore)]
#[tokio::test]
async fn test_tenko_webhooks_delete_db_error() {
    test_group!("カバレッジ 100% 補完");
    test_case!("tenko_webhooks delete: DB エラー → 500", {
        let _db = crate::common::DB_RENAME_LOCK.lock().unwrap();
        let _flock = crate::common::db_rename_flock();
        let state = crate::common::setup_app_state().await;
        let base_url = crate::common::spawn_test_server(state.clone()).await;
        let tenant_id = crate::common::create_test_tenant(&state.pool, "WHDelErr").await;
        let jwt = crate::common::create_test_jwt(tenant_id, "admin");

        // RENAME webhook_configs to cause DB error
        sqlx::query("ALTER TABLE alc_api.webhook_configs RENAME TO webhook_configs_bak")
            .execute(&state.pool)
            .await
            .unwrap();

        let client = reqwest::Client::new();
        let fake_id = "00000000-0000-0000-0000-000000000001";
        let res = client
            .delete(format!("{base_url}/api/tenko/webhooks/{fake_id}"))
            .header("Authorization", format!("Bearer {jwt}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 500);

        // Restore
        sqlx::query("ALTER TABLE alc_api.webhook_configs_bak RENAME TO webhook_configs")
            .execute(&state.pool)
            .await
            .unwrap();
    });
}
