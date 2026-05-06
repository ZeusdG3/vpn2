FROM rust:latest as builder
WORKDIR /usr/src/app

# Copia todo el proyecto[cite: 1, 2]
COPY . .

# Compila el edge
WORKDIR /usr/src/app/edge
RUN cargo build --release

# Imagen final (incluye iproute2 para diagnósticos de red requeridos)
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y iproute2 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/edge /usr/local/bin/edge
CMD ["edge"]