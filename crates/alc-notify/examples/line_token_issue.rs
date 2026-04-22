//! LINE channel access token v2.1 発行を local から検証する CLI。
//!
//! 使い方:
//! ```bash
//! export LINE_CHANNEL_ID=1234567890
//! export LINE_KEY_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
//! export LINE_PRIVATE_KEY_PATH=/path/to/private.pem   # PKCS#1 or PKCS#8 PEM
//! cargo run -p alc-notify --example line_token_issue
//! ```
//!
//! 追加で `--push USER_ID "text"` を渡すと、発行したトークンで push まで試す。
//!
//! Exit code:
//!   0 = success
//!   1 = failure (エラー本文を stderr に出す)
//!
//! DB には触らない。秘密鍵は手元の PEM を使うので、
//! 本番 Secret Manager からコピーしてきて試すこと。

use alc_notify::clients::line::{LineClient, LineConfig};
use std::env;
use std::fs;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let channel_id = match env::var("LINE_CHANNEL_ID") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("LINE_CHANNEL_ID が設定されていません");
            return ExitCode::from(2);
        }
    };
    let key_id = match env::var("LINE_KEY_ID") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("LINE_KEY_ID が設定されていません");
            return ExitCode::from(2);
        }
    };
    let pk_path = match env::var("LINE_PRIVATE_KEY_PATH") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("LINE_PRIVATE_KEY_PATH が設定されていません");
            return ExitCode::from(2);
        }
    };
    let private_key = match fs::read_to_string(&pk_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("PEM 読み込み失敗 ({pk_path}): {e}");
            return ExitCode::from(2);
        }
    };

    let config = LineConfig {
        channel_id,
        channel_secret: env::var("LINE_CHANNEL_SECRET").unwrap_or_default(),
        key_id,
        private_key,
    };

    let client = LineClient::new();

    println!("→ LINE oauth2/v2.1/token を呼び出し中...");
    let token = match client.get_access_token(&config).await {
        Ok(t) => {
            println!(
                "✓ access_token 発行成功 (先頭 20 文字): {}...",
                &t[..t.len().min(20)]
            );
            t
        }
        Err(e) => {
            eprintln!("✗ token 発行失敗: {e}");
            return ExitCode::from(1);
        }
    };

    // オプション: --push USER_ID "text"
    let args: Vec<String> = env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--push") {
        let user_id = match args.get(pos + 1) {
            Some(v) => v.as_str(),
            None => {
                eprintln!("--push には USER_ID を続けて指定してください");
                return ExitCode::from(2);
            }
        };
        let text = args
            .get(pos + 2)
            .map(|s| s.as_str())
            .unwrap_or("test from line_token_issue");
        println!("→ push_text to {user_id}");
        match client.push_text(&config, user_id, text).await {
            Ok(()) => println!("✓ push 成功"),
            Err(e) => {
                eprintln!("✗ push 失敗: {e}");
                return ExitCode::from(1);
            }
        }
    }

    let _ = token; // unused if --push not given
    ExitCode::SUCCESS
}
