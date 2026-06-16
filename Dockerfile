FROM rust:1.88-bookworm AS builder

RUN rustup component add rustfmt clippy llvm-tools-preview
RUN cargo install cargo-llvm-cov --locked

WORKDIR /workspace
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo 'pub fn lib() {}' > src/lib.rs
RUN cargo build --release 2>/dev/null || true
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        apt \
        dpkg \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false -u 10001 aptmcp

COPY --from=builder /workspace/target/release/apt-mcp /usr/local/bin/apt-mcp

USER aptmcp
WORKDIR /home/aptmcp

# MCP stdio transport — bind to stdin/stdout only
ENTRYPOINT ["apt-mcp"]
