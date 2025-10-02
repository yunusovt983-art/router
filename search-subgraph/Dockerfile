# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY search-subgraph/Cargo.toml ./search-subgraph/

# Create dummy source files to cache dependencies
RUN mkdir -p search-subgraph/src && \
    echo "fn main() {}" > search-subgraph/src/main.rs

# Build dependencies
RUN cargo build --release --package search-subgraph

# Copy actual source code
COPY search-subgraph/src ./search-subgraph/src

# Build the application
RUN touch search-subgraph/src/main.rs && \
    cargo build --release --package search-subgraph

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/search-subgraph /usr/local/bin/search-subgraph

# Create non-root user
RUN useradd -r -s /bin/false search
USER search

EXPOSE 4005

CMD ["search-subgraph"]