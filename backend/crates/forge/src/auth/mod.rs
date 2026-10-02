//! Authentication: argon2id password hashing + short-lived JWTs.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub company_id: Uuid,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Clone)]
pub struct Jwt {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Jwt {
    pub fn new(secret: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn issue(&self, user_id: Uuid, company_id: Uuid, role: &str) -> Result<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            company_id,
            role: role.to_string(),
            exp: (now + Duration::hours(24)).timestamp(),
            iat: now.timestamp(),
        };
        encode(&Header::default(), &claims, &self.encoding)
            .map_err(|e| Error::Internal(format!("signing token: {e}")))
    }

    pub fn verify(&self, token: &str) -> Result<Claims> {
        decode::<Claims>(token, &self.decoding, &Validation::default())
            .map(|d| d.claims)
            .map_err(|e| Error::Unauthorized(format!("invalid token: {e}")))
    }
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| Error::Internal(format!("hashing password: {e}")))
}

pub fn verify_password(hash: &str, password: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| Error::Internal(format!("bad hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let h = hash_password("s3cret!").unwrap();
        assert!(verify_password(&h, "s3cret!").unwrap());
        assert!(!verify_password(&h, "wrong").unwrap());
    }

    #[test]
    fn jwt_roundtrip() {
        let jwt = Jwt::new("test-secret-test-secret-test-secret");
        let t = jwt.issue(Uuid::new_v4(), Uuid::new_v4(), "owner").unwrap();
        let claims = jwt.verify(&t).unwrap();
        assert_eq!(claims.role, "owner");
    }

    #[test]
    fn jwt_rejects_garbage() {
        let jwt = Jwt::new("test-secret-test-secret-test-secret");
        assert!(jwt.verify("not-a-token").is_err());
    }
}
