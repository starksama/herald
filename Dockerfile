# Build stage
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf

WORKDIR /app

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/api/Cargo.toml crates/api/
COPY crates/worker/Cargo.toml crates/worker/
COPY crates/core/Cargo.toml crates/core/
COPY crates/db/Cargo.toml crates/db/

# Create dummy files to build dependencies
RUN mkdir -p crates/api/src crates/worker/src crates/core/src crates/db/src && \
    echo "fn main() {}" > crates/api/src/main.rs && \
    echo "fn main() {}" > crates/worker/src/main.rs && \
    echo "" > crates/core/src/lib.rs && \
    echo "" > crates/db/src/lib.rs

# Build dependencies
RUN cargo build --release

# Copy actual source
COPY crates crates

# Touch files to invalidate cache and rebuild
RUN touch crates/api/src/main.rs crates/worker/src/main.rs crates/core/src/lib.rs crates/db/src/lib.rs

# Build the actual binaries
RUN cargo build --release

# Runtime stage
FROM alpine:3.19

RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/herald-api /usr/local/bin/
COPY --from=builder /app/target/release/herald-worker /usr/local/bin/

# Copy migrations for reference
COPY migrations /app/migrations

EXPOSE 8080

CMD ["herald-api"]
