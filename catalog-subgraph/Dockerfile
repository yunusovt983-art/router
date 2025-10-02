# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY catalog-subgraph/Cargo.toml ./catalog-subgraph/

# Create dummy source files to cache dependencies
RUN mkdir -p catalog-subgraph/src && \
    echo "fn main() {}" > catalog-subgraph/src/main.rs

# Build dependencies
RUN cargo build --release --package catalog-subgraph

# Copy actual source code
COPY catalog-subgraph/src ./catalog-subgraph/src

# Build the application
RUN touch catalog-subgraph/src/main.rs && \
    cargo build --release --package catalog-subgraph

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/catalog-subgraph /usr/local/bin/catalog-subgraph

# Create non-root user
RUN useradd -r -s /bin/false catalog
USER catalog

EXPOSE 4003

CMD ["catalog-subgraph"]