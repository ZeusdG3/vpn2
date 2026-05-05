# ── Stage 1: Builder ──────────────────────────────────────────────────────────
FROM rust:latest AS builder
 
WORKDIR /build
 
# Copiar workspace completo
COPY rust/ .
 
# Compilar sólo el binario coordinator en modo release
RUN cargo build --release -p coordinator
 
# ── Stage 2: Runtime mínimo ───────────────────────────────────────────────────
FROM debian:bookworm-slim
 
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
 
COPY --from=builder /build/target/release/coordinator /usr/local/bin/coordinator
 
# Puerto de datos y heartbeat
EXPOSE 9000 9002
 
ENTRYPOINT ["/usr/local/bin/coordinator"]