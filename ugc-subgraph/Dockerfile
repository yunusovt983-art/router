# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/

# Create dummy source files to cache dependencies
RUN mkdir -p ugc-subgraph/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --package ugc-subgraph

# Copy actual source code
COPY ugc-subgraph/src ./ugc-subgraph/src
COPY ugc-subgraph/migrations ./ugc-subgraph/migrations

# Build the application
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# Development stage (for development with hot reload)
FROM builder as development
RUN cargo install cargo-watch
WORKDIR /app
CMD ["cargo", "watch", "-x", "run --package ugc-subgraph"]

# Runtime stage
FROM debian:bookworm-slim as runtime

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
        ca-certificates \
        curl \
        libssl3 \
        && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/ugc-subgraph

# Create non-root user
RUN useradd -r -s /bin/false -u 1001 ugc

# Create directories with proper permissions
RUN mkdir -p /app/logs && \
    chown -R ugc:ugc /app

USER ugc
WORKDIR /app

EXPOSE 4001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]