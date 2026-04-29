# ============================================================
# Etapa 1: Compilación
# ============================================================
FROM rust:1.78-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copiar Cargo.toml para cachear dependencias
COPY rust/Cargo.toml ./

# Crear código dummy — permite cachear la descarga de dependencias
# lib.rs dummy con las estructuras mínimas para que compile
RUN mkdir -p src && \
    printf '%s\n' \
      'use serde::{Deserialize, Serialize};' \
      'pub fn now_ms() -> u64 { 0 }' \
      '#[derive(Serialize, Deserialize, Clone, Debug)]' \
      'pub struct SensorReading { pub sensor_id: String, pub timestamp_ms: u64, pub value: f64, pub unit: String, pub sequence: u64 }' \
      '#[derive(Serialize, Deserialize, Clone, Debug)]' \
      'pub struct EdgeReport { pub edge_id: String, pub timestamp_ms: u64, pub window_avg: f64, pub anomaly_detected: bool, pub sample_count: u64, pub latency_ms: u64, pub sensor_id: String, pub last_sequence: u64 }' \
      '#[derive(Serialize, Deserialize, Clone, Debug)]' \
      'pub struct CoordStatus { pub active_edges: usize, pub total_readings: u64, pub anomalies_last_min: u64, pub uptime_s: u64, pub throughput_msg_per_sec: f64, pub latency_p50_ms: f64, pub latency_p99_ms: f64, pub anomaly_rate_pct: f64 }' \
      '#[derive(Serialize, Deserialize, Clone, Debug)]' \
      'pub struct Heartbeat { pub node_id: String, pub role: String, pub timestamp_ms: u64 }' \
      > src/lib.rs && \
    echo 'fn main(){}' > src/coordinator.rs && \
    echo 'fn main(){}' > src/edge.rs && \
    echo 'fn main(){}' > src/sensor.rs && \
    cargo build --release && \
    rm -rf src

# Copiar código real y recompilar (rápido, dependencias ya cacheadas)
COPY rust/src ./src
RUN touch src/*.rs && cargo build --release

# ============================================================
# Etapa 2: Imagen final mínima
# ============================================================
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    iproute2 \
    iputils-ping \
    curl \
    iperf3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/target/release/coordinator .
COPY --from=builder /usr/src/app/target/release/edge .
COPY --from=builder /usr/src/app/target/release/sensor .
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
