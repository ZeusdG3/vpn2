#!/bin/bash
set -e

echo "=== IoT Pipeline Entrypoint ==="
echo "Rol: ${ROLE:-desconocido}"
echo "Worker/Node ID: ${NODE_ID:-N/A}"

# Aplicar reglas tc netem si está definido NETEM_PROFILE
if [ -n "$NETEM_PROFILE" ]; then
    echo "Aplicando perfil de red degradada: $NETEM_PROFILE"
    sleep 1  # Esperar que la interfaz de red esté lista

    IFACE="${NETEM_IFACE:-eth0}"
    # Limpiar reglas previas si existen
    tc qdisc del dev "$IFACE" root 2>/dev/null || true

    case "$NETEM_PROFILE" in
        baseline)
            echo "Baseline: sin degradación"
            # No se aplica ninguna regla
            ;;
        latencia_iot)
            echo "Perfil: Latencia IoT (80ms ±20ms)"
            tc qdisc add dev "$IFACE" root netem delay 80ms 20ms
            ;;
        perdida_paquetes)
            echo "Perfil: Pérdida de paquetes (8%)"
            tc qdisc add dev "$IFACE" root netem loss 8%
            ;;
        enlace_limitado)
            echo "Perfil: Enlace limitado (512kbit, delay 50ms)"
            tc qdisc add dev "$IFACE" root netem rate 512kbit delay 50ms
            ;;
        worker1)
            echo "Perfil worker1: 120ms ±30ms, loss 2%, 20Mbit"
            tc qdisc add dev "$IFACE" root netem delay 120ms 30ms loss 2% rate 20Mbit
            ;;
        worker2)
            echo "Perfil worker2: 60ms ±10ms, 50Mbit"
            tc qdisc add dev "$IFACE" root netem delay 60ms 10ms rate 50Mbit
            ;;
        worker3)
            echo "Perfil worker3: 30ms ±5ms, loss 1%, 30Mbit"
            tc qdisc add dev "$IFACE" root netem delay 30ms 5ms loss 1% rate 30Mbit
            ;;
        worker4)
            echo "Perfil worker4: 80ms ±40ms, 10Mbit"
            tc qdisc add dev "$IFACE" root netem delay 80ms 40ms rate 10Mbit
            ;;
        *)
            echo "Perfil desconocido: $NETEM_PROFILE, sin degradación"
            ;;
    esac

    echo "Reglas tc aplicadas en $IFACE:"
    tc qdisc show dev "$IFACE"
else
    echo "NETEM_PROFILE no definido, sin simulación de red"
fi

# Seleccionar y ejecutar el binario según el rol
ROLE="${ROLE:-coordinator}"
case "$ROLE" in
    coordinator)
        echo "Iniciando Coordinador..."
        exec /usr/src/app/coordinator
        ;;
    edge)
        echo "Iniciando Edge Node..."
        exec /usr/src/app/edge
        ;;
    sensor)
        echo "Iniciando Sensor..."
        exec /usr/src/app/sensor
        ;;
    *)
        echo "ERROR: Rol desconocido '$ROLE'. Usar: coordinator, edge, sensor"
        exit 1
        ;;
esac
