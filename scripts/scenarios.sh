#!/bin/bash
# =============================================================
# scenarios.sh — Ejecuta y documenta los 5 escenarios tc netem
# Uso: sudo bash scripts/scenarios.sh [IFACE] [COORDINATOR_IP]
# Ejemplo: sudo bash scripts/scenarios.sh eth0 10.10.10.1
# =============================================================

IFACE="${1:-eth0}"
COORDINATOR_IP="${2:-10.10.10.1}"
IPERF_PORT=5201
LOG_DIR="./output/escenarios"
mkdir -p "$LOG_DIR"

timestamp() { date '+%Y-%m-%d %H:%M:%S'; }

apply_netem() {
    local profile="$1"
    tc qdisc del dev "$IFACE" root 2>/dev/null || true
    case "$profile" in
        baseline)
            echo ">> Sin reglas tc (baseline)"
            ;;
        latencia_iot)
            tc qdisc add dev "$IFACE" root netem delay 80ms 20ms
            ;;
        perdida_paquetes)
            tc qdisc add dev "$IFACE" root netem loss 8%
            ;;
        enlace_limitado)
            tc qdisc add dev "$IFACE" root netem rate 512kbit delay 50ms
            ;;
    esac
    echo ">> tc qdisc show dev $IFACE:"
    tc qdisc show dev "$IFACE"
}

measure_iperf() {
    local label="$1"
    local logfile="$LOG_DIR/${label}_iperf.txt"
    echo ">> Medición iperf3 para escenario: $label"
    echo "Escenario: $label — $(timestamp)" > "$logfile"
    echo "--- Throughput TCP ---" >> "$logfile"
    iperf3 -c "$COORDINATOR_IP" -p "$IPERF_PORT" -t 10 -J >> "$logfile" 2>&1 || \
        echo "WARN: iperf3 falló (¿está corriendo iperf3 -s en el coordinador?)" | tee -a "$logfile"
    echo "" >> "$logfile"
    echo "--- Latencia ping ---" >> "$logfile"
    ping -c 20 "$COORDINATOR_IP" >> "$logfile" 2>&1
    echo "Guardado en $logfile"
}

run_scenario() {
    local name="$1"
    local label="$2"
    echo ""
    echo "======================================================"
    echo " ESCENARIO: $name"
    echo " $(timestamp)"
    echo "======================================================"

    apply_netem "$label"
    sleep 2
    measure_iperf "$label"

    echo ""
    echo ">> Escenario $name activo. Presiona ENTER para continuar al siguiente..."
    read -r
}

echo "=============================================="
echo " Script de Escenarios tc netem — Proyecto 2"
echo " Interfaz: $IFACE | Coordinador: $COORDINATOR_IP"
echo "=============================================="
echo ""
echo "PREREQUISITO: En el coordinador debe correr:"
echo "  iperf3 -s -p $IPERF_PORT &"
echo ""
echo "Presiona ENTER para comenzar..."
read -r

# ---- ESCENARIO 1: Baseline ----
run_scenario "Baseline (sin degradación)" "baseline"

# ---- ESCENARIO 2: Latencia IoT ----
run_scenario "Latencia IoT (80ms ±20ms)" "latencia_iot"

# ---- ESCENARIO 3: Pérdida de paquetes ----
run_scenario "Pérdida de paquetes (8%)" "perdida_paquetes"

# ---- ESCENARIO 4: Enlace limitado ----
run_scenario "Enlace limitado (512kbit + 50ms)" "enlace_limitado"

# ---- ESCENARIO 5: Falla de nodo edge ----
echo ""
echo "======================================================"
echo " ESCENARIO 5: Falla de nodo edge"
echo " $(timestamp)"
echo "======================================================"
echo ">> Limpiando reglas tc (vuelta a baseline para este escenario)"
tc qdisc del dev "$IFACE" root 2>/dev/null || true

FALLA_LOG="$LOG_DIR/falla_edge.txt"
echo "Escenario: Falla de edge — $(timestamp)" > "$FALLA_LOG"
echo "" >> "$FALLA_LOG"

echo ">> Contenedores edge en ejecución:"
docker ps --filter "name=iot_edge" --format "table {{.Names}}\t{{.Status}}" | tee -a "$FALLA_LOG"

echo ""
echo ">> ¿Qué contenedor edge quieres matar? (ej: iot_edge_peer1)"
read -r EDGE_CONTAINER

echo ">> Matando contenedor: $EDGE_CONTAINER — $(timestamp)" | tee -a "$FALLA_LOG"
docker stop "$EDGE_CONTAINER" >> "$FALLA_LOG" 2>&1

echo ">> Esperando 15 segundos para que el coordinador detecte la falla..."
sleep 15
echo ">> Verificando métricas del coordinador:"
curl -s "http://${COORDINATOR_IP}:8080/metrics" | tee -a "$FALLA_LOG"

echo ""
echo ">> Reiniciando el edge: $EDGE_CONTAINER"
docker start "$EDGE_CONTAINER" >> "$FALLA_LOG" 2>&1

echo ">> Esperando 15 segundos para verificar reconexión..."
sleep 15
echo ">> Métricas post-reconexión:"
curl -s "http://${COORDINATOR_IP}:8080/metrics" | tee -a "$FALLA_LOG"
echo ""
echo "Guardado en $FALLA_LOG"

# Limpiar reglas tc al final
echo ""
echo "======================================================"
echo " TODOS LOS ESCENARIOS COMPLETADOS"
echo " Resultados en: $LOG_DIR/"
echo "======================================================"
echo ">> Limpiando reglas tc..."
tc qdisc del dev "$IFACE" root 2>/dev/null || true
echo "Listo."
