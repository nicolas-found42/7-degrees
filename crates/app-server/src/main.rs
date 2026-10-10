//! Binary entry point. The library holds the app; this binary serves it.
#[tokio::main]
async fn main() {
    app_server::main().await;
}
