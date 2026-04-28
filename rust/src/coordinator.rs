// src/coordinator.rs — Coordinador central: agrega datos, métricas, detección de fallos
mod messages;

use axum::{Router, extract::State, http::StatusCode, routing::get, routing::post, Json};
use messages::{CoordStatus, EdgeReport, Heartbeat, now_ms};
use std::{
    collections::HashMap,
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

const EDGE_TIMEOUT_SECS: u64 = 10;

#[derive(Clone)]
struct CoordState {
    start_time: Arc<Instant>,
    // edge_id -> último timestamp de heartbeat recibido
    edge_heartbeats: Arc<Mutex<HashMap<String, u64>>>,
    // métricas acumuladas
    total_readings: Arc<Mutex<u64>>,
    anomalies_total: Arc<Mutex<u64>>,
    anomalies_last_min: Arc<Mutex<u64>>,
    // latencias para percentiles (ventana de 60s)
    latency_window: Arc<Mutex<Vec<(u64, u64)>>>, // (timestamp_ms, latency_ms)
    // throughput: lecturas por segundo en ventana
    readings_window: Arc<Mutex<Vec<u64>>>, // timestamps de recepciones
    // uptime por edge
    edge_first_seen: Arc<Mutex<HashMap<String, u64>>>,
    // últimas secuencias por sensor (para detectar pérdidas)
    last_sequences: Arc<Mutex<HashMap<String, u64>>>,
    messages_lost: Arc<Mutex<u64>>,
}

impl CoordState {
    fn new() -> Self {
        CoordState {
            start_time: Arc::new(Instant::now()),
            edge_heartbeats: Arc::new(Mutex::new(HashMap::new())),
            total_readings: Arc::new(Mutex::new(0)),
            anomalies_total: Arc::new(Mutex::new(0)),
            anomalies_last_min: Arc::new(Mutex::new(0)),
            latency_window: Arc::new(Mutex::new(Vec::new())),
            readings_window: Arc::new(Mutex::new(Vec::new())),
            edge_first_seen: Arc::new(Mutex::new(HashMap::new())),
            last_sequences: Arc::new(Mutex::new(HashMap::new())),
            messages_lost: Arc::new(Mutex::new(0)),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let listen_port = env::var("LISTEN_PORT").unwrap_or_else(|_| "8080".to_string());
    info!("[coordinator] Iniciando en puerto {}", listen_port);

    let state = CoordState::new();

    // Tarea: monitorear edges caídos cada 5 segundos
    {
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let now = now_ms();
                let heartbeats = st.edge_heartbeats.lock().await;
                for (edge_id, last_ts) in heartbeats.iter() {
                    let elapsed_secs = (now - last_ts) / 1000;
                    if elapsed_secs > EDGE_TIMEOUT_SECS {
                        error!(
                            "[coordinator] ALERTA: edge '{}' sin heartbeat hace {}s — posiblemente caído",
                            edge_id, elapsed_secs
                        );
                    }
                }
            }
        });
    }

    // Tarea: limpiar ventanas de métricas cada 60s y loggear resumen
    {
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let cutoff = now_ms() - 60_000;

                // Limpiar ventana de latencias
                {
                    let mut lw = st.latency_window.lock().await;
                    lw.retain(|(ts, _)| *ts > cutoff);
                }
                // Limpiar ventana de throughput
                {
                    let mut rw = st.readings_window.lock().await;
                    rw.retain(|ts| *ts > cutoff);
                }
                // Resetear anomalías del último minuto
                {
                    let mut alm = st.anomalies_last_min.lock().await;
                    *alm = 0;
                }

                // Log de resumen
                let total = *st.total_readings.lock().await;
                let lost = *st.messages_lost.lock().await;
                let anom = *st.anomalies_total.lock().await;
                info!(
                    "[coordinator] Resumen 60s: total_lecturas={} perdidas={} anomalías={}",
                    total, lost, anom
                );
            }
        });
    }

    let app = Router::new()
        .route("/report", post(handle_report))
        .route("/heartbeat", post(handle_heartbeat))
        .route("/status", get(handle_status))
        .route("/metrics", get(handle_metrics))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", listen_port);
    info!("[coordinator] Escuchando en {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_report(
    State(state): State<CoordState>,
    Json(report): Json<EdgeReport>,
) -> StatusCode {
    let recv_ts = now_ms();
    let e2e_latency = recv_ts.saturating_sub(report.timestamp_ms);

    info!(
        "[coordinator] Reporte de edge='{}' sensor='{}' avg={:.2} anomalía={} latencia_e2e={}ms",
        report.edge_id, report.sensor_id, report.window_avg, report.anomaly_detected, e2e_latency
    );

    // Detectar mensajes perdidos por gaps en secuencia
    {
        let mut seqs = state.last_sequences.lock().await;
        let key = format!("{}:{}", report.edge_id, report.sensor_id);
        if let Some(last) = seqs.get(&key) {
            let expected = last + 1;
            if report.last_sequence > expected {
                let lost = report.last_sequence - expected;
                warn!(
                    "[coordinator] Posible pérdida: sensor='{}' esperaba seq={} recibió seq={} ({}msgs perdidos)",
                    report.sensor_id, expected, report.last_sequence, lost
                );
                *state.messages_lost.lock().await += lost;
            }
        }
        seqs.insert(key, report.last_sequence);
    }

    // Actualizar métricas
    {
        let mut total = state.total_readings.lock().await;
        *total += 1;
    }
    if report.anomaly_detected {
        *state.anomalies_total.lock().await += 1;
        *state.anomalies_last_min.lock().await += 1;
    }
    {
        let mut lw = state.latency_window.lock().await;
        lw.push((recv_ts, e2e_latency));
    }
    {
        let mut rw = state.readings_window.lock().await;
        rw.push(recv_ts);
    }
    // Registrar primer contacto del edge
    {
        let mut first = state.edge_first_seen.lock().await;
        first.entry(report.edge_id.clone()).or_insert(recv_ts);
    }

    StatusCode::OK
}

