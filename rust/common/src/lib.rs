use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_id: u32,
    pub timestamp_ms: u64,
    pub value: f64,
    pub unit: String,
    pub sequence_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeReport {
    pub edge_id: u32,
    pub window_avg: f64,
    pub anomaly_detected: bool,
    pub sample_count: u32,
    pub latency_ms: u64,
    pub sequence_number: u64,
    pub sensor_timestamp_ms: u64, // Se usara para el calculo de la latencia E2E
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordStatus {
    pub active_edges: u32,
    pub total_readings: u64,
    pub anomalies_last_min: u32,
    pub uptime_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub node_id: u32,
    pub role: String,
    pub timestamp_ms: u64,
}