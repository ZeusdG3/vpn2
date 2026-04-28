# Proyecto 2 — Pipeline IoT Distribuido en Rust
## Equipo Pyrrinoids — IL355 Programación de Sistemas Avanzados

---

## Arquitectura general

```
[Integrante 1 — 10.10.10.1]          [Integrante 2 — 10.10.10.2]
  coordinator (puerto 8080)    ←────    edge-peer1 (puerto 9090)
  edge-coord  (puerto 9090)              sensor-peer1-temp
  sensor-coord-1                         sensor-peer1-humidity
  sensor-coord-2

                                        [Integrante 3 — 10.10.10.3]
                               ←────    edge-peer2 (puerto 9090)
                                         sensor-peer2-temp
                                         sensor-peer2-vibration

```

- **Sensor**: genera una lectura cada 2 segundos (valor sinusoidal simulado). Envía `SensorReading` al Edge. Envía heartbeats cada 5s.
- **Edge**: recibe lecturas de múltiples sensores, calcula media móvil de ventana 3, detecta anomalías (umbral 25.0). Reenvía `EdgeReport` al coordinador. Envía heartbeats cada 5s.
- **Coordinador**: recibe reports y heartbeats, calcula métricas:
  - Throughput total y por edge (msg/s)
  - Latencia E2E (P50, P99 en ventana de 60s)
  - Tasa de anomalías histórica
  - Mensajes perdidos estimados (por gaps de secuencia)
  - Uptime por nodo (sensores y edges)
- Todo se muestra en la terminal cada 5 segundos formateado como tabla.

El sistema es **totalmente dinámico**: basta con lanzar nuevos sensores/edges con IDs y direcciones distintas, y el coordinador los mostrará automáticamente.

---

**Comunicación:** Los edges se conectan al coordinador vía HTTP REST sobre la red ZeroTier (IPs 10.10.10.x). Los sensores se conectan a su edge local dentro de la red Docker de cada host.

---

## Requisitos de Software

- **Rust** (versión 1.70 o superior)
- **Cargo** (incluido con Rust)
- **Git** (opcional, para clonar)

Verifica tu versión:
```bash

rustc --version
cargo --version

```
---
# Algoritmo Rust
---

## Estructura del Proyecto

El proyecto es un workspace de Cargo con tres bins:

```

ProyectoPipeline_IoT/
├── Cargo.toml            # workspace con tres miembros
├── common/
│   ├── Cargo.toml
│   └── src/lib.rs        # estructuras compartidas
├── sensor/
│   ├── Cargo.toml
│   └── src/main.rs
├── edge/
│   ├── Cargo.toml
│   └── src/main.rs
└── coordinator/
│   ├── Cargo.toml
│   └── src/main.rs

```

---

## Ejecución del Sistema

### 1. Iniciar el Coordinador (única instancia)

El coordinador escucha en:

* Datos: 127.0.0.1:9000 (Localhost)
* Heartbeats: 127.0.0.1:9002
  
```

cargo run --bin coordinator

```

Verás las métricas actualizadas cada 5 segundos.

---

### 2. Iniciar un Edge (pueden ser múltiples)

Cada edge necesita un ID único y un puerto de escucha para sensores.

Ejemplo: Edge ID=100 escuchando en puerto 9001

```

cargo run --bin edge -- --id 100 --listen-addr 127.0.0.1:9001

```

#### Argumentos disponibles:

`-i, --id` (obligatorio, ej 100)

`-l, --listen-addr` (dirección donde escucha sensores, por defecto 127.0.0.1:9001)

`--coord-addr` (dirección del coordinador para datos, por defecto 127.0.0.1:9000)

`--heartbeat-addr` (dirección del coordinador para heartbeats, por defecto 127.0.0.1:9002)

---

### 3. Iniciar uno o más Sensores

Cada sensor necesita un ID único y la dirección del edge al que se conecta.

Ejemplo: Sensor ID=1 conectándose al edge de puerto 9001

```

cargo run --bin sensor -- --id 1 --edge-addr 127.0.0.1:9001

```

#### Argumentos:

`-i, --id` (ID del sensor)

`-e, --edge-addr` (dirección del edge, ej 127.0.0.1:9001)

`--heartbeat-addr` (dirección del coordinador para heartbeats, por defecto 127.0.0.1:9002)

---

