// src/sensor.rs — Generador de datos sintéticos IoT
mod messages;

use messages::{Heartbeat, SensorReading, now_ms};
use rand::Rng;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let sensor_id = env::var("SENSOR_ID").unwrap_or_else(|_| "sensor-1".to_string());
    let edge_url = env::var("EDGE_URL").unwrap_or_else(|_| "http://edge:9090".to_string());
    let interval_ms: u64 = env::var("PUBLISH_INTERVAL_MS")
        .unwrap_or_else(|_| "500".to_string())
        .parse()
        .unwrap_or(500);
    let unit = env::var("SENSOR_UNIT").unwrap_or_else(|_| "celsius".to_string());
    // Rango de valores: base ± ruido
    let base_value: f64 = env::var("BASE_VALUE")
        .unwrap_or_else(|_| "25.0".to_string())
        .parse()
        .unwrap_or(25.0);
    let anomaly_threshold: f64 = env::var("ANOMALY_THRESHOLD")
        .unwrap_or_else(|_| "38.0".to_string())
        .parse()
        .unwrap_or(38.0);

    info!(
        "[{}] Sensor iniciado → edge: {} | intervalo: {}ms | umbral anomalía: {}",
        sensor_id, edge_url, interval_ms, anomaly_threshold
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Error creando cliente HTTP");

    let mut rng = rand::thread_rng();
    let mut sequence: u64 = 0;
    let mut consecutive_errors = 0u32;

    loop {
        sleep(Duration::from_millis(interval_ms)).await;

        // Generar valor sintético con ruido gaussiano aproximado
        let noise: f64 = rng.gen_range(-3.0..3.0);
        // Simular pico ocasional de anomalía (~5% del tiempo)
        let spike = if rng.gen_bool(0.05) {
            rng.gen_range(15.0..20.0)
        } else {
            0.0
        };
        let value = base_value + noise + spike;
        sequence += 1;

        let reading = SensorReading {
            sensor_id: sensor_id.clone(),
            timestamp_ms: now_ms(),
            value,
            unit: unit.clone(),
            sequence,
        };

        // Enviar heartbeat cada 10 lecturas
        if sequence % 10 == 0 {
            let hb = Heartbeat {
                node_id: sensor_id.clone(),
                role: "sensor".to_string(),
                timestamp_ms: now_ms(),
            };
            let _ = client
                .post(format!("{}/heartbeat", edge_url))
                .json(&hb)
                .send()
                .await;
        }

        // Publicar lectura al edge con reintentos y backoff exponencial
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match client
                .post(format!("{}/reading", edge_url))
                .json(&reading)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if value > anomaly_threshold {
                        warn!(
                            "[{}] seq={} ANOMALÍA detectada: {:.2}{}",
                            sensor_id, sequence, value, unit
                        );
                    } else {
                        info!(
                            "[{}] seq={} valor={:.2}{} → OK",
                            sensor_id, sequence, value, unit
                        );
                    }
                    consecutive_errors = 0;
                    break;
                }
                Ok(resp) => {
                    warn!(
                        "[{}] Edge respondió error HTTP {}, intento {}",
                        sensor_id,
                        resp.status(),
                        attempt
                    );
                }
                Err(e) => {
                    warn!(
                        "[{}] Error conectando al edge (intento {}): {}",
                        sensor_id, attempt, e
                    );
                }
            }

            if attempt >= 3 {
                error!(
                    "[{}] seq={} descartada tras {} intentos",
                    sensor_id, sequence, attempt
                );
                consecutive_errors += 1;
                if consecutive_errors >= 10 {
                    error!("[{}] Demasiados errores consecutivos, esperando 10s...", sensor_id);
                    sleep(Duration::from_secs(10)).await;
                    consecutive_errors = 0;
                }
                break;
            }
            // Backoff exponencial: 200ms, 400ms, 800ms
            let backoff = Duration::from_millis(200 * 2u64.pow(attempt - 1));
            sleep(backoff).await;
        }
    }
}
