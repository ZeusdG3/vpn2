# ── Stage 1: Builder ──────────────────────────────────────────────────────────
FROM rust:latest AS builder
 
WORKDIR /build
 
# Copiar workspace completo
COPY rust/ .
 
# Compilar sólo el binario sensor en modo release
RUN cargo build --release -p sensor
 
# ── Stage 2: Runtime mínimo ───────────────────────────────────────────────────
FROM debian:bookworm-slim
 
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
 
COPY --from=builder /build/target/release/sensor /usr/local/bin/sensor
 
ENTRYPOINT ["/usr/local/bin/sensor"]