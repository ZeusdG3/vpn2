#!/bin/bash
# =============================================================
# get-zt-iface.sh — Encuentra la interfaz ZeroTier y genera
# los comandos exactos de k3s para este equipo
# Uso: bash scripts/get-zt-iface.sh [ROL] [TOKEN] [COORDINATOR_IP]
#   ROL: server | peer1 | peer2
# =============================================================

ROL="${1:-server}"
TOKEN="${2:-}"
COORD_IP="${3:-10.10.10.1}"

# Detectar interfaz ZeroTier automáticamente
ZT_IFACE=$(ip link show | grep -oP 'zt[a-z0-9]+' | head -1)

if [ -z "$ZT_IFACE" ]; then
    echo "ERROR: No se encontró interfaz ZeroTier."
    echo "Verifica que ZeroTier esté corriendo: sudo zerotier-cli status"
    exit 1
fi

# Detectar IP ZeroTier de este nodo
ZT_IP=$(ip addr show "$ZT_IFACE" 2>/dev/null | grep -oP '10\.\d+\.\d+\.\d+' | head -1)

echo "======================================================"
echo " Interfaz ZeroTier detectada: $ZT_IFACE"
echo " IP ZeroTier de este nodo:    ${ZT_IP:-no asignada aun}"
echo "======================================================"
echo ""

case "$ROL" in
    server)
        echo "--- Comando para INTEGRANTE 1 (server k3s) ---"
        echo ""
        echo "curl -sfL https://get.k3s.io | \\"
        echo "    INSTALL_K3S_EXEC=\"server \\"
        echo "        --node-ip=${ZT_IP:-10.10.10.1} \\"
        echo "        --advertise-address=${ZT_IP:-10.10.10.1} \\"
        echo "        --flannel-iface=${ZT_IFACE} \\"
        echo "        --disable=traefik\" \\"
        echo "    sh -"
        ;;
    peer1)
        if [ -z "$TOKEN" ]; then
            echo "ERROR: Necesitas el TOKEN del Integrante 1."
            echo "Uso: bash scripts/get-zt-iface.sh peer1 TOKEN_AQUI 10.10.10.1"
            exit 1
        fi
        echo "--- Comando para INTEGRANTE 2 (agent k3s peer1) ---"
        echo ""
        echo "curl -sfL https://get.k3s.io | \\"
        echo "    K3S_URL=\"https://${COORD_IP}:6443\" \\"
        echo "    K3S_TOKEN=\"${TOKEN}\" \\"
        echo "    INSTALL_K3S_EXEC=\"agent \\"
        echo "        --node-ip=${ZT_IP:-10.10.10.2} \\"
        echo "        --flannel-iface=${ZT_IFACE}\" \\"
        echo "    sh -"
        ;;
    peer2)
        if [ -z "$TOKEN" ]; then
            echo "ERROR: Necesitas el TOKEN del Integrante 1."
            echo "Uso: bash scripts/get-zt-iface.sh peer2 TOKEN_AQUI 10.10.10.1"
            exit 1
        fi
        echo "--- Comando para INTEGRANTE 3 (agent k3s peer2) ---"
        echo ""
        echo "curl -sfL https://get.k3s.io | \\"
        echo "    K3S_URL=\"https://${COORD_IP}:6443\" \\"
        echo "    K3S_TOKEN=\"${TOKEN}\" \\"
        echo "    INSTALL_K3S_EXEC=\"agent \\"
        echo "        --node-ip=${ZT_IP:-10.10.10.3} \\"
        echo "        --flannel-iface=${ZT_IFACE}\" \\"
        echo "    sh -"
        ;;
    *)
        echo "Uso: bash scripts/get-zt-iface.sh [server|peer1|peer2] [TOKEN] [COORDINATOR_IP]"
        exit 1
        ;;
esac

echo ""
echo "======================================================"
echo " Copia y pega el comando de arriba en tu terminal"
echo "======================================================"
