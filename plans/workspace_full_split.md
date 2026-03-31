# Cargo Workspace 完全分割計画

## 目的

- テスト並列化: 990 mock tests (49秒/1バイナリ) → ドメイン別バイナリで並列実行
- ビルド高速化: ルート変更時の再コンパイル範囲を限定
- 依存隔離: jsonwebtoken, ring, rust-s3, printpdf を個別クレートに閉じ込め

## 現状 (Phase 1-2 完了後)

```
rust-alc-api/
├── crates/
│   ├── alc-csv-parser/    ✅ (csv, encoding_rs, zip)
│   ├── alc-compare/       ✅ (chrono, serde)
│   └── alc-pdf/           ✅ (printpdf)
└── src/                   ← 残り全部ここ
    ├── auth/              (jwt.rs, google.rs, lineworks.rs)
    ├── db/                (models.rs, tenant.rs, repository/×35)
    ├── middleware/         (auth.rs)
    ├── routes/            (×26 ファイル)
    ├── storage/           (mod.rs, gcs.rs, r2.rs)
    ├── fcm.rs, webhook.rs
    ├── lib.rs, main.rs
    └── test_macros.rs
```

## 目標構成

```
rust-alc-api/
├── crates/
│   ├── alc-csv-parser/    ✅ 完了
│   ├── alc-compare/       ✅ 完了
│   ├── alc-pdf/           ✅ 完了
│   ├── alc-core/          🆕 Phase 3: 共通型 + Repository trait + AppState + middleware
│   ├── alc-auth/          🆕 Phase 4: JWT, Google OAuth, LINE WORKS
│   ├── alc-storage/       🆕 Phase 5: StorageBackend trait + GCS + R2
│   ├── alc-tenko/         🆕 Phase 6: 点呼ドメイン (routes + mock tests)
│   ├── alc-dtako/         🆕 Phase 7: デジタコドメイン (routes + mock tests)
│   ├── alc-devices/       🆕 Phase 8: デバイス管理 (routes + mock tests)
│   ├── alc-carins/        🆕 Phase 9: 車検証ドメイン (routes + mock tests)
│   └── alc-misc/          🆕 Phase 10: 残りの小ルート群
└── alc-server/            🆕 main.rs + ルーター結合のみ
```

---

## Phase 3: alc-core (共通基盤)

**これが全体の鍵。他の全フェーズがここに依存する。**

### 移動するもの

| 移動元 | 移動先 | 内容 |
|--------|--------|------|
| `src/db/models.rs` | `alc-core/src/models.rs` | User, Tenant, Employee, Measurement 等の全モデル型 |
| `src/db/repository/*.rs` (trait 定義のみ) | `alc-core/src/repository/*.rs` | 35個の Repository trait |
| `src/db/tenant.rs` | `alc-core/src/tenant.rs` | テナント関連ユーティリティ |
| `src/middleware/auth.rs` | `alc-core/src/middleware.rs` | require_jwt, require_tenant, AuthUser, TenantId |
| `src/storage/mod.rs` (StorageBackend trait のみ) | `alc-core/src/storage.rs` | StorageBackend trait 定義 |
| `src/fcm.rs` (FcmSenderTrait のみ) | `alc-core/src/fcm.rs` | FcmSenderTrait trait 定義 |
| `src/webhook.rs` (WebhookService trait のみ) | `alc-core/src/webhook.rs` | WebhookService trait 定義 |
| `src/lib.rs` (AppState) | `alc-core/src/lib.rs` | AppState 構造体 |
| `src/test_macros.rs` | `alc-core/src/test_macros.rs` | テスト出力マクロ |

### 残すもの (main crate → 後で alc-server に)

| ファイル | 理由 |
|---------|------|
| `src/db/repository/*.rs` (Pg 実装) | sqlx::query! マクロが DB 接続を要求 |
| `src/routes/*.rs` | 後のフェーズで個別分離 |
| `src/main.rs` | エントリポイント |

### alc-core の依存

```toml
[dependencies]
axum = { version = "0.8", features = ["multipart"] }  # middleware に必要
tokio = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "chrono", "uuid"] }  # FromRow, PgPool
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
async-trait = "0.1"
thiserror = "2"
jsonwebtoken = "9"  # verify_access_token が middleware で使われる
```

### 重要な設計判断

