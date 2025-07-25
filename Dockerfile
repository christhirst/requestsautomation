FROM clux/muslrust:stable as chef
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN apt-get update && apt-get install -y protobuf-compiler
ENV OPENSSL_DIR=/usr/local/ssl
ENV PROTOC=/usr/bin/protoc
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build & cache dependencies
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json


RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
# Copy source code from previous stage
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY proto ./proto
COPY config ./config
COPY build.rs ./
# Add only what's needed to build

# Build application
RUN cargo build --release --target x86_64-unknown-linux-musl --bin requestsautomation
#RUN cargo clean

FROM gcr.io/distroless/cc AS runtime
#WORKDIR /usr/local/bin/app
COPY --from=planner /app/config /app/config
#COPY --from=planner /app/Config.toml /

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/requestsautomation /app
WORKDIR /app
EXPOSE 8180 8280 50051
CMD ["./requestsautomation"]