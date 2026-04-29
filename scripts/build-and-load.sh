#!/bin/bash
# =============================================================
# build-and-load.sh — Construye la imagen Docker y la carga en k3s
# Uso: bash scripts/build-and-load.sh
# Ejecutar desde la raíz del proyecto
# =============================================================

set -e

IMAGE_NAME="iot-pipeline"
IMAGE_TAG="latest"
TAR_FILE="/tmp/iot-pipeline.tar"

echo "======================================================"
echo " Build y carga de imagen para k3s"
echo "======================================================"

# 1. Construir imagen Docker normal
echo ""
echo "--- Paso 1: Construyendo imagen Docker ---"
docker build -t "${IMAGE_NAME}:${IMAGE_TAG}" -f docker/Dockerfile .
echo "Imagen construida: ${IMAGE_NAME}:${IMAGE_TAG}"

# 2. Exportar imagen a tar
echo ""
echo "--- Paso 2: Exportando imagen a tar ---"
docker save "${IMAGE_NAME}:${IMAGE_TAG}" -o "$TAR_FILE"
echo "Exportada en: $TAR_FILE"

# 3. Importar en k3s (containerd)
echo ""
echo "--- Paso 3: Importando imagen en k3s/containerd ---"
sudo k3s ctr images import "$TAR_FILE"
echo "Imagen importada en k3s"

# 4. Verificar que está disponible
echo ""
echo "--- Paso 4: Verificando imagen en k3s ---"
sudo k3s ctr images list | grep "$IMAGE_NAME"

echo ""
echo "======================================================"
echo " Imagen lista para usar en Kubernetes"
echo " Ahora aplica los manifiestos:"
echo "   kubectl apply -f k8s/"
echo "======================================================"
