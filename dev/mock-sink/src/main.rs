mod types;

use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

fn read_env(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}

#[tokio::main]
async fn main() {
    // Always show something on startup, even if tracing misbehaves
    eprintln!("[mock-sink] booting...");

    // Tracing setup with sane default
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Read env (with defaults for local dev)
    let mqtt_url_raw = std::env::var("MQTT_URL").unwrap_or_default();
    let mqtt_url = if mqtt_url_raw.trim().is_empty() {
        "mqtt://localhost:1883".to_string()
    } else {
        mqtt_url_raw.trim().to_string()
    };
    let username = read_env("MQTT_USERNAME", "devuser");
    let password = read_env("MQTT_PASSWORD", "devpass");
    // Prefer MQTT_TELEMETRY_TOPIC; fall back to legacy MQTT_TOPICS; final default for dev
    let topics_csv = match std::env::var("MQTT_TELEMETRY_TOPIC") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => read_env("MQTT_TOPICS", "argus/devices/+/telemetry"),
    };

    // Extra fallbacks for host/port in case URL parsing fails
    let host_fallback = read_env("MQTT_HOST", "localhost");
    let port_fallback: u16 = read_env("MQTT_PORT", "1883").parse().unwrap_or(1883);

    tracing::info!(
        "env: MQTT_URL='{}' MQTT_HOST='{}' MQTT_PORT={} MQTT_USERNAME='{}' topics='{}'",
        mqtt_url,
        host_fallback,
        port_fallback,
        username,
        topics_csv
    );

    // Parse URL or gracefully fallback to host/port
    let (host, port) = match Url::parse(&mqtt_url) {
        Ok(u) => {
            let host = u.host_str().unwrap_or(&host_fallback).to_string();
            let port = u.port().unwrap_or(port_fallback);
            (host, port)
        }
        Err(e) => {
            tracing::warn!("MQTT_URL parse error: {e}; falling back to {}:{}", host_fallback, port_fallback);
            (host_fallback, port_fallback)
        }
    };

    tracing::info!("connecting to {host}:{port} as mock-sink; topics: {topics_csv}");

    // MQTT options
    let mut opts = MqttOptions::new("mock-sink", host.clone(), port);
    opts.set_credentials(username, password);
    opts.set_keep_alive(std::time::Duration::from_secs(30));

    // Create client and eventloop
    let (client, mut eventloop) = AsyncClient::new(opts, 32);

    // Drive the eventloop in this task; do not exit on first error
    let mut got_connack = false;
    let mut subscribed_once = false;

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(inc)) => {
                match &inc {
                    Incoming::ConnAck(ack) => {
                        got_connack = true;
                        tracing::info!("connected to MQTT broker: {ack:?}");
                    }
                    Incoming::Publish(p) => {
                        let payload = String::from_utf8_lossy(&p.payload);
                        tracing::info!("{} <- {}", p.topic, payload);
                        if let Ok(tlm) = serde_json::from_slice::<types::Telemetry>(&p.payload) {
                            tracing::debug!("parsed telemetry: {:?}", tlm);
                        }
                    }
                    _ => {
                        tracing::trace!("incoming: {inc:?}");
                    }
                }

                // Subscribe once after connection established
                if got_connack && !subscribed_once {
                    for t in topics_csv.split(',') {
                        let t = t.trim();
                        if !t.is_empty() {
                            if let Err(e) = client.subscribe(t, QoS::AtLeastOnce).await {
                                tracing::error!("subscribe error for '{t}': {e}");
                            } else {
                                tracing::info!("subscribed: {t}");
                            }
                        }
                    }
                    subscribed_once = true;
                }
            }
            Ok(Event::Outgoing(out)) => {
                tracing::trace!("outgoing: {out:?}");
            }
            Err(e) => {
                // Keep the process running and visible; retry after a short delay
                tracing::error!("eventloop error: {e}; retrying in 2s");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }
}
