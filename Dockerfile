# syntax=docker/dockerfile:1.7-labs

FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-component --locked && cargo install wasm-tools --locked

WORKDIR /usr/src/app

COPY Cargo.toml ./
COPY wit ./wit
RUN mkdir -p src && printf 'pub fn placeholder() {}\n' > src/lib.rs

# warm dependency cache only
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo component build --release || true

COPY . .

RUN set -eux; \
    pwd; \
    ls -la; \
    echo "---- src ----"; \
    ls -la src; \
    echo "---- wit ----"; \
    ls -la wit; \
    echo "---- Cargo.toml ----"; \
    sed -n '1,200p' Cargo.toml; \
    echo "---- src/lib.rs ----"; \
    sed -n '1,200p' src/lib.rs; \
    echo "---- src/bindings.rs ----"; \
    sed -n '1,120p' src/bindings.rs; \
    echo "---- wit/world.wit ----"; \
    sed -n '1,200p' wit/world.wit

# force real rebuild from real sources
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    set -eux; \
    rm -rf /usr/src/app/target; \
    cargo component build --release; \
    echo "---- release dir ----"; \
    ls -l /usr/src/app/target/wasm32-wasip1/release; \
    echo "---- component wit ----"; \
    wasm-tools component wit /usr/src/app/target/wasm32-wasip1/release/aa_proxy_test_hook.wasm; \
    mkdir -p /out; \
    cp /usr/src/app/target/wasm32-wasip1/release/aa_proxy_test_hook.wasm /out/test_hook.wasm

FROM scratch AS export
COPY --from=builder /out/ /

# DOCKER_BUILDKIT=1 docker build --target export --output type=local,dest=./output .
# scp -O output/*.wasm root@10.0.0.1:/data/wasm-hooks/