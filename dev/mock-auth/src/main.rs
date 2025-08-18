mod types;
mod handlers;

use axum::{routing::{get, post}, Router};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    eprintln!("[mock-auth] booting...");
    // Initialize tracing with a sensible default if RUST_LOG isn't set
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/auth/device/register", post(handlers::register))
        .route("/healthz", get(|| async { "ok" }));

    let addr: SocketAddr = ([0, 0, 0, 0], 8080).into();
    tracing::info!("mock-auth listening on http://{addr}");

    let listener = TcpListener::bind(addr).await.expect("bind listener");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
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
