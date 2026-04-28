use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::User;

/// Access token の有効期限 (秒)
pub const ACCESS_TOKEN_EXPIRY_SECS: i64 = 3600; // 1時間
/// Refresh token の有効期限 (日)
pub const REFRESH_TOKEN_EXPIRY_DAYS: i64 = 30;

/// App JWT のクレーム
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppClaims {
    pub sub: Uuid,
    pub email: String,
    pub name: String,
    pub tenant_id: Uuid,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_slug: Option<String>,
    pub iat: i64,
    pub exp: i64,
}

/// JWT シークレットのラッパー
#[derive(Clone)]
pub struct JwtSecret(pub String);

/// Access token を発行
pub fn create_access_token(
    user: &User,
    secret: &JwtSecret,
    org_slug: Option<String>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = AppClaims {
        sub: user.id,
        email: user.email.clone(),
        name: user.name.clone(),
        tenant_id: user.tenant_id,
        role: user.role.clone(),
        org_slug,
        iat: now.timestamp(),
        exp: (now + Duration::seconds(ACCESS_TOKEN_EXPIRY_SECS)).timestamp(),
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.0.as_bytes()),
    )
}

/// Access token を検証してクレームを返す
pub fn verify_access_token(
    token: &str,
    secret: &JwtSecret,
) -> Result<AppClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let token_data = decode::<AppClaims>(
        token,
        &DecodingKey::from_secret(secret.0.as_bytes()),
        &validation,
    )?;

    Ok(token_data.claims)
}

/// 内部 API 用 JWT のクレーム (auth-worker → rust-alc-api 間の callback で使用)
///
/// 通常のユーザー JWT (`AppClaims`) と区別するため `aud = "alc-api-internal"` を強制する。
/// `JWT_SECRET` (HS256) は両者で共有しているため、aud で用途分離しないと
/// auth-worker が発行した内部 JWT がうっかり require_jwt 経路で受け入れられかねない。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InternalClaims {
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
}

/// 内部 API 用 JWT を発行 (主に rust-alc-api 内テストや CLI から auth-worker 用 JWT を生成する用途)
pub fn create_internal_token(
    secret: &JwtSecret,
    iss: &str,
    ttl_seconds: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = InternalClaims {
        iss: iss.to_string(),
        aud: INTERNAL_AUD.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::seconds(ttl_seconds)).timestamp(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.0.as_bytes()),
    )
}

/// 内部 API 用 JWT を検証
pub fn verify_internal_token(
    token: &str,
    secret: &JwtSecret,
) -> Result<InternalClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.set_audience(&[INTERNAL_AUD]);

    let token_data = decode::<InternalClaims>(
        token,
        &DecodingKey::from_secret(secret.0.as_bytes()),
        &validation,
    )?;

    Ok(token_data.claims)
}

pub const INTERNAL_AUD: &str = "alc-api-internal";

/// Refresh token を生成し、(raw_token, hash) を返す
pub fn create_refresh_token() -> (String, String) {
    let raw = format!("rt_{}", Uuid::new_v4().simple());
    let hash = hash_refresh_token(&raw);
    (raw, hash)
}

/// Refresh token の有効期限を返す
pub fn refresh_token_expires_at() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS)
}

