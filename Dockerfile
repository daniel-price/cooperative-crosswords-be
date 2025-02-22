FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq5 ca-certificates
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin cooperative-crosswords

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq5 ca-certificates
WORKDIR /app
COPY --from=builder /app/target/release/cooperative-crosswords /usr/local/bin
COPY build.sh /usr/local/bin
ENTRYPOINT ["/usr/local/bin/cooperative-crosswords"]