### Ejemplo con múltiples nodos en una sola máquina

```

Terminal	Comando
1	cargo run --bin coordinator
2	cargo run --bin edge -- --id 100 --listen-addr 127.0.0.1:9001
3	cargo run --bin edge -- --id 101 --listen-addr 127.0.0.1:9003
4	cargo run --bin sensor -- --id 1 --edge-addr 127.0.0.1:9001
5	cargo run --bin sensor -- --id 2 --edge-addr 127.0.0.1:9001
6	cargo run --bin sensor -- --id 3 --edge-addr 127.0.0.1:9003

```

---

### Ejecución en Múltiples Máquinas

Solo necesitaas cambiar las direcciones IP en los argumentos:

Máquina A (Coordinador):
`cargo run --bin coordinator (escucha en 0.0.0.0:9000 y 0.0.0.0:9002)`

Máquina B (Edge):
`cargo run --bin edge -- --id 100 --listen-addr 0.0.0.0:9001 --coord-addr <IP_A>:9000 --heartbeat-addr <IP_A>:9002`

Máquina C (Sensor):
`cargo run --bin sensor -- --id 1 --edge-addr <IP_B>:9001 --heartbeat-addr <IP_A>:9002`

---

### Personalización
* Cambiar umbral de anomalía: modifica `threshold` en `edge/src/main.rs`.
* Frecuencia de sensores: cambia `Duration::from_secs(2)` en sensor.
* Ventana de media móvil: cambia `MovingAverage::new(3)`.
* Puertos y direcciones: usa argumentos de línea de comandos (ver arriba).

---

# VPN

---

## PASO 0 — Instalar dependencias (TODOS los integrantes)

En cada máquina virtual Ubuntu 24.04:

```bash
# Actualizar sistema
sudo apt update && sudo apt upgrade -y

# Instalar Docker
sudo apt install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | sudo tee /etc/apt/sources.list.d/docker.list
sudo apt update
sudo apt install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Agregar tu usuario al grupo docker (para no usar sudo)
sudo usermod -aG docker $USER
newgrp docker

# Verificar Docker
docker run hello-world

# Instalar herramientas de red
sudo apt install -y iproute2 iputils-ping iperf3 curl

# Instalar ZeroTier (si no está instalado)
curl -s https://install.zerotier.com | sudo bash

# Verificar ZeroTier
sudo zerotier-cli status
```

---

## PASO 1 — Configurar ZeroTier (si no está hecho)

```bash
# Unirse a la red ZeroTier del equipo (reemplazar NETWORK_ID)
sudo zerotier-cli join <NETWORK_ID>

# Verificar que la red aparece
sudo zerotier-cli listnetworks

# Ver tu IP ZeroTier asignada
ip addr show | grep zt
# O: sudo zerotier-cli listnetworks (columna "IP/CIDR")
```

**En my.zerotier.com (el que creó la red):**
- Aprobar cada miembro en la sección "Members"
- Asignar IPs manualmente: 10.10.10.1, 10.10.10.2, 10.10.10.3

**Verificar conectividad entre máquinas:**
```bash
# Desde Integrante 2 o 3:
ping 10.10.10.1   # debe responder

# Desde Integrante 1:
ping 10.10.10.2
ping 10.10.10.3
```

---

## PASO 2 — Clonar/copiar el proyecto (TODOS)

```bash
# Opción A: clonar desde git (si tienen repo)
git clone <URL_REPO> proyecto2
cd proyecto2

# Opción B: copiar el zip y descomprimir
unzip proyecto2.zip
cd proyecto2
```

---

## PASO 3 — Abrir puertos en firewall (TODOS)

```bash
# En Ubuntu con ufw:
sudo ufw allow 22/tcp        # SSH (ya debería estar abierto)
sudo ufw allow 8080/tcp      # Coordinador (solo Integrante 1)
sudo ufw allow 9090/tcp      # Edge nodes (Integrantes 2 y 3)
sudo ufw allow 5201/tcp      # iperf3
sudo ufw allow 5201/udp      # iperf3 UDP
sudo ufw reload
sudo ufw status
```

---

## PASO 4 — INTEGRANTE 1: Levantar el Coordinador

