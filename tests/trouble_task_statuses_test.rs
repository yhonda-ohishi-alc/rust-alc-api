#[macro_use]
mod common;

use serde_json::Value;

// ============================================================
// trouble_task_statuses master CRUD + RLS
// ============================================================

#[tokio::test]
async fn test_task_statuses_seed_on_create_tenant() {
    test_group!("trouble_task_statuses: migration-seeded defaults");
    test_case!(
        "テナント作成直後: migration の seed で4件返る",
        {
            let state = common::setup_app_state().await;
            let base_url = common::spawn_test_server(state.clone()).await;
            let tenant_id =
                common::create_test_tenant(state.pool(), "Task Statuses Tenant A").await;
            let jwt = common::create_test_jwt(tenant_id, "admin");
            let client = reqwest::Client::new();
            let auth = format!("Bearer {jwt}");

            let res = client
                .get(format!("{base_url}/api/trouble/task-statuses"))
                .header("Authorization", &auth)
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 200);
            let list: Vec<Value> = res.json().await.unwrap();
            assert_eq!(list.len(), 4, "should have 4 default statuses");

            let keys: Vec<_> = list.iter().map(|s| s["key"].as_str().unwrap()).collect();
            assert!(keys.contains(&"open"));
            assert!(keys.contains(&"in_progress"));
            assert!(keys.contains(&"waiting"));
            assert!(keys.contains(&"done"));

            // is_done true only for `done`
            for s in &list {
                let expected_done = s["key"].as_str() == Some("done");
                assert_eq!(
                    s["is_done"].as_bool().unwrap(),
                    expected_done,
                    "is_done for {}",
                    s["key"]
                );
            }

            // sort_order ascending
            let sort_orders: Vec<i64> = list
                .iter()
                .map(|s| s["sort_order"].as_i64().unwrap())
                .collect();
            let mut sorted = sort_orders.clone();
            sorted.sort();
            assert_eq!(sort_orders, sorted, "results should be sort_order ASC");
        }
    );
}

#[tokio::test]
async fn test_task_statuses_crud() {
    test_group!("trouble_task_statuses: POST / PUT / DELETE");
    test_case!("作成・更新・削除のハッピーパス", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;
        let tenant_id = common::create_test_tenant(state.pool(), "Task Statuses Tenant B").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let client = reqwest::Client::new();
        let auth = format!("Bearer {jwt}");

        // POST: create a custom status
        let res = client
            .post(format!("{base_url}/api/trouble/task-statuses"))
            .header("Authorization", &auth)
            .json(&serde_json::json!({
                "key": "on_hold",
                "name": "保留",
                "color": "#ABCDEF",
                "sort_order": 25,
                "is_done": false,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
        let created: Value = res.json().await.unwrap();
        assert_eq!(created["key"], "on_hold");
        assert_eq!(created["name"], "保留");
        assert_eq!(created["color"], "#ABCDEF");
        let id = created["id"].as_str().unwrap().to_string();

        // PUT: update sort_order + name + color + is_done
        let res = client
            .put(format!("{base_url}/api/trouble/task-statuses/{id}"))
            .header("Authorization", &auth)
            .json(&serde_json::json!({
                "sort_order": 99,
                "name": "保留中",
                "color": "#112233",
                "is_done": true,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let updated: Value = res.json().await.unwrap();
        assert_eq!(updated["sort_order"], 99);
        assert_eq!(updated["name"], "保留中");
        assert_eq!(updated["color"], "#112233");
        assert_eq!(updated["is_done"], true);

        // DELETE
        let res = client
            .delete(format!("{base_url}/api/trouble/task-statuses/{id}"))
            .header("Authorization", &auth)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 204);

        // DELETE again → 404
        let res = client
            .delete(format!("{base_url}/api/trouble/task-statuses/{id}"))
            .header("Authorization", &auth)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    });
}

#[tokio::test]
async fn test_task_statuses_create_empty_name_400() {
    test_group!("trouble_task_statuses: 空文字バリデーション");
    test_case!("空 name → 400", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;
        let tenant_id =
            common::create_test_tenant(state.pool(), "Task Statuses Tenant Empty").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let client = reqwest::Client::new();
        let auth = format!("Bearer {jwt}");

        let res = client
            .post(format!("{base_url}/api/trouble/task-statuses"))
            .header("Authorization", &auth)
            .json(&serde_json::json!({ "name": "   " }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400);
    });
}

#[tokio::test]
async fn test_task_statuses_unique_name_conflict() {
    test_group!("trouble_task_statuses: UNIQUE(name) 競合");
    test_case!("同じ name で再 POST → 409 CONFLICT", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;
        let tenant_id = common::create_test_tenant(state.pool(), "Task Statuses Tenant Dup").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let client = reqwest::Client::new();
        let auth = format!("Bearer {jwt}");

        // Seed 4 defaults already inserted by migration → `未着手` exists.
        let res = client
            .post(format!("{base_url}/api/trouble/task-statuses"))
            .header("Authorization", &auth)
            .json(&serde_json::json!({
                "key": "another",
                "name": "未着手",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 409);
    });
}

#[tokio::test]
async fn test_task_statuses_rls_isolation() {
    test_group!("trouble_task_statuses: RLS テナント分離");
    test_case!("Tenant A の status は Tenant B から見えない", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;
        let tenant_a = common::create_test_tenant(state.pool(), "Task Statuses RLS A").await;
        let tenant_b = common::create_test_tenant(state.pool(), "Task Statuses RLS B").await;

        let jwt_a = common::create_test_jwt(tenant_a, "admin");
        let jwt_b = common::create_test_jwt(tenant_b, "admin");
        let client = reqwest::Client::new();

        // A creates unique status
        let res = client
            .post(format!("{base_url}/api/trouble/task-statuses"))
            .header("Authorization", format!("Bearer {jwt_a}"))
            .json(&serde_json::json!({
                "key": "tenant_a_only",
                "name": "A専用",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);

        // B should not see it
        let res = client
            .get(format!("{base_url}/api/trouble/task-statuses"))
            .header("Authorization", format!("Bearer {jwt_b}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let b_list: Vec<Value> = res.json().await.unwrap();
        assert!(
            b_list
                .iter()
                .all(|s| s["key"].as_str() != Some("tenant_a_only")),
            "tenant B must not see tenant A's status"
        );
    });
}

#[tokio::test]
async fn test_task_statuses_update_not_found() {
    test_group!("trouble_task_statuses: PUT 404");
    test_case!("存在しない id への PUT → 404", {
        let state = common::setup_app_state().await;
        let base_url = common::spawn_test_server(state.clone()).await;
        let tenant_id = common::create_test_tenant(state.pool(), "Task Statuses Tenant 404").await;
        let jwt = common::create_test_jwt(tenant_id, "admin");
        let client = reqwest::Client::new();
        let auth = format!("Bearer {jwt}");

        let bogus = uuid::Uuid::new_v4();
        let res = client
            .put(format!("{base_url}/api/trouble/task-statuses/{bogus}"))
            .header("Authorization", &auth)
            .json(&serde_json::json!({ "sort_order": 7 }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    });
}
