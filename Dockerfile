# Stage 1: Build Frontend
FROM node:20-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build:simple

# Stage 2: Build Backend
FROM rust:1.84-slim-bullseye AS backend-builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev gcc
COPY . .
# Copy built frontend assets from stage 1 to the location expected by rust-embed
# The rust-embed macro looks at "frontend/public/" relative to Cargo.toml
COPY --from=frontend-builder /app/frontend/public ./frontend/public

# Build release binary
# We use --release and --locked to ensure reproducible builds
RUN cargo build --release --locked

# Stage 3: Runtime
FROM debian:bullseye-slim
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=backend-builder /app/target/release/obsidian-host .
# Copy default config
COPY config.toml .

# Create directory for data
RUN mkdir -p /data/vaults

# Set environment variables matching AppConfig (prefix OBSIDIAN__)
ENV OBSIDIAN__SERVER__HOST=0.0.0.0
ENV OBSIDIAN__SERVER__PORT=8080
ENV OBSIDIAN__DATABASE__PATH=/data/obsidian-host.db

# Expose port
EXPOSE 8080

# Define volume for persistent data
VOLUME ["/data"]

CMD ["./obsidian-host"]
