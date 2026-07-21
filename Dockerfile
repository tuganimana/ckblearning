# ---- build ----
# ckb-script/ckb-types/ckb-traits 1.1.x (see Cargo.lock) require rustc >= 1.95.
FROM rust:1.96-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/ckb-rust /app/ckb-rust

# Railway injects PORT; default for local `docker run`
ENV PORT=8080
EXPOSE 8080

CMD ["./ckb-rust"]
