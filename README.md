# Proyecto 2 — Pipeline IoT Distribuido en Rust + k3s
## Equipo Pyrrinoids — IL355 Programación de Sistemas Avanzados

---

## Arquitectura general

```
[Integrante 1 — 10.10.10.1]           [Integrante 2 — 10.10.10.2]
  Docker:                       <────   k3s cluster (server + agent):
    coordinator (puerto 8080)             Deployment: edge-peer1 (2 réplicas)
    edge-coord                            Deployment: sensors-peer1
    sensor-coord-1                        NodePort: 30090
    sensor-coord-2
                                         [Integrante 3 — 10.10.10.3]
                                <────   k3s cluster (agent):
                                          Deployment: edge-peer2 (2 réplicas)
                                          Deployment: sensors-peer2
                                          NodePort: 30091
```

Red ZeroTier: 10.10.10.0/24 — todos los nodos se ven entre si.
Nivel 3: ZeroTier usado por CGNAT. Compensaciones: tc netem + Kubernetes obligatorio.

---

## PASO 0 — Instalar dependencias (LOS 3 INTEGRANTES, en cada VM)

```bash
sudo apt update && sudo apt upgrade -y

# Docker
sudo apt install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | \
    sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
    https://download.docker.com/linux/ubuntu \
    $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
    sudo tee /etc/apt/sources.list.d/docker.list
sudo apt update
sudo apt install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

sudo usermod -aG docker $USER
newgrp docker

# Herramientas de red
sudo apt install -y iproute2 iputils-ping iperf3 curl net-tools

# ZeroTier (si no esta instalado)
curl -s https://install.zerotier.com | sudo bash
sudo zerotier-cli status
```

---

## PASO 1 — Verificar ZeroTier y abrir puertos (LOS 3 INTEGRANTES)

```bash
# Ver tu IP ZeroTier
ip addr show | grep -A2 "zt"

# Verificar conectividad (desde Integrantes 2 y 3)
ping -c 4 10.10.10.1

# Abrir todos los puertos necesarios
sudo ufw allow 22/tcp
sudo ufw allow 8080/tcp     # Coordinador
sudo ufw allow 9090/tcp     # Edge nodes
sudo ufw allow 5201/tcp     # iperf3
sudo ufw allow 5201/udp
sudo ufw allow 30090/tcp    # NodePort edge-peer1
sudo ufw allow 30091/tcp    # NodePort edge-peer2
sudo ufw allow 6443/tcp     # k3s API server
sudo ufw allow 8472/udp     # k3s Flannel VXLAN
sudo ufw allow 10250/tcp    # k3s kubelet
sudo ufw reload
sudo ufw status
```

---

## PASO 2 — Copiar el proyecto (LOS 3 INTEGRANTES)

```bash
unzip proyecto2.zip -d ~
cd ~/proyecto2
```

---

## INTEGRANTE 1 — COORDINADOR (10.10.10.1)

### Instalar k3s como SERVER (nodo master)

```bash
# OPCION FACIL: usar el script que detecta tu interfaz ZeroTier automaticamente
# y genera el comando exacto para ti
cd ~/proyecto2
bash scripts/get-zt-iface.sh server

# El script te mostrara el comando exacto. Copiarlo y ejecutarlo.
# Ejemplo de lo que genera:
# curl -sfL https://get.k3s.io | \
#     INSTALL_K3S_EXEC="server \
#         --node-ip=10.10.10.1 \
#         --advertise-address=10.10.10.1 \
#         --flannel-iface=ztabcd1234 \
#         --disable=traefik" \
#     sh -

# Verificar que arranco
sudo systemctl status k3s

# Configurar kubectl
mkdir -p ~/.kube
sudo cp /etc/rancher/k3s/k3s.yaml ~/.kube/config
sudo chown $USER:$USER ~/.kube/config

# Verificar nodo (debe aparecer Ready en ~30 segundos)
kubectl get nodes

# Obtener el token para que los otros integrantes se unan
# Copiar este token y enviarlo a los Integrantes 2 y 3
sudo cat /var/lib/rancher/k3s/server/node-token

# Compartir el kubeconfig con los otros integrantes
# Enviar el contenido de este archivo (cambiar la IP antes de enviarlo)
cat ~/.kube/config
```

