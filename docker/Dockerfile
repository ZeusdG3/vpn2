# Etapa de compilación
FROM rust:1.78-slim AS builder

# Dependencias del sistema para compilar
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copiar Cargo.toml y crear src dummy para cachear dependencias
COPY rust/Cargo.toml ./
RUN mkdir src && \
    echo 'fn main(){}' > src/coordinator.rs && \
    echo 'fn main(){}' > src/edge.rs && \
    echo 'fn main(){}' > src/sensor.rs && \
    echo 'pub fn now_ms()->u64{0}' > src/messages.rs && \
    cargo build --release && \
    rm -rf src

# Copiar código real y compilar
COPY rust/src ./src
RUN touch src/*.rs && cargo build --release

# Imagen final mínima
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    iproute2 \
    iputils-ping \
    curl \
    iperf3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/target/release/coordinator .
COPY --from=builder /usr/src/app/target/release/edge .
COPY --from=builder /usr/src/app/target/release/sensor .
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
