# FoxNIO Backend Dockerfile

# Build stage
FROM rust:1.88-slim AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY migration ./migration

# Create dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --locked && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build for release
RUN cargo build --release --locked --bin foxnio

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/foxnio /usr/local/bin/

# Copy migration files
COPY migration ./migration

# Create non-root user
RUN useradd -r -s /bin/false foxnio && \
    chown -R foxnio:foxnio /app

USER foxnio

# Expose port
# Align with backend listening port
EXPOSE 8080

# Health check
# Healthcheck targets actual listening port
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the binary
CMD ["foxnio"]