### Levantar el Coordinador con Docker

```bash
cd ~/proyecto2

docker compose -f docker-compose.coordinator.yml build
docker compose -f docker-compose.coordinator.yml up -d

# Verificar que responde
curl http://localhost:8080/status
curl http://localhost:8080/metrics

# Levantar iperf3 para las pruebas de escenarios
iperf3 -s -p 5201 &
```

### Aplicar manifiestos de Kubernetes

```bash
cd ~/proyecto2

# Verificar que la IP del coordinador es correcta en el ConfigMap
cat k8s/01-configmap.yaml
# Si la IP ZeroTier no es 10.10.10.1, editar:
# nano k8s/01-configmap.yaml

# Aplicar todos los manifiestos
kubectl apply -f k8s/

# Verificar que se crearon
kubectl get all -n iot-pipeline
```

---

## INTEGRANTE 2 — PEER 1 (10.10.10.2)

### Instalar k3s como AGENT

Necesitas del Integrante 1: el TOKEN (lo obtiene con `sudo cat /var/lib/rancher/k3s/server/node-token`).

```bash
# OPCION FACIL: usar el script con tu TOKEN
cd ~/proyecto2
bash scripts/get-zt-iface.sh peer1 TOKEN_DEL_INTEGRANTE_1 10.10.10.1

# El script genera el comando exacto. Copiarlo y ejecutarlo.

# Verificar que el agent arranco
sudo systemctl status k3s-agent
```

### Configurar kubectl

```bash
mkdir -p ~/.kube
# El Integrante 1 te envia el contenido de su ~/.kube/config
# Pegarlo en este archivo y cambiar la IP:
nano ~/.kube/config
# Buscar la linea que dice:  server: https://127.0.0.1:6443
# Cambiarla por:             server: https://10.10.10.1:6443

# Verificar que ves el cluster
kubectl get nodes
# Deben aparecer el nodo del Integrante 1 y el tuyo (Ready)
```

### Construir y cargar imagen en k3s

```bash
cd ~/proyecto2

# Construir la imagen y cargarla en k3s
bash scripts/build-and-load.sh

# Verificar imagen disponible
sudo k3s ctr images list | grep iot-pipeline
```

### Verificar tus pods

Los manifiestos ya los aplico el Integrante 1. Los pods de edge-peer1 y sensors-peer1
se programan automaticamente en tu nodo.

```bash
# Ver pods en tu nodo
kubectl get pods -n iot-pipeline -o wide
# Los pods edge-peer1-XXXX deben estar en la columna NODE con tu IP

# Ver logs de los pods edge
kubectl logs -n iot-pipeline -l app=edge,peer=peer1 -f --max-log-requests=4

# Ver que el Service NodePort existe
kubectl get svc -n iot-pipeline
# edge-peer1-svc debe tener puerto 30090
```

---

## INTEGRANTE 3 — PEER 2 (10.10.10.3)

### Instalar k3s como AGENT

```bash
# OPCION FACIL: usar el script con tu TOKEN
cd ~/proyecto2
bash scripts/get-zt-iface.sh peer2 TOKEN_DEL_INTEGRANTE_1 10.10.10.1

# El script genera el comando exacto. Copiarlo y ejecutarlo.

sudo systemctl status k3s-agent
```

### Configurar kubectl

```bash
mkdir -p ~/.kube
nano ~/.kube/config
# Pegar el config del Integrante 1
# Cambiar server: https://127.0.0.1:6443 por server: https://10.10.10.1:6443

kubectl get nodes
```

### Construir y cargar imagen

```bash
cd ~/proyecto2
bash scripts/build-and-load.sh
sudo k3s ctr images list | grep iot-pipeline
```

### Verificar pods del Peer 2

```bash
kubectl get pods -n iot-pipeline -o wide
# Los pods edge-peer2-XXXX deben estar en tu nodo

kubectl logs -n iot-pipeline -l app=edge,peer=peer2 -f --max-log-requests=4
```

---

## PASO 5 — Verificar el sistema completo