1. **Repository trait と Pg 実装の分離**
   - trait 定義: `alc-core/src/repository/auth.rs` → `pub trait AuthRepository: Send + Sync { ... }`
   - Pg 実装: `src/db/repository/auth.rs` → `impl AuthRepository for PgAuthRepository { ... }`
   - 各 repository ファイルを trait 部分と impl 部分に分割する作業が必要

2. **AppState は alc-core に置く**
   - 全ルートクレートが `State<AppState>` を使うため
   - `AppState` は trait object のみ保持 → Pg 実装への依存なし

3. **middleware は alc-core に置く**
   - `require_jwt` / `require_tenant` は全ルートクレートが使う
   - `verify_access_token` (from alc-auth) を呼ぶため、alc-core は alc-auth に依存
   - → **循環依存の問題**: alc-auth が User 型 (alc-core) に依存し、alc-core が verify_access_token (alc-auth) に依存
   - **解決策**: middleware は alc-core ではなく **alc-auth** に置く。
     alc-core: 型 + trait + AppState (jwt/auth 依存なし)
     alc-auth: jwt + google + lineworks + middleware (alc-core に依存)
     各ルートクレート: alc-core + alc-auth に依存

### 修正後の依存グラフ

```
alc-core (型, trait, AppState)  ← jwt依存なし
    ↑
alc-auth (jwt, google, lineworks, middleware)  ← alc-core に依存
    ↑
alc-tenko, alc-dtako, alc-devices, ...  ← alc-core + alc-auth に依存
    ↑
alc-server (main.rs, Pg実装, ルーター結合)  ← 全部に依存
```

---

## Phase 4: alc-auth

### 移動するもの

| 移動元 | 移動先 |
|--------|--------|
| `src/auth/jwt.rs` | `alc-auth/src/jwt.rs` |
| `src/auth/google.rs` | `alc-auth/src/google.rs` |
| `src/auth/lineworks.rs` | `alc-auth/src/lineworks.rs` |
| `src/middleware/auth.rs` | `alc-auth/src/middleware.rs` |

### User 依存の解決

`create_access_token(user: &User, ...)` の `User` 型は alc-core にある。
alc-auth は alc-core に依存するので、`use alc_core::models::User` で OK。

### alc-auth の依存

```toml
[dependencies]
alc-core = { path = "../alc-core" }
jsonwebtoken = "9"
ring = "0.17"
reqwest = { version = "0.12", features = ["json"] }
axum = "0.8"  # middleware
sha2 = "0.10"
hmac = "0.12"
base64 = "0.22"
```

---

## Phase 5: alc-storage

### 移動するもの

| 移動元 | 移動先 |
|--------|--------|
| `src/storage/gcs.rs` | `alc-storage/src/gcs.rs` |
| `src/storage/r2.rs` | `alc-storage/src/r2.rs` |

StorageBackend trait は Phase 3 で alc-core に移動済み。

### alc-storage の依存

```toml
[dependencies]
alc-core = { path = "../alc-core" }
rust-s3 = { version = "0.35", default-features = false, features = ["tokio-rustls-tls"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
```

---

## Phase 6-10: ドメイン別ルート分離

### ドメイン分割案

| クレート | ルートファイル | mock tests | テスト数 (概算) |
|---------|-------------|------------|---------------|
| **alc-tenko** | tenko_sessions, tenko_records, tenko_schedules, tenko_call, tenko_webhooks, health_baselines, equipment_failures, daily_health | 7 test files | ~250 |
| **alc-dtako** | dtako_upload, dtako_restraint_report, dtako_restraint_report_pdf, dtako_csv_proxy, dtako_drivers, dtako_operations, dtako_work_times, dtako_daily_hours, dtako_vehicles, dtako_event_classifications, dtako_scraper | 11 test files | ~350 |
| **alc-devices** | devices | 1 test file | ~120 |
| **alc-carins** | car_inspections, car_inspection_files, carins_files, nfc_tags | 4 test files | ~80 |
| **alc-misc** | employees, measurements, timecard, carrying_items, communication_items, guidance_records, driver_info, upload, auth, sso_admin, bot_admin, tenant_users | 12 test files | ~190 |

### 各ドメインクレートの構成

