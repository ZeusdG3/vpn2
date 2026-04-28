use common::{EdgeReport, Heartbeat, CoordStatus, current_timestamp_ms};
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use anyhow::Result;
use log::{info, warn};

struct Metrics {
    total_readings: u64,
    total_anomalies: u64,
    lost_messages: u64,
    
    // Throughput tracking
    last_throughput_calc: u64,
    readings_since_last: u64,
    edge_readings_since_last: HashMap<u32, u64>,
    
    // Ventanas de 60s
    latencies_60s: VecDeque<(u64, u64)>, // (timestamp_recepcion, latencia_e2e)
    anomalies_60s: VecDeque<u64>,        // timestamp_recepcion
    
    // Nodos
    node_first_seen: HashMap<String, u64>,
    node_last_seen: HashMap<String, u64>,
    last_sequence_per_edge: HashMap<u32, u64>,
    start_time: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let data_addr = "127.0.0.1:9000";
    let heartbeat_addr = "127.0.0.1:9002";

    let state = Arc::new(Mutex::new(Metrics {
        total_readings: 0,
        total_anomalies: 0,
        lost_messages: 0,
        last_throughput_calc: current_timestamp_ms(),
        readings_since_last: 0,
        edge_readings_since_last: HashMap::new(),
        latencies_60s: VecDeque::new(),
        anomalies_60s: VecDeque::new(),
        node_first_seen: HashMap::new(),
        node_last_seen: HashMap::new(),
        last_sequence_per_edge: HashMap::new(),
        start_time: current_timestamp_ms(),
    }));

    let data_listener = TcpListener::bind(data_addr).await?;
    info!("Coordinator escuchando datos en {}", data_addr);
    let heartbeat_listener = TcpListener::bind(heartbeat_addr).await?;
    info!("Coordinator escuchando heartbeats en {}", heartbeat_addr);

    // Tarea: Procesar Heartbeats
    let state_hb = state.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = heartbeat_listener.accept().await {
            let state_cloned = state_hb.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stream);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(hb) = serde_json::from_str::<Heartbeat>(&line) {
                        let key = format!("{}_{}", hb.role, hb.node_id);
                        let mut st = state_cloned.lock().unwrap();
                        st.node_first_seen.entry(key.clone()).or_insert(hb.timestamp_ms);
                        st.node_last_seen.insert(key, hb.timestamp_ms);
                    }
                }
            });
        }
    });

    // Tarea: Exponer y calcular métricas periódicas (cada 5s)
    let state_metrics = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let mut st = state_metrics.lock().unwrap();
            let now = current_timestamp_ms();
            let cutoff_60s = now.saturating_sub(60_000);

            // Limpiar ventanas de 60s
            st.latencies_60s.retain(|&(ts, _)| ts > cutoff_60s);
            st.anomalies_60s.retain(|&ts| ts > cutoff_60s);

            // Calcular Throughput
            let elapsed_s = (now - st.last_throughput_calc) as f64 / 1000.0;
            let throughput_total = (st.readings_since_last as f64 / elapsed_s).max(0.0);
            
            // P50 y P99 Latencia E2E
            let mut lats: Vec<u64> = st.latencies_60s.iter().map(|&(_, l)| l).collect();
            lats.sort_unstable();
            let p50 = if lats.is_empty() { 0 } else { lats[lats.len() * 50 / 100] };
            let p99 = if lats.is_empty() { 0 } else { lats[lats.len() * 99 / 100] };

            // Tasa de anomalías
            let anomaly_rate = if st.total_readings == 0 { 0.0 } else {
                (st.total_anomalies as f64 / st.total_readings as f64) * 100.0
            };

            // Active Edges (vistos en los últimos 15s)
            let active_edges = st.node_last_seen.iter()
                .filter(|(k, &v)| k.starts_with("edge") && (now - v) < 15_000)
                .count() as u32;

            // Generar CoordStatus struct
            let coord_status = CoordStatus {
                active_edges,
                total_readings: st.total_readings,
                anomalies_last_min: st.anomalies_60s.len() as u32,
                uptime_s: (now - st.start_time) / 1000,
            };

            println!("\n--- MÉTRICAS DE RENDIMIENTO ---");
            println!("Throughput Total: {:.2} msg/s", throughput_total);
            for (edge_id, count) in &st.edge_readings_since_last {
                println!("  └ Throughput Edge {}: {:.2} msg/s", edge_id, (*count as f64 / elapsed_s));
            }
            println!("Latencia E2E (60s) -> P50: {} ms | P99: {} ms", p50, p99);
            println!("Tasa de Anomalías (Histórica): {:.2}%", anomaly_rate);
            println!("Mensajes Perdidos Estimados: {}", st.lost_messages);
            println!("CoordStatus JSON: {}", serde_json::to_string(&coord_status).unwrap());
            println!("Uptime por Nodo:");
            for (node, start_ts) in &st.node_first_seen {
                println!("  └ {}: {} s", node, (now - start_ts) / 1000);
            }
            println!("-------------------------------\n");

            // Reset de contadores de throughput
            st.last_throughput_calc = now;
            st.readings_since_last = 0;
            st.edge_readings_since_last.clear();
        }
    });

    // Tarea: Procesar Datos y actualizar estado
    while let Ok((stream, _)) = data_listener.accept().await {
        let state_data = state.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(report) = serde_json::from_str::<EdgeReport>(&line) {
                    let now = current_timestamp_ms();
                    let mut st = state_data.lock().unwrap();
                    
                    st.total_readings += 1;
                    st.readings_since_last += 1;
                    *st.edge_readings_since_last.entry(report.edge_id).or_insert(0) += 1;

                    // Latencia E2E (Recepción - Generación en Sensor)
                    let e2e_latency = now.saturating_sub(report.sensor_timestamp_ms);
                    st.latencies_60s.push_back((now, e2e_latency));

                    // Anomalías
                    if report.anomaly_detected {
                        st.total_anomalies += 1;
                        st.anomalies_60s.push_back(now);
                        warn!("Anomalía detectada en Edge {}, avg: {:.2}", report.edge_id, report.window_avg);
                    }

                    // Mensajes Perdidos (Gaps)
                    let expected_seq = st.last_sequence_per_edge.get(&report.edge_id).map(|s| s + 1).unwrap_or(report.sequence_number);
                    if report.sequence_number > expected_seq {
                        st.lost_messages += report.sequence_number - expected_seq;
                    }
                    st.last_sequence_per_edge.insert(report.edge_id, report.sequence_number);
                }
            }
        });
    }

    Ok(())
}