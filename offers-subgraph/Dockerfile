# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY offers-subgraph/Cargo.toml ./offers-subgraph/

# Create dummy source files to cache dependencies
RUN mkdir -p offers-subgraph/src && \
    echo "fn main() {}" > offers-subgraph/src/main.rs

# Build dependencies
RUN cargo build --release --package offers-subgraph

# Copy actual source code
COPY offers-subgraph/src ./offers-subgraph/src

# Build the application
RUN touch offers-subgraph/src/main.rs && \
    cargo build --release --package offers-subgraph

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/offers-subgraph /usr/local/bin/offers-subgraph

# Create non-root user
RUN useradd -r -s /bin/false offers
USER offers

EXPOSE 4004

CMD ["offers-subgraph"]