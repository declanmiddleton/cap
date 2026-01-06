# Multi-stage Dockerfile for CAP

# Build stage
FROM rust:1.75-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 cap

# Create directories
RUN mkdir -p /app/config /app/logs /app/wordlists /app/reports && \
    chown -R cap:cap /app

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/cap /usr/local/bin/cap

# Copy configuration and wordlists
COPY config/default.toml /app/config/
COPY wordlists/*.txt /app/wordlists/

# Change ownership
RUN chown -R cap:cap /app

# Switch to non-root user
USER cap

# Expose API port
EXPOSE 8443

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD cap --help || exit 1

# Default command
ENTRYPOINT ["cap"]
CMD ["--help"]

