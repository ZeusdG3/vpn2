#!/bin/bash
# =============================================================
# verify.sh — Verifica conectividad, contenedores y métricas
# Uso: bash scripts/verify.sh [COORDINATOR_IP]
# =============================================================

COORDINATOR_IP="${1:-10.10.10.1}"
echo "======================================================"
echo " Verificación del Sistema IoT Pipeline — Proyecto 2"
echo " Coordinador: $COORDINATOR_IP"
echo "======================================================"

ok()   { echo "  [OK]  $1"; }
warn() { echo "  [!!]  $1"; }
fail() { echo "  [XX]  $1"; }

echo ""
echo "--- 1. Conectividad ZeroTier ---"
if ping -c 3 -W 2 "$COORDINATOR_IP" > /dev/null 2>&1; then
    ok "Ping al coordinador ($COORDINATOR_IP) exitoso"
else
    fail "No hay ping al coordinador. Verificar ZeroTier (zerotier-cli listnetworks)"
fi

echo ""
echo "--- 2. Estado de ZeroTier ---"
if command -v zerotier-cli &>/dev/null; then
    zerotier-cli listnetworks
    zerotier-cli listpeers | head -20
else
    warn "zerotier-cli no encontrado en este host"
fi

echo ""
echo "--- 3. Contenedores Docker en ejecución ---"
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "--- 4. API del Coordinador ---"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 \
    "http://${COORDINATOR_IP}:8080/status" 2>/dev/null)
if [ "$STATUS" = "200" ]; then
    ok "Coordinador responde en /status"
    echo ""
    echo "     Respuesta /status:"
    curl -s "http://${COORDINATOR_IP}:8080/status" | python3 -m json.tool 2>/dev/null || \
        curl -s "http://${COORDINATOR_IP}:8080/status"
else
    fail "Coordinador no responde en :8080 (código HTTP: $STATUS)"
fi

echo ""
echo "--- 5. Métricas del Coordinador ---"
METRICS=$(curl -s --connect-timeout 5 "http://${COORDINATOR_IP}:8080/metrics" 2>/dev/null)
if [ -n "$METRICS" ]; then
    ok "Endpoint /metrics accesible"
    echo ""
    echo "$METRICS"
else
    fail "No se pudo acceder a /metrics"
fi

echo ""
echo "--- 6. Reglas tc netem activas ---"
for iface in eth0 ztXXXXXXXX; do
    if ip link show "$iface" &>/dev/null; then
        echo "  Interfaz $iface:"
        tc qdisc show dev "$iface" 2>/dev/null || echo "    Sin reglas"
    fi
done

echo ""
echo "--- 7. Logs recientes de contenedores ---"
for name in iot_coordinator iot_edge_local iot_edge_peer1 iot_edge_peer2; do
    if docker ps -q -f name="$name" | grep -q .; then
        echo ""
        echo "  >> $name (últimas 5 líneas):"
        docker logs --tail=5 "$name" 2>&1 | sed 's/^/     /'
    fi
done

echo ""
echo "======================================================"
echo " Verificación completa"
echo "======================================================"