```bash
cd proyecto2

# Construir imagen Docker
docker compose -f docker-compose.coordinator.yml build

# Levantar coordinador + edge local + sensores locales
docker compose -f docker-compose.coordinator.yml up -d

# Ver logs en tiempo real
docker compose -f docker-compose.coordinator.yml logs -f

# Verificar que responde:
curl http://localhost:8080/status
curl http://localhost:8080/metrics

# Levantar iperf3 server (para pruebas de escenarios)
iperf3 -s -p 5201 &
```

**Verificar desde otra máquina:**
```bash
# Desde Integrante 2 o 3:
curl http://10.10.10.1:8080/status
```

---

## PASO 5 — INTEGRANTE 2: Levantar Peer 1

```bash
cd proyecto2

# Construir imagen
docker compose -f docker-compose.peer1.yml build

# Levantar con la IP del coordinador
COORDINATOR_IP=10.10.10.1 docker compose -f docker-compose.peer1.yml up -d

# Ver logs
docker compose -f docker-compose.peer1.yml logs -f

# En los logs del coordinador (Integrante 1) deberías ver:
# [coordinator] Nuevo nodo registrado: 'edge-peer1' rol='edge'
```

---

## PASO 6 — INTEGRANTE 3: Levantar Peer 2

```bash
cd proyecto2

# Construir imagen
docker compose -f docker-compose.peer2.yml build

# Levantar con la IP del coordinador
COORDINATOR_IP=10.10.10.1 docker compose -f docker-compose.peer2.yml up -d

# Ver logs
docker compose -f docker-compose.peer2.yml logs -f
```

---

## PASO 7 — Verificar el sistema completo

```bash
# Desde cualquier máquina
bash scripts/verify.sh 10.10.10.1

# Ver status del coordinador
curl -s http://10.10.10.1:8080/status | python3 -m json.tool

# Ver métricas en texto plano
curl -s http://10.10.10.1:8080/metrics
```

Deberías ver en `/status`:
- `active_edges` > 0 (al menos 2 edges activos)
- `total_readings` incrementando
- `throughput_msg_per_sec` > 0

---

## PASO 8 — Ejecutar escenarios tc netem (DOCUMENTACIÓN OBLIGATORIA)

### En el coordinador (Integrante 1): levantar iperf3 server
```bash
iperf3 -s -p 5201
```

### En cada peer (Integrantes 2 y 3): ejecutar el script de escenarios
```bash
sudo bash scripts/scenarios.sh eth0 10.10.10.1
```

El script recorre automáticamente los 5 escenarios y guarda resultados en `output/escenarios/`.

### Comandos manuales por escenario (para referencia en el reporte):

```bash
# Ver interfaz ZeroTier
ip link show | grep zt

# ESCENARIO 1: Baseline
sudo tc qdisc del dev eth0 root 2>/dev/null; echo "Baseline activo"
sudo tc qdisc show dev eth0

# ESCENARIO 2: Latencia IoT
sudo tc qdisc del dev eth0 root 2>/dev/null
sudo tc qdisc add dev eth0 root netem delay 80ms 20ms
sudo tc qdisc show dev eth0
iperf3 -c 10.10.10.1 -p 5201 -t 10
ping -c 10 10.10.10.1

# ESCENARIO 3: Pérdida de paquetes
sudo tc qdisc del dev eth0 root 2>/dev/null
sudo tc qdisc add dev eth0 root netem loss 8%
sudo tc qdisc show dev eth0
iperf3 -c 10.10.10.1 -p 5201 -t 10
ping -c 20 10.10.10.1

# ESCENARIO 4: Enlace limitado
sudo tc qdisc del dev eth0 root 2>/dev/null
sudo tc qdisc add dev eth0 root netem rate 512kbit delay 50ms
sudo tc qdisc show dev eth0
iperf3 -c 10.10.10.1 -p 5201 -t 10

# ESCENARIO 5: Falla de edge
# Matar un contenedor edge
docker stop iot_edge_peer1
# Esperar 15s y verificar detección en logs del coordinador:
# [coordinator] ALERTA: edge 'edge-peer1' sin heartbeat hace Xs
curl http://10.10.10.1:8080/metrics
# Recuperar el edge
docker start iot_edge_peer1
# Verificar reconexión automática en logs

# Limpiar reglas al terminar
sudo tc qdisc del dev eth0 root 2>/dev/null
```

---

## PASO 9 — Evidencias a capturar (para el reporte)