```bash
cd ~/proyecto2

# Verificacion Kubernetes
bash scripts/verify-k8s.sh 10.10.10.1

# Ver todos los pods en tiempo real
kubectl get pods -n iot-pipeline -w

# Status del coordinador
curl -s http://10.10.10.1:8080/status | python3 -m json.tool

# Metricas completas
curl -s http://10.10.10.1:8080/metrics
```

Lo que debe verse funcionando:
- kubectl get nodes → 3 nodos en estado Ready
- kubectl get pods -n iot-pipeline → 4 pods edge (2 por peer) + 2 pods sensor en Running
- /status del coordinador → active_edges mayor a 0, total_readings creciendo

---

## PASO 6 — Escenarios tc netem (INTEGRANTES 2 Y 3)

```bash
# Encontrar la interfaz ZeroTier exacta
ip link show | grep zt
# Ejemplo: ztabcd1234

# Ejecutar el script de escenarios (reemplazar la interfaz)
sudo bash scripts/scenarios.sh ztXXXXXXXX 10.10.10.1
```

### Comandos manuales por escenario

```bash
IFACE="ztXXXXXXXX"   # Reemplazar con tu interfaz ZeroTier real

# Cargar modulo netem si es necesario
sudo modprobe sch_netem

# ESCENARIO 1 — Baseline (sin degradacion)
sudo tc qdisc del dev $IFACE root 2>/dev/null; true
tc qdisc show dev $IFACE
iperf3 -c 10.10.10.1 -p 5201 -t 10
ping -c 10 10.10.10.1

# ESCENARIO 2 — Latencia IoT
sudo tc qdisc del dev $IFACE root 2>/dev/null; true
sudo tc qdisc add dev $IFACE root netem delay 80ms 20ms
tc qdisc show dev $IFACE
iperf3 -c 10.10.10.1 -p 5201 -t 10
ping -c 10 10.10.10.1

# ESCENARIO 3 — Perdida de paquetes
sudo tc qdisc del dev $IFACE root 2>/dev/null; true
sudo tc qdisc add dev $IFACE root netem loss 8%
tc qdisc show dev $IFACE
iperf3 -c 10.10.10.1 -p 5201 -t 10
ping -c 20 10.10.10.1

# ESCENARIO 4 — Enlace limitado
sudo tc qdisc del dev $IFACE root 2>/dev/null; true
sudo tc qdisc add dev $IFACE root netem rate 512kbit delay 50ms
tc qdisc show dev $IFACE
iperf3 -c 10.10.10.1 -p 5201 -t 10

# ESCENARIO 5 — Falla de pod edge (Kubernetes lo reinicia automaticamente)
kubectl get pods -n iot-pipeline -l app=edge
# Copiar el nombre de un pod y borrarlo:
kubectl delete pod -n iot-pipeline <nombre-del-pod-edge>
# Ver como Kubernetes lo reactiva:
kubectl get pods -n iot-pipeline -w
# Ver deteccion en logs del coordinador:
docker logs iot_coordinator --tail=20
# Verificar metricas post-recuperacion:
curl http://10.10.10.1:8080/metrics

# Limpiar reglas al terminar
sudo tc qdisc del dev $IFACE root 2>/dev/null; true
```

---

## PASO 7 — Evidencias para el reporte

```bash
# Cluster y nodos
kubectl get nodes -o wide
kubectl get all -n iot-pipeline

# Deployments con replicas (evidencia clave de Kubernetes)
kubectl get deployments -n iot-pipeline
kubectl describe deployment edge-peer1 -n iot-pipeline
kubectl describe deployment edge-peer2 -n iot-pipeline

# Services NodePort
kubectl get svc -n iot-pipeline

# Pods con distribucion en nodos
kubectl get pods -n iot-pipeline -o wide

# Contenedores Docker del coordinador
docker ps

# ZeroTier
sudo zerotier-cli listnetworks
sudo zerotier-cli listpeers

# Reglas tc durante escenarios
sudo tc qdisc show dev ztXXXXXXXX

# API del coordinador
curl -s http://10.10.10.1:8080/status | python3 -m json.tool
curl -s http://10.10.10.1:8080/metrics

# Logs del coordinador detectando nodos y anomalias
docker logs iot_coordinator --tail=50

# Rollout status (para mostrar tolerancia a fallos)
kubectl rollout status deployment/edge-peer1 -n iot-pipeline
kubectl rollout status deployment/edge-peer2 -n iot-pipeline
```

