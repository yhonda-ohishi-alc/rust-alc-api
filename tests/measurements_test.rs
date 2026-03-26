mod common;

use serde_json::Value;

// ============================================================
// ヘルパー
// ============================================================

async fn setup() -> (rust_alc_api::AppState, String, uuid::Uuid, String, reqwest::Client) {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;
    let tenant_id = common::create_test_tenant(&state.pool, "Meas Test").await;
    let jwt = common::create_test_jwt(tenant_id, "admin");
    let client = reqwest::Client::new();
    (state, base_url, tenant_id, jwt, client)
}

async fn setup_with_employee() -> (String, String, String, reqwest::Client) {
    let (_, base_url, _, jwt, client) = setup().await;
    let auth = format!("Bearer {jwt}");
    let emp = common::create_test_employee(&client, &base_url, &auth, "TestEmp", "TE01").await;
    let emp_id = emp["id"].as_str().unwrap().to_string();
    (base_url, auth, emp_id, client)
}

// ============================================================
// 作成テスト
// ============================================================

#[tokio::test]
async fn test_create_measurement() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    let res = client
        .post(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "employee_id": emp_id,
            "alcohol_value": 0.05,
            "result_type": "pass"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let m: Value = res.json().await.unwrap();
    assert_eq!(m["alcohol_value"], 0.05);
    assert_eq!(m["result_type"], "pass");
    assert_eq!(m["status"], "completed");
    assert_eq!(m["employee_id"], emp_id);
}

#[tokio::test]
async fn test_create_measurement_with_medical() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    let res = client
        .post(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "employee_id": emp_id,
            "alcohol_value": 0.0,
            "result_type": "pass",
            "temperature": 36.5,
            "systolic": 120,
            "diastolic": 80,
            "pulse": 72,
            "medical_manual_input": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let m: Value = res.json().await.unwrap();
    assert_eq!(m["temperature"], 36.5);
    assert_eq!(m["systolic"], 120);
    assert_eq!(m["diastolic"], 80);
    assert_eq!(m["pulse"], 72);
    assert_eq!(m["medical_manual_input"], true);
}

