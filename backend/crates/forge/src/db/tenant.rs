//! Per-company database provisioning and pooled access.
//!
//! MythForge keeps **one PostgreSQL database per company**. The control plane
//! (`DATABASE_URL`) stores who exists; each company's data lives in its own
//! database, created on demand with [`provision`] and reached through the
//! per-company pool returned by [`TenantPools::get`]. Nothing here needs a
//! restart: a new company means a new row + a new database, and the generic
//! data-plane router (`crate::forge::runtime`) serves every company from the
//! same code paths.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::Executor;

use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::error::{Error, Result};

/// Identifier characters allowed in a company database name. Everything else is
/// stripped by [`slugify`] before the identifier is ever used in SQL.
const ALLOWED_IDENT_CHARS: &str = "abcdefghijklmnopqrstuvwxyz0123456789";

/// Reduce a free-form company name to a safe, stable Postgres identifier suffix.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().to_lowercase().chars() {
        if ALLOWED_IDENT_CHARS.contains(ch) {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "company".to_string()
    } else {
        out.chars().take(40).collect()
    }
}

/// Provisioning result, also written to the control-plane row.
#[derive(Debug, Clone)]
pub struct TenantInfo {
    pub company_id: Uuid,
    pub database_name: String,
    pub created: bool,
}

/// Create the `co_<slug>` database for a company and apply the base schema.
///
/// Safe to call repeatedly: an existing database is only verified, never dropped.
pub async fn provision(
    admin_pool: &PgPool,
    company_id: Uuid,
    name_hint: &str,
) -> Result<TenantInfo> {
    let database_name = format!("co_{}_{}", slugify(name_hint), short_id(company_id));
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)",
    )
    .bind(&database_name)
    .fetch_one(admin_pool)
    .await
    .map_err(|e| Error::Internal(format!("checking tenant database: {e}")))?;

    if !exists {
        // CREATE DATABASE cannot be parameterised or run inside a transaction.
        let stmt = format!(r#"CREATE DATABASE "{}""#, database_name);
        admin_pool
            .execute(sqlx::query(&stmt))
            .await
            .map_err(|e| {
                Error::Internal(format!("creating tenant database {database_name}: {e}"))
            })?;
        info!(database = %database_name, "provisioned company database");
    }

    let pool = build_pool(&database_name).await?;
    if !exists {
        sqlx::raw_sql(TENANT_BASE_SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| {
                Error::Internal(format!("applying tenant base schema to {database_name}: {e}"))
            })?;
    }
    Ok(TenantInfo {
        company_id,
        database_name,
        created: !exists,
    })
}

/// Per-tenant connect options for the same host/user as `DATABASE_URL`.
fn tenant_connect_options(database_name: &str) -> Result<PgConnectOptions> {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://forge:forge@localhost:5432/forge".to_string());
    let opts: PgConnectOptions = dsn
        .parse()
        .map_err(|e| Error::Internal(format!("parsing DATABASE_URL: {e}")))?;
    Ok(opts.database(database_name))
}

async fn build_pool(database_name: &str) -> Result<PgPool> {
    let opts = tenant_connect_options(database_name)?;
    PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect_with(opts)
        .await
        .map_err(|e| Error::Internal(format!("connecting to tenant db {database_name}: {e}")))
}

/// Short, collision-resistant suffix for a company id (hex, 8 chars).
fn short_id(id: Uuid) -> String {
    let bytes = id.as_bytes();
    hex::encode(&bytes[..4])
}

/// Cache of per-company pools. Bounded by the number of companies; entries are
/// never evicted because a company's pool is cheap and always valid.
#[derive(Default)]
pub struct TenantPools {
    pools: RwLock<HashMap<String, Arc<PgPool>>>,
}

impl TenantPools {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pool for an existing tenant database.
    pub async fn get(&self, database_name: &str) -> Result<Arc<PgPool>> {
        if let Some(p) = self.pools.read().await.get(database_name) {
            return Ok(p.clone());
        }
        let pool = Arc::new(build_pool(database_name).await?);
        self.pools
            .write()
            .await
            .insert(database_name.to_string(), pool.clone());
        Ok(pool)
    }
}

/// Tables present in *every* company database on day zero. AI-generated
/// modules add their own tables through the migration runner.
pub const TENANT_BASE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS mf_module (
    id          uuid PRIMARY KEY,
    slug        text NOT NULL,
    name        text NOT NULL,
    kind        text NOT NULL DEFAULT 'custom',
    icon        text,
    position    integer NOT NULL DEFAULT 0,
    is_active   boolean NOT NULL DEFAULT true,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (slug)
);

CREATE TABLE IF NOT EXISTS mf_entity (
    id          uuid PRIMARY KEY,
    module_id   uuid NOT NULL REFERENCES mf_module(id) ON DELETE CASCADE,
    slug        text NOT NULL,
    name        text NOT NULL,
    description text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (module_id, slug)
);

CREATE TABLE IF NOT EXISTS mf_field (
    id           uuid PRIMARY KEY,
    entity_id    uuid NOT NULL REFERENCES mf_entity(id) ON DELETE CASCADE,
    slug         text NOT NULL,
    name         text NOT NULL,
    field_type   text NOT NULL,
    required     boolean NOT NULL DEFAULT false,
    is_ref       boolean NOT NULL DEFAULT false,
    ref_entity   text,
    choices      jsonb NOT NULL DEFAULT '[]'::jsonb,
    position     integer NOT NULL DEFAULT 0,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (entity_id, slug)
);

CREATE TABLE IF NOT EXISTS mf_view (
    id          uuid PRIMARY KEY,
    module_id   uuid NOT NULL REFERENCES mf_module(id) ON DELETE CASCADE,
    slug        text NOT NULL,
    name        text NOT NULL,
    kind        text NOT NULL DEFAULT 'table',
    config      jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (module_id, slug)
);

-- Generic record storage. Every generated module writes here, keyed by entity
-- slug, so no DDL is needed to store data and no restart is needed to add a
-- field. Structured columns exist for query/sort; `data` keeps the full record.
CREATE TABLE IF NOT EXISTS mf_record (
    id          uuid PRIMARY KEY,
    entity_slug text NOT NULL,
    module_slug text NOT NULL,
    data        jsonb NOT NULL DEFAULT '{}'::jsonb,
    search      text NOT NULL DEFAULT '',
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS mf_record_entity_idx ON mf_record (module_slug, entity_slug);
CREATE INDEX IF NOT EXISTS mf_record_updated_idx ON mf_record (updated_at DESC);
CREATE INDEX IF NOT EXISTS mf_record_search_idx ON mf_record USING gin (to_tsvector('simple', search));

CREATE TABLE IF NOT EXISTS mf_automation_run (
    id            uuid PRIMARY KEY,
    automation_id uuid NOT NULL,
    status        text NOT NULL DEFAULT 'ok',
    started_at    timestamptz NOT NULL DEFAULT now(),
    finished_at   timestamptz,
    detail        jsonb NOT NULL DEFAULT '{}'::jsonb
);
"#;
