// sqlx::migrate!() マクロ用。migrations/ ディレクトリ内のファイル追加・変更を
// Cargo に通知し、proc macro を確実に再評価させる。
//
// これがないと新規 migration 追加時に Cargo が古いキャッシュの compile 済み
// バイナリを使い回し、同一 migration version に対して異なる checksum を持つ
// バイナリが混在して sqlx 側で VersionMismatch / duplicate key が起きる。
//
// 参考: https://docs.rs/sqlx/latest/sqlx/macro.migrate.html
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
