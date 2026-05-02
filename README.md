# rust-alc-api

アルコールチェッカーシステムのバックエンド API。GCP Cloud Run にデプロイ。

**別リポジトリで管理**

## 技術スタック

- Rust (Axum)
- GCP Cloud Run
- PostgreSQL + RLS (Row Level Security)
- GCP Cloud Storage (顔写真)

## 主な機能

- 測定結果の CRUD API
- 乗務員管理 API
- 顔写真アップロード (Cloud Storage)
- RLS によるマルチテナントデータ分離

## Snapshot hook (plan 整合性チェック)

`ippoan/ippoan-dev-plans` で管理している plan / feature flag の状態と本 repo の `manifests/production.snapshot.json` の整合性を pre-commit hook + CI で機械的に保証する。

### 初回 setup (clone 直後 1 度だけ)

```bash
NODE_AUTH_TOKEN=$(gh auth token) npm install
git config core.hooksPath .githooks
```

`.npmrc` は `@ippoan` scope を GHCR registry に向ける設定済み。`NODE_AUTH_TOKEN` には `read:packages` scope の PAT (Personal Access Token) が必要 — `gh auth token` が動けば一旦 OK、CI 用に長期運用する場合は専用 PAT を `~/.npmrc` に置く。

### 日常運用

| 状況 | 対処 |
|---|---|
| `if_flag!("name#sha")` を新規追加 | 先に `ippoan/ippoan-dev-plans` で `scope:rust-alc-api` ラベル付きの plan Issue を作る → `npm run snapshot` で snapshot 更新 → commit |
| pre-commit が drift で落ちた | `npm run snapshot && git add manifests/production.snapshot.json` |
| pre-commit が "stale sha" で落ちた | snapshot を再生成して新 sha に追従 (`npm run snapshot`)、もしくは code 側を新 sha に書き換え |
| clippy が遅い (~30s+) | `SKIP_CLIPPY=1 git commit ...` で一時 skip、CI で必ず走る |

### CI 防衛線

`.github/workflows/ci.yml` の `snapshot-check` job が `ippoan/ci-workflows/.github/workflows/snapshot-check.yml@main` を呼んで同じチェックを CI 上で再実行。`auto-merge` job の `needs` に含まれているので必須チェック扱い。
