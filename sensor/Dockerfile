FROM rust:latest as builder
WORKDIR /usr/src/app

# Copia todo el proyecto[cite: 1, 2]
COPY . .

# Compila el sensor
WORKDIR /usr/src/app/sensor
RUN cargo build --release

# Imagen final
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y iproute2 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/sensor /usr/local/bin/sensor
CMD ["sensor"]