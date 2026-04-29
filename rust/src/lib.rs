// lib.rs — Estructuras compartidas del pipeline IoT
// Al ser una librería, todos los binarios pueden importarla con "use iot_pipeline::*"
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SensorReading {
    pub sensor_id: String,
    pub timestamp_ms: u64,
    pub value: f64,
    pub unit: String,
    pub sequence: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EdgeReport {
    pub edge_id: String,
    pub timestamp_ms: u64,
    pub window_avg: f64,
    pub anomaly_detected: bool,
    pub sample_count: u64,
    pub latency_ms: u64,
    pub sensor_id: String,
    pub last_sequence: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoordStatus {
    pub active_edges: usize,
    pub total_readings: u64,
    pub anomalies_last_min: u64,
    pub uptime_s: u64,
    pub throughput_msg_per_sec: f64,
    pub latency_p50_ms: f64,
    pub latency_p99_ms: f64,
    pub anomaly_rate_pct: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Heartbeat {
    pub node_id: String,
    pub role: String,
    pub timestamp_ms: u64,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
