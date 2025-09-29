use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::RwLock};
use tokio_util::io::ReaderStream;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum JobStatus {
    Created,
    Dispatched,
    InProgress,
    Completed,
    Failed,
}

impl JobStatus {
    fn is_terminal(&self) -> bool {
        matches!(self, JobStatus::Completed | JobStatus::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OtaJob {
    id: Uuid,
    device_id: String,
    artifact: String,
    version: String,
    status: JobStatus,
    created_at: DateTime<Utc>,
    dispatched_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateJobRequest {
    device_id: String,
    artifact: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct OtaCommand {
    job_id: Uuid,
    artifact_url: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct DeviceStatusPayload {
    job_id: Uuid,
    status: String,
    message: Option<String>,
}

struct AppState {
    jobs: RwLock<HashMap<Uuid, OtaJob>>,
    artifact_dir: PathBuf,
    public_base: String,
    topic_prefix: String,
    mqtt: AsyncClient,
    auth: AuthContext,
}

#[derive(Clone)]
struct AuthContext {
    client: Client,
    validate_url: String,
    required_service: String,
}

impl AppState {
    fn ota_command_topic(&self, device_id: &str) -> String {
        format!("{}{}{}", self.topic_prefix, device_id, "/ota")
    }

    fn artifact_url(&self, artifact: &str) -> String {
        format!(
            "{}/ota/artifacts/{}",
            self.public_base.trim_end_matches('/'),
            artifact
        )
    }
}

type SharedState = Arc<AppState>;

fn read_env(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => default.to_string(),
    }
}

fn read_env_optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn ensure_safe_artifact_name(name: &str) -> Result<(), (StatusCode, String)> {
    if name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "artifact name is required".into()));
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return Err((
            StatusCode::BAD_REQUEST,
            "artifact name must be relative".into(),
        ));
    }
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "artifact name must not contain parent directories".into(),
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TokenValidateRequest<'a> {
    access_token: &'a str,
}

#[derive(Clone, Serialize, Deserialize)]
struct TokenValidateResponse {
    valid: bool,
    service: Option<String>,
}

async fn ensure_authorized(
    auth: &AuthContext,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, String)> {
    let auth_header = headers.get(header::AUTHORIZATION).ok_or((
        StatusCode::UNAUTHORIZED,
        "missing authorization header".into(),
    ))?;
    let auth_str = auth_header.to_str().map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "invalid authorization header".into(),
        )
    })?;
    let token = auth_str
        .strip_prefix("Bearer ")
        .or_else(|| auth_str.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or((StatusCode::UNAUTHORIZED, "invalid bearer token".into()))?;

    let response = auth
        .client
        .post(&auth.validate_url)
        .json(&TokenValidateRequest {
            access_token: token,
        })
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "auth validate request failed");
            (StatusCode::BAD_GATEWAY, "auth service unavailable".into())
        })?;

    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "auth validate returned non-success");
        return Err((StatusCode::UNAUTHORIZED, "token validation failed".into()));
    }

    let body = response
        .json::<TokenValidateResponse>()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to decode auth validate response");
            (StatusCode::BAD_GATEWAY, "invalid auth response".into())
        })?;

    if !body.valid {
        return Err((StatusCode::UNAUTHORIZED, "invalid token".into()));
    }

    if body.service.as_deref() != Some(auth.required_service.as_str()) {
        return Err((
            StatusCode::FORBIDDEN,
            "token not permitted for this service".into(),
        ));
    }

    Ok(())
}

async fn create_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<CreateJobRequest>,
) -> Result<Json<OtaJob>, (StatusCode, String)> {
    ensure_authorized(&state.auth, &headers).await?;
    ensure_safe_artifact_name(&payload.artifact)?;
    let artifact_path = state.artifact_dir.join(&payload.artifact);
    fs::metadata(&artifact_path)
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "artifact not found".into()))?;

    let job = OtaJob {
        id: Uuid::new_v4(),
        device_id: payload.device_id.trim().to_string(),
        artifact: payload.artifact,
        version: payload.version,
        status: JobStatus::Created,
        created_at: Utc::now(),
        dispatched_at: None,
        completed_at: None,
        message: None,
    };

    {
        let mut jobs = state.jobs.write().await;
        jobs.insert(job.id, job.clone());
    }

    tracing::info!(job_id = %job.id, device_id = %job.device_id, "OTA job created");
    Ok(Json(job))
}

