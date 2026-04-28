use clap::Parser;
use common::{SensorReading, Heartbeat, current_timestamp_ms};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::time::{self, Duration};
use anyhow::Result;

#[derive(Parser)]
#[command(author, version, about = "Sensor IoT", long_about = None)]
struct Args {
    // ID único del sensor
    #[arg(short, long, default_value_t = 1)]
    id: u32,

    // Dirección del edge (Default la de localhost: 127.0.0.1:9001)
    // Pero ajustar cuando usemos IP del VPN
    #[arg(short = 'e', long, default_value = "127.0.0.1:9001")]
    edge_addr: String,

    // Dirección del heartbeat (coordinador)
    #[arg(long, default_value = "127.0.0.1:9002")]
    heartbeat_addr: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let sensor_id = args.id;
    let edge_addr = args.edge_addr;
    let heartbeat_addr = args.heartbeat_addr;

    log::info!("Sensor {} iniciado, conectando a edge {}", sensor_id, edge_addr);
    let mut edge_stream = TcpStream::connect(&edge_addr).await?;

    // Tarea de heartbeat (envía cada 5s)
    let heartbeat_sensor_id = sensor_id;
    let heartbeat_addr_clone = heartbeat_addr.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Ok(mut conn) = TcpStream::connect(&heartbeat_addr_clone).await {
                let heartbeat = Heartbeat {
                    role: "sensor".to_string(),
                    node_id: heartbeat_sensor_id,
                    timestamp_ms: current_timestamp_ms(),
                };
                let _ = conn.write_all(&serde_json::to_vec(&heartbeat).unwrap()).await;
            }
        }
    });

    let mut interval = time::interval(Duration::from_secs(2));
    let mut sequence_number = 0;
    loop {
        interval.tick().await;
        sequence_number += 1;
        let reading = SensorReading {
            sensor_id,
            timestamp_ms: current_timestamp_ms(),
            value: 20.0 + (sequence_number as f64).sin() * 5.0,
            unit: "Celsius".to_string(),
            sequence_number,
        };
        let data = serde_json::to_vec(&reading)?;
        edge_stream.write_all(&data).await?;
        edge_stream.write_all(b"\n").await?;
        edge_stream.flush().await?;
    }
}