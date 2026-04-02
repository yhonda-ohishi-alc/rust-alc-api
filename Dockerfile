# syntax=docker/dockerfile:1
FROM rust:latest AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Install sccache for fine-grained compilation caching
RUN cargo install sccache --locked
ENV RUSTC_WRAPPER=sccache SCCACHE_DIR=/sccache

# Copy everything
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
COPY migrations ./migrations
COPY assets ./assets

# Build with sccache + cache mounts
# sccache caches at rustc level — only changed crates recompile
# Cache mounts persist via buildkit-cache-dance on CI
RUN --mount=type=cache,id=sccache,target=/sccache,sharing=locked \
    --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release \
    && sccache --show-stats \
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
