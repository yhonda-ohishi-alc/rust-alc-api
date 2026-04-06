# gRPC→REST 移行 動作確認チェックリスト

staging 環境で確認。auth-worker: `https://auth-worker-staging.m-tama-ramu.workers.dev`

## 前提条件

- [ ] rust-alc-api PR#114 が main にマージ済み
- [ ] auth-worker PR#26 が main にマージ済み (または staging にデプロイ済み)
- [ ] staging DB にテストテナント + ユーザーが存在する

---

## 1. 管理画面 (cookie 認証 + HTML)

### /admin/users — ユーザー管理
- [ ] 未ログイン状態でアクセス → `/login` にリダイレクトされる
- [ ] ログイン後 → ユーザー一覧が表示される
- [ ] 「追加済み（未ログイン）」セクションが表示される
- [ ] メールアドレス入力 → 「追加する」 → 招待が作成される
- [ ] 招待の「取消」ボタン → 招待が削除される
- [ ] ユーザーの「削除」ボタン → ユーザーが削除される

### /admin/rich-menu — リッチメニュー管理
- [ ] 未ログイン状態でアクセス → `/login` にリダイレクトされる
- [ ] ログイン後 → Bot 設定が表示される
- [ ] リッチメニュー一覧が取得できる
- [ ] リッチメニュー作成ができる
- [ ] 画像アップロードができる (1MB 以下の JPEG/PNG)
- [ ] デフォルトリッチメニュー設定/解除ができる
- [ ] リッチメニュー削除ができる

### /admin/requests — アクセスリクエスト管理
- [ ] 未ログイン状態でアクセス → `/login` にリダイレクトされる
- [ ] ログイン後 → pending リクエスト一覧が表示される
- [ ] 「承認」ボタン → ステータスが approved に変わる
- [ ] 「却下」ボタン → ステータスが declined に変わる

---

## 2. OAuth redirect/callback

### Google OAuth
- [ ] `/oauth/google/redirect?redirect_uri=https://...` → Google ログイン画面にリダイレクト
- [ ] Google 認証完了 → `/oauth/google/callback` → redirect_uri に `#token=xxx` でリダイレクト
- [ ] JWT の中身 (tenant_id, email, role) が正しい
- [ ] cookie `logi_auth_token` がセットされる

### LINE WORKS OAuth
- [ ] `/oauth/lineworks/redirect?address=user@domain&redirect_uri=https://...` → LINE WORKS ログイン画面にリダイレクト
- [ ] LINE WORKS 認証完了 → callback → redirect_uri に `#token=xxx` でリダイレクト

---

## 3. WOFF 認証

### GET /auth/woff-config
- [ ] `?domain=ohishi` → `{ "woffId": "..." }` が返る
- [ ] 存在しない domain → 404 エラー

### POST /auth/woff
- [ ] `{ "accessToken": "...", "domainId": "...", "redirectUri": "..." }` → JWT が返る
- [ ] 不正な accessToken → 401 エラー
- [ ] 不正な redirectUri → 400 エラー

---

## 4. パスワードログイン

### POST /auth/login (form data)
- [ ] 正しい organization_id + username + password → redirect_uri に `#token=xxx` でリダイレクト
- [ ] 不正なパスワード → `/login` にリダイレクト (error パラメータ付き)
- [ ] 存在しない username → `/login` にリダイレクト (error パラメータ付き)
- [ ] password_hash 未設定のユーザー → 認証失敗

---

## 5. 組織 API

### POST /api/my-orgs
- [ ] Bearer token 付き → `{ "organizations": [...] }` が返る
- [ ] token なし → 401

### POST /api/switch-org
- [ ] `{ "organizationId": "target-tenant-uuid" }` + Bearer token → 新しい JWT が返る
- [ ] ターゲットテナントにユーザーが存在しない → 403
- [ ] token なし → 401

---

## 6. Access Requests + Join

### GET /join/:slug — テナント参加ページ
- [ ] 有効な slug → テナント名 + OAuth ボタンが表示される
- [ ] 存在しない slug → 404 ページ

### GET /join/:slug/done — 参加完了ページ
- [ ] `#token=xxx` 付きでアクセス → JWT を読み取り表示

### POST /api/access-requests/create
- [ ] `{ "orgSlug": "..." }` + Bearer token → リクエスト作成 (201)

### POST /api/access-requests/list
- [ ] `{ "statusFilter": "pending" }` + Bearer token → 一覧取得
- [ ] admin 以外 → 403

### POST /api/access-requests/approve
- [ ] `{ "requestId": "uuid" }` + admin token → 承認成功

### POST /api/access-requests/decline
- [ ] `{ "requestId": "uuid" }` + admin token → 却下成功

---

## 7. Rich Menu API (直接呼び出し)

### POST /api/richmenu/list
- [ ] `{ "botConfigId": "uuid" }` + Bearer token → リッチメニュー一覧

### POST /api/richmenu/create
- [ ] `{ "botConfigId": "...", "richmenuName": "...", "size": {...}, "areas": [...] }` → 作成成功

### POST /api/richmenu/delete
- [ ] `{ "botConfigId": "...", "richmenuId": "..." }` → 削除成功

### POST /api/richmenu/image
- [ ] multipart form (botConfigId, richmenuId, image) → アップロード成功
- [ ] 1MB 超 → 400 エラー

### POST /api/richmenu/default/set
- [ ] `{ "botConfigId": "...", "richmenuId": "..." }` → デフォルト設定成功

### POST /api/richmenu/default/delete
- [ ] `{ "botConfigId": "..." }` → デフォルト解除成功

---

## 8. CORS

- [ ] OPTIONS `/auth/woff` → 200 + CORS ヘッダー
- [ ] OPTIONS `/api/my-orgs` → 200 + CORS ヘッダー
- [ ] OPTIONS `/api/switch-org` → 200 + CORS ヘッダー
