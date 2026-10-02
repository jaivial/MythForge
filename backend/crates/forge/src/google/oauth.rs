//! Google OAuth2 (authorization-code) for Calendar + Gmail.

use base64::Engine;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};

/// Scopes requested on connect. Calendar + Gmail (send) cover the automations.
pub const SCOPES: &[&str] = &[
    "openid",
    "email",
    "profile",
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/gmail.send",
];

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

impl GoogleConfig {
    pub fn from_cfg(cfg: &Config) -> Result<Self> {
        if cfg.google_client_id.is_empty() || cfg.google_client_secret.is_empty() {
            return Err(Error::BadRequest(
                "google integration is not configured (GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET)".into(),
            ));
        }
        Ok(Self {
            client_id: cfg.google_client_id.clone(),
            client_secret: cfg.google_client_secret.clone(),
            redirect_url: cfg.google_redirect_url.clone(),
        })
    }
}

/// URL the browser is sent to. `state` carries the company id signed into the
/// session token; it comes back unchanged and is verified on callback.
pub fn authorize_url(cfg: &GoogleConfig, state: &str) -> String {
    format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
        urlencoding::encode(&cfg.client_id),
        urlencoding::encode(&cfg.redirect_url),
        urlencoding::encode(&SCOPES.join(" ")),
        urlencoding::encode(state),
    )
}

/// Exchange `code` for tokens and store them for the user+company.
pub async fn exchange_and_store(
    cfg: &GoogleConfig,
    http: &reqwest::Client,
    control: &PgPool,
    code: &str,
    user_id: Uuid,
    company_id: Uuid,
) -> Result<Value> {
    let resp = http
        .post(TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", cfg.redirect_url.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| Error::Internal(format!("google token exchange: {e}")))?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(Error::BadRequest(format!(
            "google rejected the code: {}",
            body.get("error_description").and_then(|e| e.as_str()).unwrap_or("unknown")
        )));
    }
    let access = body["access_token"].as_str().unwrap_or("").to_string();
    if access.is_empty() {
        return Err(Error::BadRequest("google returned no access token".into()));
    }
    let refresh = body["refresh_token"].as_str().map(String::from);
    let expires_in = body["expires_in"].as_i64().unwrap_or(3600);
    let scope = body["scope"].as_str().map(String::from);
    let email = userinfo_email(http, &access).await.ok();

    sqlx::query(
        r#"INSERT INTO google_token (id, user_id, company_id, access_token, refresh_token, expires_at, scope, email)
           VALUES ($1,$2,$3,$4,$5,now() + make_interval(secs => $6),$7,$8)
           ON CONFLICT (user_id, company_id) DO UPDATE SET
             access_token = EXCLUDED.access_token,
             refresh_token = COALESCE(EXCLUDED.refresh_token, google_token.refresh_token),
             expires_at = EXCLUDED.expires_at,
             scope = EXCLUDED.scope, email = EXCLUDED.email"#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(company_id)
    .bind(&access)
    .bind(&refresh)
    .bind(expires_in)
    .bind(&scope)
    .bind(&email)
    .execute(control)
    .await?;

    Ok(json!({ "connected": true, "email": email, "scope": scope }))
}

async fn userinfo_email(http: &reqwest::Client, access_token: &str) -> Result<String> {
    let v: Value = http
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?
        .json()
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    Ok(v["email"].as_str().unwrap_or("").to_string())
}

/// A usable access token for the (user, company) pair, refreshing if needed.
/// NOTE: `control` is the control-plane pool; per-user tokens live there.
pub async fn token_for(_control: &PgPool, _user_id: Uuid) -> Result<Option<String>> {
    // Replaced by `token_for_company`, kept for API compatibility.
    Ok(None)
}

/// A usable access token for a user inside a company, refreshed when stale.
pub async fn token_for_company(
    cfg: &GoogleConfig,
    http: &reqwest::Client,
    control: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
) -> Result<Option<String>> {
    let row: Option<(String, Option<String>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT access_token, refresh_token, expires_at FROM google_token WHERE user_id = $1 AND company_id = $2",
        )
        .bind(user_id)
        .bind(company_id)
        .fetch_optional(control)
        .await?;
    let Some((access, refresh, expires_at)) = row else {
        return Ok(None);
    };
    let fresh = expires_at
        .map(|e| e > chrono::Utc::now() + chrono::Duration::minutes(2))
        .unwrap_or(false);
    if fresh {
        return Ok(Some(access));
    }
    let Some(refresh_token) = refresh else {
        return Ok(Some(access)); // best effort
    };
    let resp = http
        .post(TOKEN_URL)
        .form(&[
            ("refresh_token", refresh_token.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| Error::Internal(format!("google refresh: {e}")))?;
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let new_access = body["access_token"].as_str().unwrap_or("").to_string();
    if new_access.is_empty() {
        return Err(Error::BadRequest("google refresh returned no token".into()));
    }
    let expires_in = body["expires_in"].as_i64().unwrap_or(3600);
    sqlx::query("UPDATE google_token SET access_token = $1, expires_at = now() + make_interval(secs => $2) WHERE user_id = $3 AND company_id = $4")
        .bind(&new_access)
        .bind(expires_in)
        .bind(user_id)
        .bind(company_id)
        .execute(control)
        .await?;
    Ok(Some(new_access))
}

/// base64url without padding, for the Gmail raw format.
pub fn b64url(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[allow(dead_code)]
fn unused(_: Uuid, _: PgPool, _: impl FromStr) {}
