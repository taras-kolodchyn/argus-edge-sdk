mod handlers;
mod types;

use anyhow::{Context, Result};
use axum::{
    Router,
    routing::{get, post},
};
use rumqttc::{
    AsyncClient, Event, Incoming, MqttOptions, Outgoing, QoS, TlsConfiguration, Transport,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{fs, net::TcpListener};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

use crate::handlers::{AppState, health, telemetry};

fn read_env(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}

fn read_env_optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn ensure_trailing_slash(mut value: String) -> String {
    if !value.ends_with('/') {
        value.push('/');
    }
    value
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    eprintln!("[mock-sink] booting...");

    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // MQTT config
    let username = read_env("MQTT_USERNAME", "devuser");
    let password = read_env("MQTT_PASSWORD", "devpass");
    // Prefer MQTT_TELEMETRY_TOPIC for a concrete publish path in CI; fallback to subscription pattern
    let topics_csv = match std::env::var("MQTT_TELEMETRY_TOPIC") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => read_env("MQTT_TOPICS", "argus/devices/+"),
    };
    let default_host = read_env("MQTT_HOST", "mqtt");
    let default_port: u16 = read_env("MQTT_PORT", "8883").parse().unwrap_or(8883);
    let (host, port) = match std::env::var("MQTT_URL") {
        Ok(url) if !url.trim().is_empty() => match Url::parse(url.trim()) {
            Ok(u) => {
                let host = u.host_str().unwrap_or(&default_host).to_string();
                let port = u.port().unwrap_or(default_port);
                (host, port)
            }
            Err(e) => {
                tracing::warn!(
                    "MQTT_URL parse error: {e}; falling back to {}:{}",
                    default_host,
                    default_port
                );
                (default_host, default_port)
            }
        },
        _ => (default_host, default_port),
    };
    tracing::info!("mqtt -> {host}:{port} as mock-sink");

    let mut opts = MqttOptions::new("mock-sink", host, port);
    opts.set_credentials(username, password);
    opts.set_keep_alive(std::time::Duration::from_secs(30));

    let ca_path = read_env("MQTT_CA_PATH", "/certs/ca.crt");
    let ca = fs::read(&ca_path)
        .await
        .with_context(|| format!("failed to read MQTT_CA_PATH at {ca_path}"))?;
    let client_auth = match (
        read_env_optional("MQTT_CERT_PATH"),
        read_env_optional("MQTT_KEY_PATH"),
    ) {
        (Some(cert_path), Some(key_path)) => {
            let cert = fs::read(&cert_path)
                .await
                .with_context(|| format!("failed to read MQTT_CERT_PATH at {cert_path}"))?;
            let key = fs::read(&key_path)
                .await
                .with_context(|| format!("failed to read MQTT_KEY_PATH at {key_path}"))?;
            Some((cert, key))
        }
        (None, None) => None,
        _ => {
            tracing::warn!(
                "MQTT client certificate/key not fully specified; proceeding without client auth"
            );
            None
        }
    };
    let transport = Transport::tls_with_config(TlsConfiguration::Simple {
        ca,
        alpn: None,
        client_auth,
    });
    opts.set_transport(transport);
    let (client, mut eventloop) = AsyncClient::new(opts, 32);

    // Drive MQTT eventloop in background
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(inc)) => match inc {
                    Incoming::ConnAck(ack) => tracing::info!("mqtt connected: {ack:?}"),
                    Incoming::Publish(p) => {
                        let payload = String::from_utf8_lossy(&p.payload);
                        tracing::info!("{} <- {}", p.topic, payload);
                    }
                    Incoming::PubAck(ack) => tracing::info!("mqtt puback <- pkid={}", ack.pkid),
                    other => tracing::trace!("mqtt incoming: {other:?}"),
                },
                Ok(Event::Outgoing(out)) => match out {
                    Outgoing::Publish(pkid) => tracing::debug!("mqtt publish -> pkid={}", pkid),
                    other => tracing::trace!("mqtt outgoing: {other:?}"),
                },
                Err(e) => {
                    tracing::error!("mqtt eventloop error: {e}; retrying in 2s");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    });

    // Subscribe to topics so Compose smoke test can assert consumption
    for t in topics_csv.split(',') {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        match client.subscribe(t, QoS::AtLeastOnce).await {
            Ok(_) => tracing::info!("subscribed: {t}"),
            Err(e) => tracing::error!("subscribe error for '{t}': {e}"),
        }
    }

    // HTTP server with Axum
    let topic_prefix = ensure_trailing_slash(read_env("MQTT_TOPIC_PREFIX", "argus/devices/"));
    tracing::info!("mqtt topic prefix -> {topic_prefix}");
    let state = Arc::new(AppState {
        mqtt: client.clone(),
        topic_prefix,
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/telemetry", post(telemetry))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                let request_id = req
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-");
                tracing::info_span!(
                    "http",
                    %request_id,
                    method = %req.method(),
                    uri = %req.uri(),
                )
            }),
        )
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    // Bind/address
    let host = read_env("MOCK_SINK_HOST", "0.0.0.0");
    let port: u16 = read_env("MOCK_SINK_PORT", "8081").parse().unwrap_or(8081);
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("mock-sink http listening on http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
