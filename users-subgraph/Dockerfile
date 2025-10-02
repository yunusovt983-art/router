# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY users-subgraph/Cargo.toml ./users-subgraph/

# Create dummy source files to cache dependencies
RUN mkdir -p users-subgraph/src && \
    echo "fn main() {}" > users-subgraph/src/main.rs

# Build dependencies
RUN cargo build --release --package users-subgraph

# Copy actual source code
COPY users-subgraph/src ./users-subgraph/src

# Build the application
RUN touch users-subgraph/src/main.rs && \
    cargo build --release --package users-subgraph

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/users-subgraph /usr/local/bin/users-subgraph

# Create non-root user
RUN useradd -r -s /bin/false users
USER users

EXPOSE 4002

CMD ["users-subgraph"]