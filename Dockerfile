# syntax=docker/dockerfile:1
FROM rust:latest AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy all manifests + sources
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
COPY migrations ./migrations
COPY assets ./assets

# Build with persistent cache mount — only changed crates recompile
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release \
    && cp target/release/rust-alc-api target/release/migrate /tmp/

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/rust-alc-api /usr/local/bin/
COPY --from=builder /tmp/migrate /usr/local/bin/
COPY --from=builder /app/migrations /app/migrations

WORKDIR /app
ENV PORT=8080
EXPOSE 8080

CMD ["rust-alc-api"]
