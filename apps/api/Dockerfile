# syntax=docker/dockerfile:1

# ---- build ----
FROM rust:1-bookworm AS builder

WORKDIR /build

# Workspace manifest + SDK crate (path dependency of the API) + the API crate.
COPY Cargo.toml README.md ./
COPY src ./src
COPY apps ./apps

RUN cargo build --release -p vipayment-api

# ---- runtime ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/vipayment-api /usr/local/bin/vipayment-api
COPY apps/api/config ./config

RUN mkdir -p /app/data/uploads && chown -R 10001:10001 /app

USER 10001:10001

ENV BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=sqlite://data/app.db \
    UPLOAD_DIR=data/uploads \
    GAMES_CONFIG_PATH=config/games.json \
    RUST_LOG=vipayment_api=info,tower_http=info

EXPOSE 8080

CMD ["vipayment-api"]
