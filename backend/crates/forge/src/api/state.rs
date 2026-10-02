//! Shared application state.

use sqlx::PgPool;
use std::sync::Arc;

use crate::ai::agent::Registry;
use crate::ai::client::Ai;
use crate::auth::Jwt;
use crate::config::Config;
use crate::db::tenant::TenantPools;

pub struct AppState {
    pub cfg: Config,
    pub pool: PgPool,
    pub admin_pool: PgPool,
    pub tenants: TenantPools,
    pub ai: Ai,
    pub jwt: Jwt,
    pub registry: Registry,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    /// Pool for a company's own database.
    pub async fn tenant_pool(&self, database_name: &str) -> crate::error::Result<Arc<PgPool>> {
        self.tenants.get(database_name).await
    }
}
