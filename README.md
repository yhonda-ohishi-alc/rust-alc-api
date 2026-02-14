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
