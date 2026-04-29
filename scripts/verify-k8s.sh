#!/bin/bash
# =============================================================
# verify-k8s.sh — Verifica el estado del cluster k3s y el pipeline
# Uso: bash scripts/verify-k8s.sh [COORDINATOR_IP]
# =============================================================

COORDINATOR_IP="${1:-10.10.10.1}"

echo "======================================================"
echo " Verificación k3s + IoT Pipeline"
echo "======================================================"

ok()   { echo "  [OK]  $1"; }
warn() { echo "  [!!]  $1"; }
fail() { echo "  [XX]  $1"; }

echo ""
echo "--- 1. Estado del cluster k3s ---"
if kubectl get nodes 2>/dev/null; then
    ok "kubectl funciona correctamente"
else
    fail "kubectl no responde. ¿Está k3s corriendo? (sudo systemctl status k3s)"
fi

echo ""
echo "--- 2. Nodos del cluster ---"
kubectl get nodes -o wide 2>/dev/null || warn "No se pudo listar nodos"

echo ""
echo "--- 3. Pods en namespace iot-pipeline ---"
kubectl get pods -n iot-pipeline -o wide 2>/dev/null || warn "Namespace iot-pipeline no encontrado"

echo ""
echo "--- 4. Deployments ---"
kubectl get deployments -n iot-pipeline 2>/dev/null || warn "Sin deployments"

echo ""
echo "--- 5. Services ---"
kubectl get services -n iot-pipeline 2>/dev/null || warn "Sin services"

echo ""
echo "--- 6. Réplicas de edge nodes ---"
echo "  edge-peer1:"
kubectl get deployment edge-peer1 -n iot-pipeline -o jsonpath='{.status.readyReplicas}' 2>/dev/null && echo " réplicas listas"
echo "  edge-peer2:"
kubectl get deployment edge-peer2 -n iot-pipeline -o jsonpath='{.status.readyReplicas}' 2>/dev/null && echo " réplicas listas"

echo ""
echo "--- 7. Logs recientes de un pod edge ---"
EDGE_POD=$(kubectl get pods -n iot-pipeline -l app=edge --no-headers -o custom-columns=":metadata.name" 2>/dev/null | head -1)
if [ -n "$EDGE_POD" ]; then
    echo "  Pod: $EDGE_POD"
    kubectl logs -n iot-pipeline "$EDGE_POD" --tail=10 2>/dev/null
else
    warn "No hay pods edge disponibles aún"
fi

echo ""
echo "--- 8. Coordinator API (desde fuera del cluster) ---"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 \
    "http://${COORDINATOR_IP}:8080/status" 2>/dev/null)
if [ "$STATUS" = "200" ]; then
    ok "Coordinador responde"
    curl -s "http://${COORDINATOR_IP}:8080/status" | python3 -m json.tool 2>/dev/null
else
    fail "Coordinador no responde en :8080 (HTTP $STATUS)"
fi

echo ""
echo "--- 9. Métricas del coordinador ---"
curl -s --connect-timeout 5 "http://${COORDINATOR_IP}:8080/metrics" 2>/dev/null || \
    warn "No se pudo acceder a /metrics"

echo ""
echo "======================================================"
echo " Comandos útiles de kubectl:"
echo "   kubectl get pods -n iot-pipeline -w          # watch en tiempo real"
echo "   kubectl logs -n iot-pipeline <pod> -f        # logs de un pod"
echo "   kubectl describe pod -n iot-pipeline <pod>   # detalles de un pod"
echo "   kubectl rollout status deployment/edge-peer1 -n iot-pipeline"
echo "======================================================"
