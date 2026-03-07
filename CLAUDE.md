# rust-alc-api

Axum + PostgreSQL RLS による ALC (アルコールチェック) API バックエンド

## プロジェクト構成

- **バックエンド**: Rust / Axum
- **認証**: Google Sign-In JWT
- **DB**: Supabase PostgreSQL (`alc_api` スキーマ、`alc_api_app` ロール NOBYPASSRLS)
- **ストレージ**: Cloudflare R2 (`alc-face-photos` バケット) / GCS 切り替え可能
- **デプロイ**: Cloud Run (`deploy.sh`)

## DB 接続の重要事項

- Supabase は rust-logi と同じプロジェクト (`tvbjvhvslgdwwlhpkezh`)、`alc_api` スキーマで分離
- `alc_api_app` ユーザーで接続すること（NOBYPASSRLS → RLS が有効）
- 必ず **直接接続 (port 5432)** を使用（Supavisor port 6543 は set_config がリセットされる）
- `DATABASE_URL` に `?options=-c search_path=alc_api` を付けてスキーマ指定

## ストレージバックエンド切り替え

- `STORAGE_BACKEND=r2` → Cloudflare R2 (`rust-s3` crate)
- `STORAGE_BACKEND=gcs` → GCS (reqwest 直接呼び出し、Cloud Run メタデータサーバー認証)
- `StorageBackend` trait で抽象化 (`src/storage/`)

## シンボリックリンク（参照用）

プロジェクトルートに関連リポジトリへのシンボリックリンクを配置している。
`.gitignore` に登録済み。VSCode の `git.scanRepositories` で git 操作可能。

| リンク名 | リンク先 | 説明 |
|---|---|---|
| `alc-app` | `/home/yhonda/js/alc-app` | フロントエンド (Nuxt) |
| `rust-nfc-bridge` | `/home/yhonda/rust/rust-nfc-bridge` | NFC ブリッジ (Rust) |
| `ble-medical-gateway` | `/home/yhonda/arduino/ble-medical-gateway` | BLE Medical Gateway (Arduino) |

## ユーティリティ

- `git-status-all.sh` — 自身 + シンボリックリンク先の全リポジトリの git status を一括表示

## マイグレーションとデプロイ

- マイグレーションファイルは `migrations/` ディレクトリに連番で配置 (`001_`, `002_`, ...)
- アプリ起動時に `sqlx::migrate!("./migrations").run(&pool)` で**自動適用**される
- `deploy.sh` は Docker ビルド時に `migrations/` をイメージに含めるため、**デプロイするだけでマイグレーションも適用される**
- 手動で psql を実行する必要はない

## タイムカード機能

- **テーブル**: `timecard_cards` (カード:社員 = 多:1) + `time_punches` (打刻記録)
- **マイグレーション**: `migrations/034_create_timecard.sql`
- **バックエンド**: `src/routes/timecard.rs`
  - カード CRUD: `POST/GET /api/timecard/cards`, `DELETE /api/timecard/cards/{id}`, `GET /api/timecard/cards/by-card/{card_id}`
  - 打刻: `POST /api/timecard/punch` (card_id → 社員特定 → 打刻 + 当日一覧返却)
  - 一覧/CSV: `GET /api/timecard/punches`, `GET /api/timecard/punches/csv`
- **フロントエンド**:
  - `TimePunchKiosk.vue` — 運行者タブ「タイムカード」(NFCタップ→打刻→当日一覧5秒表示)
  - `TimecardManager.vue` — 管理者ダッシュボード「タイムカード」(カード登録 + 打刻履歴 + CSV出力)
- **NFC**: `useNfcWebSocket()` の `onRead` で取得した card_id を `timecard_cards.card_id` と突合

## デプロイルール

- コードの修正・変更が完了したら、デプロイするかどうかを **AskUserQuestion ツールの選択肢形式** で確認すること
- 選択肢: 「デプロイする」「デプロイしない」の2択で提示
- 確認なしに `deploy.sh` を実行してはいけない
- デプロイコマンド: `./deploy.sh` (Cloud Run へデプロイ)