```bash
# 1. Contenedores corriendo
docker ps

# 2. Red Docker interna
docker network inspect bridge

# 3. Estado de ZeroTier
sudo zerotier-cli listnetworks
sudo zerotier-cli listpeers

# 4. Reglas tc activas
sudo tc qdisc show dev eth0
sudo tc qdisc show dev <interfaz_zerotier>

# 5. Status y métricas del coordinador
curl -s http://10.10.10.1:8080/status | python3 -m json.tool
curl -s http://10.10.10.1:8080/metrics

# 6. Logs de comunicación
docker logs iot_coordinator --tail=50
docker logs iot_edge_peer1 --tail=20

# 7. Ping entre nodos
ping -c 5 10.10.10.2
ping -c 5 10.10.10.3
```

---

## Comandos de operación cotidiana

```bash
# Detener todo (en cada máquina)
docker compose -f docker-compose.coordinator.yml down   # Integrante 1
docker compose -f docker-compose.peer1.yml down         # Integrante 2
docker compose -f docker-compose.peer2.yml down         # Integrante 3

# Ver logs de un contenedor específico
docker logs -f iot_coordinator
docker logs -f iot_edge_peer1

# Reiniciar un servicio
docker restart iot_edge_peer1

# Cambiar escenario de red sin reiniciar el sistema
# (aplica tc directamente, el contenedor no necesita reiniciarse)
sudo tc qdisc del dev eth0 root 2>/dev/null
sudo tc qdisc add dev eth0 root netem delay 80ms 20ms

# Escalar workers (si se necesita más de 1 edge por peer)
# Cambiar EDGE_ID en el compose y levantar con otro nombre
```

---

## Justificación técnica de ZeroTier (para la sección obligatoria del reporte)

Los tres integrantes del equipo operan bajo CGNAT, lo que impide la conectividad entrante sin IP pública. Se evaluaron las siguientes alternativas:

1. **WireGuard hub propio**: Requiere al menos un nodo con IP pública estática. Ningún integrante dispone de una.
2. **WireGuard con VPS gratuito (Oracle Free Tier)**: Evaluado, pero el proceso de aprovisionamiento excedía el tiempo disponible para la entrega y no garantizaba disponibilidad inmediata.
3. **ZeroTier** (solución adoptada): Proporciona una red virtual overlay peer-to-peer cifrada con traversal automático de NAT, sin requerir infraestructura propia. El plano de control es administrado por ZeroTier Inc., pero el tráfico de datos es P2P y cifrado. Es open source (Business Source License).

**Compensaciones implementadas** (según tabla Nivel 3):
- tc netem documentado con 5 escenarios distintos, medición iperf3 antes/después en cada uno.
- Backoff exponencial y reintentos implementados en todos los nodos Rust.
- Reconexión automática de edges sin intervención manual.

---

## Variables de entorno de referencia

| Variable | Descripción | Default |
|---|---|---|
| `COORDINATOR_IP` | IP ZeroTier del coordinador | `10.10.10.1` |
| `NETEM_PROFILE` | Perfil de degradación de red | `baseline` |
| `RUST_LOG` | Nivel de logging | `info` |
| `PUBLISH_INTERVAL_MS` | Frecuencia de publicación del sensor | `500` |
| `ANOMALY_THRESHOLD` | Valor que dispara anomalía | según sensor |
| `BASE_VALUE` | Valor base del sensor | según sensor |
| `EDGE_ID` | Identificador del edge node | `edge-1` |
| `SENSOR_ID` | Identificador del sensor | `sensor-1` |

---

## Solución de problemas

**Edge no conecta al coordinador:**
```bash
# Verificar conectividad ZeroTier
ping 10.10.10.1
# Verificar que el coordinador escucha
curl http://10.10.10.1:8080/status
# Verificar firewall
sudo ufw status
sudo ufw allow 8080/tcp
```

**tc: No such file or directory:**
```bash
# El contenedor necesita NET_ADMIN (ya configurado en los compose)
# Si aplicas tc en el host:
sudo modprobe sch_netem
sudo tc qdisc show dev eth0
```

**Docker: permission denied:**
```bash
sudo usermod -aG docker $USER
newgrp docker
```

**ZeroTier sin asignar IP:**
```bash
# En my.zerotier.com: aprobar el miembro y asignar IP manualmente
sudo zerotier-cli listnetworks  # ver estado
```