/// Refresh token を SHA-256 でハッシュ化
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user() -> User {
        User {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            google_sub: Some("google-sub-123".to_string()),
            lineworks_id: None,
            line_user_id: None,
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
            role: "admin".to_string(),
            username: None,
            password_hash: None,
            refresh_token_hash: None,
            refresh_token_expires_at: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_create_and_verify_access_token() {
        test_group!("JWTトークン");
        test_case!("アクセストークンの生成と検証", {
            let user = test_user();
            let secret = JwtSecret("test-secret-key-256-bits-long!!!".to_string());

            let token = create_access_token(&user, &secret, Some("test-slug".to_string())).unwrap();
            let claims = verify_access_token(&token, &secret).unwrap();

            assert_eq!(claims.sub, user.id);
            assert_eq!(claims.email, user.email);
            assert_eq!(claims.tenant_id, user.tenant_id);
            assert_eq!(claims.role, "admin");
        });
    }

    #[test]
    fn test_verify_with_wrong_secret_fails() {
        test_group!("JWTトークン");
        test_case!("不正なシークレットで検証失敗", {
            let user = test_user();
            let secret = JwtSecret("correct-secret-key-256-bits!!!".to_string());
            let wrong_secret = JwtSecret("wrong-secret-key-256-bits!!!!!".to_string());

            let token = create_access_token(&user, &secret, Some("test-slug".to_string())).unwrap();
            assert!(verify_access_token(&token, &wrong_secret).is_err());
        });
    }

    #[test]
    fn test_create_and_verify_internal_token() {
        test_group!("内部JWT");
        test_case!(
            "内部トークンの生成と検証 (aud=alc-api-internal)",
            {
                let secret = JwtSecret("test-internal-secret-256-bits!!!".to_string());
                let token = create_internal_token(&secret, "auth-worker", 60).unwrap();
                let claims = verify_internal_token(&token, &secret).unwrap();
                assert_eq!(claims.iss, "auth-worker");
                assert_eq!(claims.aud, INTERNAL_AUD);
            }
        );
    }

    #[test]
    fn test_internal_token_rejects_user_token() {
        test_group!("内部JWT");
        test_case!(
            "ユーザートークンは内部検証で拒否される (aud 不一致)",
            {
                let user = test_user();
                let secret = JwtSecret("shared-secret-key-256-bits-long!".to_string());
                let user_token = create_access_token(&user, &secret, None).unwrap();
                assert!(verify_internal_token(&user_token, &secret).is_err());
            }
        );
    }

    #[test]
    fn test_user_token_rejects_internal_token() {
        test_group!("内部JWT");
        test_case!(
            "内部トークンはユーザー検証で拒否される (Claims 不一致)",
            {
                let secret = JwtSecret("shared-secret-key-256-bits-long!".to_string());
                let internal = create_internal_token(&secret, "auth-worker", 60).unwrap();
                // AppClaims に sub/email/tenant_id 等が無いので decode 失敗
                assert!(verify_access_token(&internal, &secret).is_err());
            }
        );
    }

    #[test]
    fn test_internal_token_wrong_aud_rejected() {
        test_group!("内部JWT");
        test_case!("間違った aud は拒否される", {
            use jsonwebtoken::{encode as jwt_encode, EncodingKey, Header};
            let secret = JwtSecret("test-secret-key-256-bits-long!!!".to_string());
            let now = Utc::now();
            let bad = InternalClaims {
                iss: "auth-worker".to_string(),
                aud: "wrong-aud".to_string(),
                iat: now.timestamp(),
                exp: (now + Duration::seconds(60)).timestamp(),
            };
            let token = jwt_encode(
                &Header::new(Algorithm::HS256),
                &bad,
                &EncodingKey::from_secret(secret.0.as_bytes()),
            )
            .unwrap();
            assert!(verify_internal_token(&token, &secret).is_err());
        });
    }

    #[test]
    fn test_internal_token_expired_rejected() {
        test_group!("内部JWT");
        test_case!("期限切れの内部トークンは拒否される", {
            use jsonwebtoken::{encode as jwt_encode, EncodingKey, Header};
            let secret = JwtSecret("test-secret-key-256-bits-long!!!".to_string());
            let now = Utc::now();
            let expired = InternalClaims {
                iss: "auth-worker".to_string(),
                aud: INTERNAL_AUD.to_string(),
                iat: (now - Duration::seconds(120)).timestamp(),
                exp: (now - Duration::seconds(60)).timestamp(),
            };
            let token = jwt_encode(
                &Header::new(Algorithm::HS256),
                &expired,
                &EncodingKey::from_secret(secret.0.as_bytes()),
            )
            .unwrap();
            assert!(verify_internal_token(&token, &secret).is_err());
        });
    }

    #[test]
    fn test_refresh_token_generation() {
        test_group!("JWTトークン");
        test_case!("リフレッシュトークン生成", {
            let (raw, hash) = create_refresh_token();
            assert!(raw.starts_with("rt_"));
            assert_eq!(hash, hash_refresh_token(&raw));
        });
    }

    #[test]
    fn test_refresh_token_hash_consistency() {
        test_group!("JWTトークン");
        test_case!("リフレッシュトークンハッシュの一貫性", {
            let token = "rt_test123";
            let hash1 = hash_refresh_token(token);
            let hash2 = hash_refresh_token(token);
            assert_eq!(hash1, hash2);
        });
    }
}
