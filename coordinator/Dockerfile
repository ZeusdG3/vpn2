FROM rust:latest as builder
WORKDIR /usr/src/app

# Copia todo el proyecto para validar el Workspace y acceder a common[cite: 1, 2]
COPY . .

# Compila específicamente el coordinador
WORKDIR /usr/src/app/coordinator
RUN cargo build --release

# Imagen final
FROM debian:bookworm-slim
COPY --from=builder /usr/src/app/target/release/coordinator /usr/local/bin/coordinator
CMD ["coordinator"]