async fn handle_heartbeat(
    State(state): State<CoordState>,
    Json(hb): Json<Heartbeat>,
) -> StatusCode {
    let mut hbs = state.edge_heartbeats.lock().await;
    let was_absent = !hbs.contains_key(&hb.node_id);
    hbs.insert(hb.node_id.clone(), hb.timestamp_ms);

    if was_absent {
        info!(
            "[coordinator] Nuevo nodo registrado: '{}' rol='{}'",
            hb.node_id, hb.role
        );
    } else {
        info!("[coordinator] Heartbeat de '{}' ({})", hb.node_id, hb.role);
    }
    StatusCode::OK
}

async fn handle_status(State(state): State<CoordState>) -> Json<CoordStatus> {
    let uptime_s = state.start_time.elapsed().as_secs();
    let now = now_ms();
    let cutoff_60s = now - 60_000;

    let hbs = state.edge_heartbeats.lock().await;
    let active_edges = hbs
        .values()
        .filter(|ts| (now - *ts) / 1000 <= EDGE_TIMEOUT_SECS)
        .count();

    let total_readings = *state.total_readings.lock().await;
    let anomalies_last_min = *state.anomalies_last_min.lock().await;
    let anomalies_total = *state.anomalies_total.lock().await;

    // Calcular percentiles de latencia en ventana 60s
    let lw = state.latency_window.lock().await;
    let recent_latencies: Vec<u64> = lw
        .iter()
        .filter(|(ts, _)| *ts > cutoff_60s)
        .map(|(_, lat)| *lat)
        .collect();

    let (p50, p99) = compute_percentiles(&recent_latencies);

    // Throughput en ventana 60s
    let rw = state.readings_window.lock().await;
    let recent_count = rw.iter().filter(|ts| **ts > cutoff_60s).count();
    let throughput = recent_count as f64 / 60.0;

    let anomaly_rate_pct = if total_readings > 0 {
        anomalies_total as f64 / total_readings as f64 * 100.0
    } else {
        0.0
    };

    Json(CoordStatus {
        active_edges,
        total_readings,
        anomalies_last_min,
        uptime_s,
        throughput_msg_per_sec: throughput,
        latency_p50_ms: p50,
        latency_p99_ms: p99,
        anomaly_rate_pct,
    })
}

async fn handle_metrics(State(state): State<CoordState>) -> String {
    let now = now_ms();
    let uptime_s = state.start_time.elapsed().as_secs();
    let cutoff_60s = now - 60_000;

    let hbs = state.edge_heartbeats.lock().await;
    let total = *state.total_readings.lock().await;
    let lost = *state.messages_lost.lock().await;
    let anom_total = *state.anomalies_total.lock().await;
    let anom_min = *state.anomalies_last_min.lock().await;

    let lw = state.latency_window.lock().await;
    let recent_latencies: Vec<u64> = lw
        .iter()
        .filter(|(ts, _)| *ts > cutoff_60s)
        .map(|(_, lat)| *lat)
        .collect();
    let (p50, p99) = compute_percentiles(&recent_latencies);

    let rw = state.readings_window.lock().await;
    let recent_count = rw.iter().filter(|ts| **ts > cutoff_60s).count();
    let throughput = recent_count as f64 / 60.0;

    let anomaly_rate = if total > 0 {
        anom_total as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let first_seen = state.edge_first_seen.lock().await;

    let mut out = String::new();
    out.push_str("# Métricas del Coordinador IoT Pipeline\n");
    out.push_str(&format!("uptime_s {}\n", uptime_s));
    out.push_str(&format!("total_readings {}\n", total));
    out.push_str(&format!("messages_lost {}\n", lost));
    out.push_str(&format!("anomalies_total {}\n", anom_total));
    out.push_str(&format!("anomalies_last_min {}\n", anom_min));
    out.push_str(&format!("anomaly_rate_pct {:.2}\n", anomaly_rate));
    out.push_str(&format!("throughput_msg_per_sec {:.3}\n", throughput));
    out.push_str(&format!("latency_p50_ms {:.1}\n", p50));
    out.push_str(&format!("latency_p99_ms {:.1}\n", p99));

    for (edge_id, last_ts) in hbs.iter() {
        let elapsed = (now - last_ts) / 1000;
        let status = if elapsed <= EDGE_TIMEOUT_SECS { "active" } else { "lost" };
        let uptime_edge = if let Some(first) = first_seen.get(edge_id) {
            (now - first) / 1000
        } else {
            0
        };
        out.push_str(&format!(
            "edge_status{{edge=\"{}\"}} {} # last_seen={}s_ago uptime={}s\n",
            edge_id, status, elapsed, uptime_edge
        ));
    }

    out
}

fn compute_percentiles(latencies: &[u64]) -> (f64, f64) {
    if latencies.is_empty() {
        return (0.0, 0.0);
    }
    let mut sorted = latencies.to_vec();
    sorted.sort_unstable();
    let p50_idx = (sorted.len() as f64 * 0.50) as usize;
    let p99_idx = ((sorted.len() as f64 * 0.99) as usize).min(sorted.len() - 1);
    (sorted[p50_idx] as f64, sorted[p99_idx] as f64)
}
