# Build stage
FROM rust:1.89-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/lib/mw-nexus

WORKDIR /app

COPY --from=builder /app/target/release/mw-nexus /usr/local/bin/mw-nexus

ENV MW_NEXUS_ADDR=0.0.0.0:4430
ENV MW_NEXUS_DB_PATH=/var/lib/mw-nexus/mw-nexus.sqlite

EXPOSE 4430

ENTRYPOINT ["/usr/local/bin/mw-nexus"]