#[tokio::test]
async fn test_create_measurement_invalid_result_type() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    let res = client
        .post(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "employee_id": emp_id,
            "alcohol_value": 0.0,
            "result_type": "invalid"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_start_measurement() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    let res = client
        .post(format!("{base_url}/api/measurements/start"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({ "employee_id": emp_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let m: Value = res.json().await.unwrap();
    assert_eq!(m["status"], "started");
    assert!(m["alcohol_value"].is_null());
}

// ============================================================
// 一覧・フィルタ・ページネーション
// ============================================================

#[tokio::test]
async fn test_list_measurements_empty() {
    let (base_url, auth, _emp_id, client) = setup_with_employee().await;

    let res = client
        .get(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 0);
    assert_eq!(body["measurements"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_measurements() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;
    common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;

    let res = client
        .get(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 2);
    assert_eq!(body["measurements"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_list_filter_by_employee() {
    let (_, base_url, _, jwt, client) = setup().await;
    let auth = format!("Bearer {jwt}");

    let emp1 = common::create_test_employee(&client, &base_url, &auth, "Emp1", "E01").await;
    let emp2 = common::create_test_employee(&client, &base_url, &auth, "Emp2", "E02").await;
    let id1 = emp1["id"].as_str().unwrap();
    let id2 = emp2["id"].as_str().unwrap();

    common::create_test_measurement(&client, &base_url, &auth, id1).await;
    common::create_test_measurement(&client, &base_url, &auth, id2).await;
    common::create_test_measurement(&client, &base_url, &auth, id1).await;

    let res = client
        .get(format!("{base_url}/api/measurements?employee_id={id1}"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 2);
}

#[tokio::test]
async fn test_list_filter_by_result_type() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    // pass
    common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;
    // fail
    client
        .post(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "employee_id": emp_id,
            "alcohol_value": 0.3,
            "result_type": "fail"
        }))
        .send()
        .await
        .unwrap();

    let res = client
        .get(format!(
            "{base_url}/api/measurements?result_type=fail"
        ))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["measurements"][0]["result_type"], "fail");
}

#[tokio::test]
async fn test_list_filter_by_status() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    // started
    client
        .post(format!("{base_url}/api/measurements/start"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({ "employee_id": emp_id }))
        .send()
        .await
        .unwrap();
    // completed
    common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;

    let res = client
        .get(format!("{base_url}/api/measurements?status=started"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["measurements"][0]["status"], "started");
}

#[tokio::test]
async fn test_list_pagination() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    for _ in 0..3 {
        common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;
    }

    let res = client
        .get(format!(
            "{base_url}/api/measurements?page=1&per_page=2"
        ))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 3);
    assert_eq!(body["measurements"].as_array().unwrap().len(), 2);
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 2);
}

// ============================================================
// 単体取得・更新
// ============================================================

#[tokio::test]
async fn test_get_measurement() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    let m = common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;
    let id = m["id"].as_str().unwrap();

    let res = client
        .get(format!("{base_url}/api/measurements/{id}"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let got: Value = res.json().await.unwrap();
    assert_eq!(got["id"], id);
}

#[tokio::test]
async fn test_get_measurement_not_found() {
    let (base_url, auth, _emp_id, client) = setup_with_employee().await;
    let fake_id = uuid::Uuid::new_v4();

    let res = client
        .get(format!("{base_url}/api/measurements/{fake_id}"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn test_update_measurement() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    let m = common::create_test_measurement(&client, &base_url, &auth, &emp_id).await;
    let id = m["id"].as_str().unwrap();

    let res = client
        .put(format!("{base_url}/api/measurements/{id}"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "alcohol_value": 0.15,
            "temperature": 37.0
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let updated: Value = res.json().await.unwrap();
    assert_eq!(updated["alcohol_value"], 0.15);
    assert_eq!(updated["temperature"], 37.0);
    // 元の値は維持
    assert_eq!(updated["result_type"], "pass");
}

#[tokio::test]
async fn test_update_measurement_complete_flow() {
    let (base_url, auth, emp_id, client) = setup_with_employee().await;

    // start
    let res = client
        .post(format!("{base_url}/api/measurements/start"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({ "employee_id": emp_id }))
        .send()
        .await
        .unwrap();
    let m: Value = res.json().await.unwrap();
    let id = m["id"].as_str().unwrap();
    assert_eq!(m["status"], "started");

    // update → completed
    let res = client
        .put(format!("{base_url}/api/measurements/{id}"))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "status": "completed",
            "alcohol_value": 0.0,
            "result_type": "pass"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let updated: Value = res.json().await.unwrap();
    assert_eq!(updated["status"], "completed");
    assert_eq!(updated["result_type"], "pass");
}

#[tokio::test]
async fn test_measurement_tenant_isolation() {
    let state = common::setup_app_state().await;
    let base_url = common::spawn_test_server(state.clone()).await;

    let tenant_a = common::create_test_tenant(&state.pool, "Meas Iso A").await;
    let tenant_b = common::create_test_tenant(&state.pool, "Meas Iso B").await;

    let jwt_a = common::create_test_jwt(tenant_a, "admin");
    let jwt_b = common::create_test_jwt(tenant_b, "admin");
    let auth_a = format!("Bearer {jwt_a}");
    let auth_b = format!("Bearer {jwt_b}");
    let client = reqwest::Client::new();

    // テナント A に従業員+測定作成
    let emp = common::create_test_employee(&client, &base_url, &auth_a, "EmpA", "EA1").await;
    let emp_id = emp["id"].as_str().unwrap();
    let m = common::create_test_measurement(&client, &base_url, &auth_a, emp_id).await;
    let m_id = m["id"].as_str().unwrap();

    // テナント A → 見える
    let res = client
        .get(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth_a)
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 1);

    // テナント B → 見えない
    let res = client
        .get(format!("{base_url}/api/measurements"))
        .header("Authorization", &auth_b)
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"], 0);

    // テナント B → 個別取得 404
    let res = client
        .get(format!("{base_url}/api/measurements/{m_id}"))
        .header("Authorization", &auth_b)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}
