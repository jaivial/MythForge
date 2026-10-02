//! MythForge API server binary.

#[tokio::main]
async fn main() {
    forge::api::server::serve().await.expect("mythforge server failed");
}
