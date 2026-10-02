//! axum router assembly and server bootstrap.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::api::routes as r;
use crate::api::state::{AppState, SharedState};
use crate::ai::client::Ai;
use crate::auth::Jwt;
use crate::config::Config;
use crate::db::schema::CONTROL_SCHEMA;
use crate::db::tenant::TenantPools;

pub async fn serve() -> crate::error::Result<()> {
    let cfg = Config::from_env();
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(16)
        .connect(&cfg.database_url)
        .await?;
    let admin_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&cfg.admin_database_url)
        .await?;
    sqlx::raw_sql(CONTROL_SCHEMA).execute(&pool).await?;

    let state: SharedState = Arc::new(AppState {
        ai: Ai::from_config(&cfg),
        jwt: Jwt::new(&cfg.jwt_secret),
        registry: crate::agents::tools::build_registry(),
        tenants: TenantPools::new(),
        cfg: cfg.clone(),
        pool,
        admin_pool,
    });

    crate::automations::scheduler::start(state.clone());

    let cors = CorsLayer::very_permissive();
    let app = api_router(state.clone())
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    tracing::info!(%addr, model = %state.ai.model, "mythforge listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// The reusable API surface. Every company is served by these same paths.
pub fn api_router(state: SharedState) -> Router {
    let api = Router::new()
        // meta
        .route("/catalog", get(r::catalog))
        // auth
        .route("/auth/signup", post(r::signup))
        .route("/auth/login", post(r::login))
        .route("/me", get(r::me))
        // workspace
        .route("/blueprint", get(r::blueprint))
        .route("/build", post(r::build))
        // generic data plane
        .route("/data/:module/:entity", get(r::list_records).post(r::create_record))
        .route(
            "/data/:module/:entity/:id",
            get(r::get_record)
                .patch(r::update_record)
                .delete(r::delete_record),
        )
        // mascots & agents
        .route("/mascots", get(r::list_mascots).post(r::create_mascot))
        .route("/agents", get(r::list_agents).post(r::create_agent))
        .route("/agents/compose", post(r::compose_agent))
        .route("/agents/:slug/run", post(r::run_agent))
        // automations
        .route("/automations", get(r::list_automations).post(r::create_automation))
        .route("/automations/compose", post(r::compose_automation))
        .route("/automations/:id", delete(r::delete_automation))
        .route("/automations/:id/run", post(r::run_automation_now))
        // assistant chat
        .route("/chat", post(r::chat))
        // google
        .route("/google/connect", get(r::google_start))
        .route("/google/callback", get(r::google_callback))
        .route("/google/status", get(r::google_status));

    Router::new()
        .route("/healthz", get(r::healthz))
        .nest("/api/v1", api.with_state(state))
}
