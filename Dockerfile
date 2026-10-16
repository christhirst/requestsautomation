FROM clux/muslrust:stable AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    protobuf-compiler \
 && rm -rf /var/lib/apt/lists/*
ENV OPENSSL_DIR=/usr/local/ssl
ENV PROTOC=/usr/bin/protoc
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build & cache dependencies
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Copy source code
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY proto ./proto
COPY config ./config
COPY build.rs ./

# Build application
RUN cargo build --release --target x86_64-unknown-linux-musl --bin requestsautomation

FROM gcr.io/distroless/cc AS runtime
WORKDIR /app
COPY --from=builder /app/config /app/config
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/requestsautomation /app/requestsautomation
EXPOSE 8180 8280 50051
CMD ["./requestsautomation"]