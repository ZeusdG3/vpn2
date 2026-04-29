// src/edge.rs — Nodo Edge: recibe lecturas, promedio móvil, detecta anomalías, reenvía
use axum::{Router, extract::State, http::StatusCode, routing::post, Json};
use iot_pipeline::{EdgeReport, Heartbeat, SensorReading, now_ms};
use std::{
    collections::{HashMap, VecDeque},
    env,
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

const WINDOW_SIZE: usize = 10;
const ANOMALY_THRESHOLD: f64 = 38.0;

#[derive(Clone)]
struct EdgeState {
    edge_id: String,
    coordinator_url: String,
    // ventana móvil por sensor_id
    windows: Arc<Mutex<HashMap<String, VecDeque<f64>>>>,
    sample_counts: Arc<Mutex<HashMap<String, u64>>>,
    client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let edge_id = env::var("EDGE_ID").unwrap_or_else(|_| "edge-1".to_string());
    let coordinator_url =
        env::var("COORDINATOR_URL").unwrap_or_else(|_| "http://coordinator:8080".to_string());
    let listen_port = env::var("LISTEN_PORT").unwrap_or_else(|_| "9090".to_string());

    info!(
        "[{}] Edge iniciado → coordinador: {} | puerto: {}",
        edge_id, coordinator_url, listen_port
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Error creando cliente HTTP");

    let state = EdgeState {
        edge_id: edge_id.clone(),
        coordinator_url: coordinator_url.clone(),
        windows: Arc::new(Mutex::new(HashMap::new())),
        sample_counts: Arc::new(Mutex::new(HashMap::new())),
        client,
    };

    // Tarea de heartbeat al coordinador cada 3 segundos
    {
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                let hb = Heartbeat {
                    node_id: st.edge_id.clone(),
                    role: "edge".to_string(),
                    timestamp_ms: now_ms(),
                };
                let mut attempt = 0u32;
                loop {
                    attempt += 1;
                    match st
                        .client
                        .post(format!("{}/heartbeat", st.coordinator_url))
                        .json(&hb)
                        .send()
                        .await
                    {
                        Ok(r) if r.status().is_success() => {
                            info!("[{}] Heartbeat enviado al coordinador", st.edge_id);
                            break;
                        }
                        Ok(r) => {
                            warn!("[{}] Heartbeat: coordinador respondió {}", st.edge_id, r.status());
                            break;
                        }
                        Err(e) => {
                            warn!(
                                "[{}] Heartbeat fallo (intento {}): {}",
                                st.edge_id, attempt, e
                            );
                            if attempt >= 3 {
                                error!("[{}] Coordinador no alcanzable, reintentando en 5s", st.edge_id);
                                break;
                            }
                            let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                            tokio::time::sleep(backoff).await;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }

    let app = Router::new()
        .route("/reading", post(handle_reading))
        .route("/heartbeat", post(handle_heartbeat))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", listen_port);
    info!("[{}] Escuchando en {}", edge_id, addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_reading(
    State(state): State<EdgeState>,
    Json(reading): Json<SensorReading>,
) -> StatusCode {
    let recv_ts = now_ms();
    let latency_ms = recv_ts.saturating_sub(reading.timestamp_ms);

    info!(
        "[{}] Lectura de {} seq={} valor={:.2} latencia={}ms",
        state.edge_id, reading.sensor_id, reading.sequence, reading.value, latency_ms
    );

    // Actualizar ventana deslizante
    let (window_avg, sample_count) = {
        let mut windows = state.windows.lock().await;
        let mut counts = state.sample_counts.lock().await;

        let window = windows
            .entry(reading.sensor_id.clone())
            .or_insert_with(VecDeque::new);
        window.push_back(reading.value);
        if window.len() > WINDOW_SIZE {
            window.pop_front();
        }
        let avg = window.iter().sum::<f64>() / window.len() as f64;

        let count = counts.entry(reading.sensor_id.clone()).or_insert(0);
        *count += 1;

        (avg, *count)
    };

    let anomaly_detected = reading.value > ANOMALY_THRESHOLD;
    if anomaly_detected {
        warn!(
            "[{}] ANOMALÍA: sensor={} valor={:.2} umbral={}",
            state.edge_id, reading.sensor_id, reading.value, ANOMALY_THRESHOLD
        );
    }

    let report = EdgeReport {
        edge_id: state.edge_id.clone(),
        timestamp_ms: now_ms(),
        window_avg,
        anomaly_detected,
        sample_count,
        latency_ms,
        sensor_id: reading.sensor_id.clone(),
        last_sequence: reading.sequence,
    };

    // Enviar reporte al coordinador con reintentos y backoff exponencial
    let client = state.client.clone();
    let coord_url = state.coordinator_url.clone();
    let edge_id = state.edge_id.clone();

    tokio::spawn(async move {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match client
                .post(format!("{}/report", coord_url))
                .json(&report)
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => {
                    info!("[{}] Reporte enviado al coordinador (intento {})", edge_id, attempt);
                    break;
                }
                Ok(r) => {
                    warn!("[{}] Coordinador respondió {} en intento {}", edge_id, r.status(), attempt);
                    break;
                }
                Err(e) => {
                    warn!("[{}] Error enviando reporte (intento {}): {}", edge_id, attempt, e);
                    if attempt >= 5 {
                        error!("[{}] Reporte descartado tras 5 intentos", edge_id);
                        break;
                    }
                    let backoff = Duration::from_millis(200 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    });

    StatusCode::OK
}

async fn handle_heartbeat(
    State(state): State<EdgeState>,
    Json(hb): Json<Heartbeat>,
) -> StatusCode {
    info!(
        "[{}] Heartbeat recibido de {} ({})",
        state.edge_id, hb.node_id, hb.role
    );
    StatusCode::OK
}
