mod types;

use rumqttc::{AsyncClient, Event, MqttOptions, QoS};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mqtt_url = std::env::var("MQTT_URL").unwrap_or_else(|_| "mqtt://localhost:1883".into());
    let username = std::env::var("MQTT_USERNAME").unwrap_or_else(|_| "devuser".into());
    let password = std::env::var("MQTT_PASSWORD").unwrap_or_else(|_| "devpass".into());
    let topics = std::env::var("MQTT_TOPICS").unwrap_or_else(|_| "gaia/devices/+/telemetry".into());

    let url = Url::parse(&mqtt_url).expect("MQTT_URL invalid");
    let host = url.host_str().unwrap().to_string();
    let port = url.port().unwrap_or(1883);

    let mut opts = MqttOptions::new("mock-sink", host, port);
    opts.set_credentials(username, password);
    opts.set_keep_alive(std::time::Duration::from_secs(30));

    let (client, mut eventloop) = AsyncClient::new(opts, 10);
    for t in topics.split(',') {
        let t = t.trim();
        client.subscribe(t, QoS::AtLeastOnce).await.unwrap();
        tracing::info!("subscribed: {t}");
    }

    while let Ok(ev) = eventloop.poll().await {
        if let Event::Incoming(inc) = ev {
            if let rumqttc::Packet::Publish(p) = inc {
                let payload = String::from_utf8_lossy(&p.payload);
                tracing::info!("{} <- {}", p.topic, payload);
                if let Ok(tlm) = serde_json::from_slice::<types::Telemetry>(&p.payload) {
                    tracing::debug!("parsed telemetry: {:?}", tlm);
                }
            }
        }
    }
}
