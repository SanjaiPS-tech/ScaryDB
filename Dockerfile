# Multi-arch Dockerfile for ScaryDB
# Build: docker buildx build --platform linux/amd64,linux/arm64 -t scarydb:latest --push .

# Build stage
FROM rust:1.96-bookworm AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source
COPY src ./src

# Build application
RUN cargo build --release

# Runtime stage - distroless (multi-arch)
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/scurydb /app/scurydb
COPY config.json /app/config.json

# Create data directory (distroless runs as nonroot by default)
# The nonroot user is already created in distroless

# Expose port
EXPOSE 6379

# Use non-root user
USER nonroot

# Entry point
ENTRYPOINT ["/app/scurydb", "server"]