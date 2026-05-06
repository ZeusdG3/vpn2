use clap::Parser;
use common::{SensorReading, EdgeReport, Heartbeat, current_timestamp_ms};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader, AsyncWriteExt};
use tokio::time::{self, Duration};
use tokio::sync::mpsc;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufReader as StdBufReader;
use std::path::Path;

// Imports para TLS
use rustls::{ClientConfig, RootCertStore, Certificate, PrivateKey};
use tokio_rustls::TlsConnector;
use rustls_pemfile::{certs, read_all};

#[derive(Parser)]
#[command(author, version, about = "Edge IoT", long_about = None)]
struct Args {
    // ID único del edge (ejempls: 100, 101, 102...)
    #[arg(short, long, default_value_t = 100)]
    id: u32,

    // Dirección donde escuchar sensores (Default: 127.0.0.1:9001)
    // Cambiar a direccion del coordinador con VPN "Pendiente"
    #[arg(short = 'l', long, default_value = "0.0.0.0:9001")]
    listen_addr: String,

    // Dirección del coordinador (envío de datos)
    #[arg(long, default_value = "10.165.168.1:9000")]
    coord_addr: String,

    // Dirección del heartbeat (coordinador)
    #[arg(long, default_value = "10.165.168.1:9002")]
    heartbeat_addr: String,

    // --- Argumentos para las Certificaciones ---

    #[arg(long, default_value = "certs/edge.crt")]
    cert_path: String,

    #[arg(long, default_value = "certs/edge.key")]
    key_path: String,
}

// --- FUNCIONES DE CARGA DE CERTIFICADOS ---
fn load_certs(path: &Path) -> Vec<Certificate> {
    let certfile = File::open(path).expect("No se pudo abrir cert");
    let mut reader = StdBufReader::new(certfile);
    certs(&mut reader).unwrap().into_iter().map(Certificate).collect()
}

fn load_keys(path: &Path) -> Vec<PrivateKey> {
    let keyfile = File::open(path).expect("No se pudo abrir key");
    let mut reader = StdBufReader::new(keyfile);
    let mut keys = Vec::new();
    for item in read_all(&mut reader).unwrap() {
        match item {
            rustls_pemfile::Item::RSAKey(key) => keys.push(PrivateKey(key)),
            rustls_pemfile::Item::PKCS8Key(key) => keys.push(PrivateKey(key)),
            rustls_pemfile::Item::ECKey(key) => keys.push(PrivateKey(key)),
            _ => {}
        }
    }
    keys
}

struct MovingAverage {
    window_size: usize,
    buffer: HashMap<u32, Vec<f64>>,
}
impl MovingAverage {
    fn new(window_size: usize) -> Self { Self { window_size, buffer: HashMap::new() } }
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
    
    // --- CONFIGURACIÓN mTLS ---
    let mut root_store = RootCertStore::empty();
    let ca_file = File::open("certs/ca.crt")?;
    let mut ca_reader = StdBufReader::new(ca_file);
    for cert in certs(&mut ca_reader).unwrap() {
        root_store.add(&Certificate(cert))?;
    }

    let edge_certs = load_certs(Path::new(&args.cert_path));
    let mut edge_keys = load_keys(Path::new(&args.key_path));

    let config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_client_auth_cert(edge_certs, edge_keys.remove(0))
        .expect("Error configurando certificados del Edge");

    let connector = TlsConnector::from(Arc::new(config));
    let dns_name = "localhost".try_into().unwrap(); // Debe coincidir con el CN del cert del coord

    // --- CONEXIÓN DE DATOS (con TLS) ---
    let stream = TcpStream::connect(&args.coord_addr).await?;
    let mut tls_stream = connector.connect(dns_name, stream).await?;
    log::info!("Edge {} conectado (mTLS) al coordinador en {}", args.id, args.coord_addr);

    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Hilo para enviar datos cifrados
    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if tls_stream.write_all(&data).await.is_err() { break; }
            if tls_stream.write_all(b"\n").await.is_err() { break; }
        }
    });

    // --- HEARTBEAT (mTLS) ---
    let hb_connector = connector.clone();
    let hb_addr = args.heartbeat_addr.clone();
    let hb_id = args.id;
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));
        let hb_dns: rustls::ServerName = "localhost".try_into().unwrap();
        loop {
            interval.tick().await;
            if let Ok(tcp) = TcpStream::connect(&hb_addr).await {
                if let Ok(mut tls) = hb_connector.connect(hb_dns.clone(), tcp).await {
                    let hb = Heartbeat {
                        role: "edge".to_string(),
                        node_id: hb_id,
                        timestamp_ms: current_timestamp_ms(),
                    };
                    // Convertimos a JSON y añadimos el salto de línea \n
                    if let Ok(mut payload) = serde_json::to_vec(&hb) {
                        payload.push(b'\n');
                        let _ = tls.write_all(&payload).await;
                        let _ = tls.flush().await;
                }
                }
            }
        }
    });

    // --- ESCUCHA DE SENSORES (TCP Plano) ---
    let listener = TcpListener::bind(&args.listen_addr).await?;
    let filter = Arc::new(Mutex::new(MovingAverage::new(3)));
    let threshold = 25.0;

    while let Ok((sensor_stream, _)) = listener.accept().await {
        let tx_clone = tx.clone();
        let filter_clone = filter.clone();
        let edge_id_clone = args.id;

        tokio::spawn(async move {
            let mut lines = TokioBufReader::new(sensor_stream).lines();
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
                    let _ = tx_clone.send(serde_json::to_vec(&report).unwrap());
                }
            }
        });
    }
    Ok(())
}