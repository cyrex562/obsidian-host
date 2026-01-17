FROM rust:1.75 as builder

WORKDIR /app

# Copy manifest files
COPY Cargo.toml ./
COPY src ./src

# Build dependencies and application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/obsidian-host /usr/local/bin/obsidian-host

# Copy frontend files
COPY frontend/public ./frontend/public

# Expose port
EXPOSE 8080

# Set environment variables
ENV OBSIDIAN_SERVER_HOST=0.0.0.0
ENV OBSIDIAN_SERVER_PORT=8080
ENV OBSIDIAN_DATABASE_PATH=/data/obsidian-host.db

# Create data directory
RUN mkdir -p /data

# Run the application
CMD ["obsidian-host"]
