//! Runtime configuration. Everything comes from the environment so the same
//! image runs in dev, CI and production (12-factor).

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    /// Postgres host/admin DSN used to CREATE DATABASE per company.
    pub admin_database_url: String,
    pub jwt_secret: String,
    pub anthropic_base_url: String,
    pub anthropic_api_key: String,
    pub anthropic_model: String,
    pub anthropic_max_tokens: u32,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_url: String,
    pub cors_origins: Vec<String>,
    pub app_url: String,
}

fn var(key: &str, default: &str) -> String {
    env::var(key).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = var("DATABASE_URL", "postgres://forge:forge@localhost:5432/forge");
        let admin_database_url = var(
            "ADMIN_DATABASE_URL",
            "postgres://forge:forge@localhost:5432/postgres",
        );
        let google_redirect = var(
            "GOOGLE_REDIRECT_URL",
            format!("{}/api/v1/google/callback", var("APP_URL", "http://localhost:5173")).as_str(),
        );
        Self {
            port: var("PORT", "8080").parse().unwrap_or(8080),
            database_url,
            admin_database_url,
            jwt_secret: var("JWT_SECRET", "dev-secret-change-me-please-0123456789"),
            anthropic_base_url: var("ANTHROPIC_BASE_URL", "https://api.anthropic.com"),
            anthropic_api_key: var("ANTHROPIC_API_KEY", ""),
            anthropic_model: var("ANTHROPIC_MODEL", "glm-5.3-flash"),
            anthropic_max_tokens: var("ANTHROPIC_MAX_TOKENS", "4096").parse().unwrap_or(4096),
            google_client_id: var("GOOGLE_CLIENT_ID", ""),
            google_client_secret: var("GOOGLE_CLIENT_SECRET", ""),
            google_redirect_url: google_redirect,
            cors_origins: var("CORS_ORIGINS", "http://localhost:5173,http://127.0.0.1:5173")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            app_url: var("APP_URL", "http://localhost:5173"),
        }
    }

    pub fn anthropic_configured(&self) -> bool {
        !self.anthropic_api_key.is_empty()
    }
}
