//! axum router assembly.

use axum::routing::get;


pub async fn serve() -> crate::error::Result<()> {
    let app = axum::Router::new().route("/healthz", get(|| async { "ok" }));
    let cfg = crate::config::Config::from_env();
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
