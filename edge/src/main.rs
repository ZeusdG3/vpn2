use clap::Parser;
use common::{SensorReading, EdgeReport, Heartbeat, current_timestamp_ms};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use tokio::time::{self, Duration};
use tokio::sync::mpsc;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(author, version, about = "Edge IoT", long_about = None)]
struct Args {
    // ID único del edge (ejempls: 100, 101, 102...)
    #[arg(short, long, default_value_t = 100)]
    id: u32,

    // Dirección donde escuchar sensores (Default: 127.0.0.1:9001)
    // Cambiar a direccion del coordinador con VPN "Pendiente"
    #[arg(short = 'l', long, default_value = "127.0.0.1:9001")]
    listen_addr: String,

    /// Dirección del coordinador (envío de datos)
    #[arg(long, default_value = "127.0.0.1:9000")]
    coord_addr: String,

    /// Dirección del heartbeat (coordinador)
    #[arg(long, default_value = "127.0.0.1:9002")]
    heartbeat_addr: String,
}

struct MovingAverage {
    window_size: usize,
    buffer: HashMap<u32, Vec<f64>>,
}

impl MovingAverage {
    fn new(window_size: usize) -> Self {
        Self { window_size, buffer: HashMap::new() }
    }

    fn filter(&mut self, sensor_id: u32, value: f64) -> (f64, usize) {
        let buf = self.buffer.entry(sensor_id).or_insert_with(Vec::new);
        buf.push(value);
        if buf.len() > self.window_size { buf.remove(0); }
        let avg = buf.iter().sum::<f64>() / buf.len() as f64;
        (avg, buf.len())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let edge_id = args.id;
    let listen_addr = args.listen_addr;
    let coord_addr = args.coord_addr;
    let heartbeat_addr = args.heartbeat_addr;

    // Conectar al coordinador (canal de datos)
    let coordinator_stream = TcpStream::connect(&coord_addr).await?;
    log::info!("Edge {} conectado al coordinador en {}", edge_id, coord_addr);

    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    let mut coordinator_writer = coordinator_stream;
    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if coordinator_writer.write_all(&data).await.is_err() { break; }
            if coordinator_writer.write_all(b"\n").await.is_err() { break; }
        }
    });

    // Heartbeat del edge
    let heartbeat_edge_id = edge_id;
    let heartbeat_addr_clone = heartbeat_addr.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Ok(mut conn) = TcpStream::connect(&heartbeat_addr_clone).await {
                let heartbeat = Heartbeat {
                    role: "edge".to_string(),
                    node_id: heartbeat_edge_id,
                    timestamp_ms: current_timestamp_ms(),
                };
                let _ = conn.write_all(&serde_json::to_vec(&heartbeat).unwrap()).await;
            }
        }
    });

    let listener = TcpListener::bind(&listen_addr).await?;
    log::info!("Edge {} escuchando sensores en {}", edge_id, listen_addr);

    let filter = Arc::new(Mutex::new(MovingAverage::new(3)));
    let threshold = 25.0; // umbral de anomalía

    while let Ok((sensor_stream, _)) = listener.accept().await {
        let tx_clone = tx.clone();
        let filter_clone = filter.clone();
        let edge_id_clone = edge_id;

        tokio::spawn(async move {
            let reader = BufReader::new(sensor_stream);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(reading) = serde_json::from_str::<SensorReading>(&line) {
                    let now = current_timestamp_ms();
                    let (window_avg, sample_count) = {
                        let mut f = filter_clone.lock().unwrap();
                        f.filter(reading.sensor_id, reading.value)
                    };
                    let report = EdgeReport {
                        edge_id: edge_id_clone,
                        window_avg,
                        anomaly_detected: window_avg > threshold,
                        sample_count: sample_count as u32,
                        latency_ms: now.saturating_sub(reading.timestamp_ms),
                        sequence_number: reading.sequence_number,
                        sensor_timestamp_ms: reading.timestamp_ms,
                    };
                    if tx_clone.send(serde_json::to_vec(&report).unwrap()).is_err() { break; }
                }
            }
        });
    }
    Ok(())
}