---

## Comandos utiles de operacion

```bash
# Logs de todos los pods edge en tiempo real
kubectl logs -n iot-pipeline -l app=edge -f --max-log-requests=8

# Reiniciar un deployment completo
kubectl rollout restart deployment/edge-peer1 -n iot-pipeline

# Escalar replicas (para mostrar escalabilidad)
kubectl scale deployment edge-peer1 --replicas=3 -n iot-pipeline
kubectl scale deployment edge-peer1 --replicas=2 -n iot-pipeline

# Matar un pod (k3s lo reinicia solo, util para escenario 5)
kubectl delete pod -n iot-pipeline <nombre-pod>

# Ver eventos del cluster
kubectl get events -n iot-pipeline --sort-by='.lastTimestamp'

# Detener todo
docker compose -f docker-compose.coordinator.yml down   # Integrante 1
sudo systemctl stop k3s-agent                           # Integrantes 2 y 3
sudo systemctl stop k3s                                 # Integrante 1

# Reiniciar
sudo systemctl restart k3s
sudo systemctl restart k3s-agent
```

---

## Solucion de problemas

### k3s agent no se conecta al server
```bash
# Verificar puerto 6443 abierto en Integrante 1
sudo ufw allow 6443/tcp
# Probar conectividad
curl -k https://10.10.10.1:6443
# Ver logs del agent
sudo journalctl -u k3s-agent -f
```

### Pods en estado Pending
```bash
kubectl describe pod -n iot-pipeline <nombre-pod>
# "no nodes available" -> el nodo agent no se unio correctamente
# "image not found"    -> ejecutar build-and-load.sh en ese nodo
```

### Imagen no encontrada en k3s
```bash
# Debe ejecutarse en CADA nodo donde correran los pods
bash scripts/build-and-load.sh
sudo k3s ctr images list | grep iot-pipeline
```

### tc: No such file or directory
```bash
sudo modprobe sch_netem
ip link show | grep zt   # verificar nombre exacto de interfaz
```

### Pods edge no conectan al coordinador
```bash
# Probar desde dentro del pod
kubectl exec -n iot-pipeline <pod-edge> -- curl http://10.10.10.1:8080/status
# Si falla: verificar firewall del Integrante 1
sudo ufw allow 8080/tcp
```

---

## Justificacion tecnica de ZeroTier (seccion obligatoria del reporte)

Los tres integrantes operan bajo CGNAT, lo que impide recibir conexiones entrantes sin IP publica. Se evaluaron las siguientes alternativas:

WireGuard hub propio: Requiere al menos un nodo con IP publica estatica. Ninguno de los integrantes dispone de ella en su proveedor de internet residencial.

WireGuard con VPS gratuito: Se evaluo Oracle Free Tier y Fly.io. El proceso de aprovisionamiento presento demoras en la aprobacion de cuentas y no garantizaba disponibilidad inmediata para la entrega.

ZeroTier (solucion adoptada): Proporciona una red virtual overlay peer-to-peer con traversal automatico de CGNAT y NAT, cifrado de capa 2 mediante curva eliptica, y administracion del plano de control en la nube de ZeroTier Inc. El trafico de datos es peer-to-peer y cifrado extremo a extremo. El software es de codigo abierto (Business Source License 1.1).

Compensaciones implementadas segun tabla Nivel 3:
- tc netem documentado con 5 escenarios distintos, medicion iperf3 antes y despues en cada uno, aplicado sobre la interfaz ZeroTier (no eth0, para afectar solo el trafico inter-nodo real).
- Kubernetes con k3s: edge nodes desplegados como Deployments con 2 replicas cada uno, con RollingUpdate strategy y health checks.
- Backoff exponencial y reintentos automaticos en todos los nodos Rust (sensor, edge, coordinator).
- Reconexion automatica de edges sin intervencion manual.
- Deteccion de nodos caidos en menos de 10 segundos mediante heartbeats cada 3 segundos.