```
alc-tenko/
├── Cargo.toml
├── src/
│   ├── lib.rs          # pub fn router() → Router<AppState>
│   ├── sessions.rs     # tenko_sessions ルートハンドラ
│   ├── records.rs
│   ├── schedules.rs
│   ├── call.rs
│   ├── webhooks.rs
│   ├── health_baselines.rs
│   ├── equipment_failures.rs
│   └── daily_health.rs
└── tests/
    └── mock_tests/     # 各ルートの mock tests
```

### 各クレートの依存

```toml
[dependencies]
alc-core = { path = "../alc-core" }     # AppState, Repository traits, models
alc-auth = { path = "../alc-auth" }     # middleware (require_jwt, require_tenant)
axum = "0.8"
serde = { version = "1", features = ["derive"] }
uuid = "1"
chrono = "0.4"
tracing = "0.1"
```

### mock_helpers の共有

mock_helpers (MockAuthRepository 等 34個) は **alc-core の dev-dependencies** として公開:

```
alc-core/src/test_helpers/   # #[cfg(feature = "test-helpers")]
├── mock_repos_a.rs
├── mock_repos_b.rs
├── mock_repos_c.rs
├── mock_app_state.rs
├── mock_storage.rs
└── mock_webhook.rs
```

各ルートクレートの Cargo.toml:
```toml
[dev-dependencies]
alc-core = { path = "../alc-core", features = ["test-helpers"] }
```

---

## 実行順序と見積もり

| Phase | クレート | 作業内容 | 依存関係 |
|-------|---------|---------|---------|
| 3 | alc-core | 型+trait+AppState 抽出、repository trait/impl 分離 | なし |
| 4 | alc-auth | auth+middleware 移動、User→alc_core::models::User | Phase 3 |
| 5 | alc-storage | gcs+r2 移動 | Phase 3 |
| 6 | alc-tenko | 点呼ルート+テスト移動 | Phase 3,4 |
| 7 | alc-dtako | デジタコルート+テスト移動 | Phase 3,4 |
| 8 | alc-devices | デバイスルート+テスト移動 | Phase 3,4 |
| 9 | alc-carins | 車検証ルート+テスト移動 | Phase 3,4 |
| 10 | alc-misc | 残りルート+テスト移動 | Phase 3,4 |

Phase 5 は Phase 3 のみに依存するため、Phase 4 と並列実行可能。
Phase 6-10 は Phase 3,4 完了後に並列実行可能。

---

## リスクと注意点

### 1. Repository trait/impl 分離が最大の作業量
- 35ファイルの各 repository を trait 定義と Pg 実装に分割
- trait 定義 → alc-core、Pg 実装 → alc-server (最終 crate)
- 方法: 各ファイルの `pub trait XxxRepository` を alc-core に移動、`impl XxxRepository for PgXxxRepository` は元の場所に残す

### 2. sqlx::FromRow と models
- `#[derive(FromRow)]` は sqlx に依存 → alc-core が sqlx に依存する必要あり
- ただし sqlx の compile-time checking (`query!`) は Pg 実装側のみなので、alc-core では `sqlx` を features 最小限で使用

### 3. coverage_100.toml のパス更新
- 50ファイル分のパスが変わる → 各フェーズで更新

### 4. CI workflow の更新
- `cargo llvm-cov --workspace` は既に対応済み
- `--test mock_tests` → 各クレートの tests に分散するため、CI スクリプト修正必要

### 5. Dockerfile の更新
- `deploy.sh` のビルドステップが workspace 対応になっているか確認
- `cargo build --release -p alc-server` で最終バイナリを指定

### 6. mock_helpers の feature gate
- `#[cfg(feature = "test-helpers")]` で mock 構造体を公開
- テスト時のみコンパイルされるよう制御

---

## 期待効果

### ビルド時間
- ルート1ファイル変更 → そのドメインクレートのみ再コンパイル
- auth 変更 → alc-auth + 依存クレートのみ
- printpdf, rust-s3, jsonwebtoken, ring は各クレートに隔離済み

### テスト時間
- 現状: mock_tests 1バイナリ 49秒
- 分割後: 5ドメイン × 並列 → 理論上 ~15秒 (最大ドメインの所要時間)
- `cargo test --workspace` で全バイナリ同時実行

### 開発体験
- ドメインごとに独立してテスト実行可能
- `cargo test -p alc-tenko` で点呼系のみ即座にテスト