async fn list_jobs(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<OtaJob>>, (StatusCode, String)> {
    ensure_authorized(&state.auth, &headers).await?;
    let jobs = state.jobs.read().await;
    Ok(Json(jobs.values().cloned().collect()))
}

async fn get_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<OtaJob>, (StatusCode, String)> {
    ensure_authorized(&state.auth, &headers).await?;
    let jobs = state.jobs.read().await;
    jobs.get(&id)
        .cloned()
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "job not found".into()))
}

async fn dispatch_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<OtaJob>, (StatusCode, String)> {
    ensure_authorized(&state.auth, &headers).await?;
    let (device_id, artifact, version, current_status) = {
        let jobs = state.jobs.read().await;
        let job = jobs
            .get(&id)
            .ok_or((StatusCode::NOT_FOUND, "job not found".into()))?;
        (
            job.device_id.clone(),
            job.artifact.clone(),
            job.version.clone(),
            job.status.clone(),
        )
    };

    if current_status.is_terminal() {
        return Err((
            StatusCode::CONFLICT,
            "job already finished; cannot dispatch".into(),
        ));
    }

    let command = OtaCommand {
        job_id: id,
        artifact_url: state.artifact_url(&artifact),
        version,
    };
    let payload = serde_json::to_vec(&command)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let topic = state.ota_command_topic(&device_id);

    state
        .mqtt
        .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("mqtt publish failed: {e}")))?;

    {
        let mut jobs = state.jobs.write().await;
        if let Some(job) = jobs.get_mut(&id) {
            job.status = JobStatus::Dispatched;
            job.dispatched_at = Some(Utc::now());
            job.message = Some("command dispatched".into());
        }
    }

    tracing::info!(job_id = %id, topic, "OTA command dispatched");

    let jobs = state.jobs.read().await;
    jobs.get(&id)
        .cloned()
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "job not found".into()))
}

async fn list_artifacts(
    State(state): State<SharedState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let mut entries = fs::read_dir(&state.artifact_dir)
        .await
        .map_err(internal_error)?;
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(internal_error)? {
        let file_type = entry.file_type().await.map_err(internal_error)?;
        if file_type.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }
    }
    files.sort();
    Ok(Json(files))
}

async fn get_artifact(
    State(state): State<SharedState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Response, (StatusCode, String)> {
    ensure_safe_artifact_name(&name)?;
    let full_path = state.artifact_dir.join(&name);
    let metadata = fs::metadata(&full_path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "artifact not found".into()))?;
    if !metadata.is_file() {
        return Err((StatusCode::NOT_FOUND, "artifact not found".into()));
    }

    let file = fs::File::open(&full_path).await.map_err(internal_error)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/octet-stream"),
    );
    Ok(response)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

fn internal_error<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn parse_status_topic<'a>(prefix: &str, topic: &'a str) -> Option<&'a str> {
    let rest = topic.strip_prefix(prefix)?;
    let mut parts = rest.split('/');
    let device = parts.next()?;
    if device.is_empty() {
        return None;
    }
    let seg1 = parts.next()?;
    let seg2 = parts.next()?;
    if seg1 == "ota" && seg2 == "status" && parts.next().is_none() {
        Some(device)
    } else {
        None
    }
}

async fn handle_status_message(state: &AppState, topic: &str, payload: &[u8]) {
    let Some(device_id) = parse_status_topic(&state.topic_prefix, topic) else {
        return;
    };

    let Ok(status_payload) = serde_json::from_slice::<DeviceStatusPayload>(payload) else {
        tracing::warn!(
            target = "mock-ota",
            topic,
            "failed to parse OTA status payload"
        );
        return;
    };

    let mut jobs = state.jobs.write().await;
    if let Some(job) = jobs.get_mut(&status_payload.job_id) {
        let new_status = match status_payload.status.as_str() {
            s if matches!(s, "in_progress" | "downloading" | "installing") => JobStatus::InProgress,
            s if matches!(s, "completed" | "success" | "ok") => JobStatus::Completed,
            s if matches!(s, "failed" | "error") => JobStatus::Failed,
            _ => JobStatus::InProgress,
        };
        if job.dispatched_at.is_none() {
            job.dispatched_at = Some(Utc::now());
        }
        if new_status.is_terminal() {
            job.completed_at = Some(Utc::now());
        }
        job.status = new_status;
        job.message = status_payload
            .message
            .or_else(|| Some(format!("reported by {device_id}")));
        tracing::info!(
            job_id = %job.id,
            device_id,
            status = ?job.status,
            "updated OTA job status"
        );
    } else {
        tracing::info!(job_id = %status_payload.job_id, device_id, "received status for unknown job");
    }
}

fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/ota/jobs", post(create_job).get(list_jobs))
        .route("/ota/jobs/:id", get(get_job))
        .route("/ota/jobs/:id/dispatch", post(dispatch_job))
        .route("/ota/artifacts", get(list_artifacts))
        .route("/ota/artifacts/:name", get(get_artifact))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                let request_id = req
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-");
                tracing::info_span!("http", %request_id, method = %req.method(), uri = %req.uri())
            }),
        )
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let host = read_env("MOCK_OTA_HOST", "0.0.0.0");
    let port: u16 = read_env("MOCK_OTA_PORT", "8090").parse().unwrap_or(8090);
    let public_base = read_env("MOCK_OTA_PUBLIC_BASE", "http://mock-ota:8090");
    let topic_prefix = ensure_trailing_slash(read_env("MQTT_TOPIC_PREFIX", "gaia/devices/"));
    let artifact_dir = PathBuf::from(read_env("MOCK_OTA_ARTIFACT_DIR", "/artifacts"));
    let validate_url = read_env(
        "MOCK_AUTH_VALIDATE_URL",
        "http://mock-auth:8080/auth/token/validate",
    );
    let required_service = read_env("MOCK_OTA_REQUIRED_SERVICE", "mock-ota");

    let mqtt_username = read_env("MQTT_USERNAME", "devuser");
    let mqtt_password = read_env("MQTT_PASSWORD", "devpass");
    let mqtt_default_host = read_env("MQTT_HOST", "mqtt");
    let mqtt_default_port: u16 = read_env("MQTT_PORT", "8883").parse().unwrap_or(8883);

    let (mqtt_host, mqtt_port) = match read_env_optional("MQTT_URL") {
        Some(url) if !url.is_empty() => match Url::parse(&url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or(&mqtt_default_host).to_string();
                let port = parsed.port().unwrap_or(mqtt_default_port);
                (host, port)
            }
            Err(e) => {
                tracing::warn!(
                    "MQTT_URL parse error: {e}; falling back to {}:{}",
                    mqtt_default_host,
                    mqtt_default_port
                );
                (mqtt_default_host, mqtt_default_port)
            }
        },
        _ => (mqtt_default_host, mqtt_default_port),
    };

    tracing::info!("mqtt -> {mqtt_host}:{mqtt_port} as mock-ota");

    let mut opts = MqttOptions::new("mock-ota", mqtt_host, mqtt_port);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_credentials(mqtt_username, mqtt_password);

    let ca_path = read_env("MQTT_CA_PATH", "/certs/ca.crt");
    let ca_bytes = fs::read(&ca_path)
        .await
        .with_context(|| format!("failed to read MQTT_CA_PATH at {ca_path}"))?;
    let client_auth = match (
        read_env_optional("MQTT_CERT_PATH"),
        read_env_optional("MQTT_KEY_PATH"),
    ) {
        (Some(cert), Some(key)) => {
            let cert_bytes = fs::read(&cert)
                .await
                .with_context(|| format!("failed to read MQTT_CERT_PATH at {cert}"))?;
            let key_bytes = fs::read(&key)
                .await
                .with_context(|| format!("failed to read MQTT_KEY_PATH at {key}"))?;
            Some((cert_bytes, key_bytes))
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
        ca: ca_bytes,
        alpn: None,
        client_auth,
    });
    opts.set_transport(transport);

    let (client, mut eventloop) = AsyncClient::new(opts, 32);
    let http_client = Client::builder().build()?;

    client
        .subscribe(topic_prefix.clone() + "+/ota/status", QoS::AtLeastOnce)
        .await
        .context("subscribe to OTA status topic")?;

    let state = Arc::new(AppState {
        jobs: RwLock::new(HashMap::new()),
        artifact_dir,
        public_base,
        topic_prefix,
        mqtt: client.clone(),
        auth: AuthContext {
            client: http_client,
            validate_url,
            required_service,
        },
    });

    let mqtt_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    let topic = publish.topic.clone();
                    handle_status_message(&mqtt_state, &topic, &publish.payload).await;
                }
                Ok(Event::Incoming(other)) => {
                    tracing::trace!("mqtt incoming: {other:?}");
                }
                Ok(Event::Outgoing(out)) => {
                    tracing::trace!("mqtt outgoing: {out:?}");
                }
                Err(e) => {
                    tracing::error!("mqtt eventloop error: {e}; retrying in 2s");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });

    let app = build_router(Arc::clone(&state));

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("mock-ota listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn ensure_trailing_slash(mut value: String) -> String {
    if !value.ends_with('/') {
        value.push('/');
    }
    value
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

#[cfg(test)]
#[path = "../tests/mod.rs"]
mod tests;
