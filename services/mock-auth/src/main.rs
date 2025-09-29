use anyhow::Result;
use axum::Router;
use mock_auth::build_router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    eprintln!("[mock-auth] booting...");
    tracing::info!("mock-auth starting up...");
    // Initialize tracing with a sensible default if RUST_LOG isn't set
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();
    let app: Router = build_router();

    // Bind host/port from env with sensible defaults. Prefer service-specific vars.
    let host = std::env::var("MOCK_AUTH_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("MOCK_AUTH_PORT")
        .or_else(|_| std::env::var("PORT")) // fallback for platforms that provide PORT
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("mock-auth listening on http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("mock-auth shutdown complete");
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigint = signal(SignalKind::interrupt()).expect("listen SIGINT");
    let mut sigterm = signal(SignalKind::terminate()).expect("listen SIGTERM");
    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
    tracing::info!("shutdown signal received");
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("shutdown signal received");
}
