FROM rust:1.93-slim-bullseye AS builder
WORKDIR /usr/src/drakeify

RUN apt-get update && apt-get install -y pkg-config libc6-dev musl-dev musl-tools build-essential && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./

RUN rustup target add x86_64-unknown-linux-musl

COPY ./src ./src
COPY ./migrations ./migrations
COPY ./static ./static

RUN cargo build --release --target x86_64-unknown-linux-musl

RUN ls -la /usr/src/drakeify/target/release
RUN ls -la /usr/src/drakeify/target/x86_64-unknown-linux-musl/release

FROM debian:bullseye-slim AS certs
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

FROM scratch AS runtime

WORKDIR /

# Copy both binaries
COPY --from=builder /usr/src/drakeify/target/x86_64-unknown-linux-musl/release/drakeify /drakeify
COPY --from=builder /usr/src/drakeify/target/x86_64-unknown-linux-musl/release/drakeify-cli /drakeify-cli

# Symlink drakeify-cli to /bin/sh for k9s compatibility
COPY --from=builder /usr/src/drakeify/target/x86_64-unknown-linux-musl/release/drakeify-cli /bin/sh

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
# LLM Configuration
ARG LLM_HOST=http://localhost:11434
ARG LLM_MODEL=llama3.1:latest
ARG LLM_ENDPOINT=/v1/chat/completions
ARG IDENTITY="helpful assistant"
ARG CONTEXT_SIZE=4096
ARG STREAM=false

# Proxy Mode Configuration
ARG HEADLESS=true
ARG PROXY_PORT=8080
ARG PROXY_HOST=0.0.0.0

# System Prompt
ARG SYSTEM_PROMPT="You are a helpful AI assistant with access to tools and plugins. Be concise, accurate, and helpful."

# Logging Configuration
ARG LOG_LEVEL=info
ARG LOG_TO_FILE=false
ARG LOG_FILE=/var/log/drakeify.log

# Session Configuration
ARG SESSIONS_DIR=/data/sessions
ARG AUTO_SAVE=true

# HTTP Configuration
ARG ALLOW_HTTP=true
ARG HTTP_TIMEOUT_SECS=30
ARG HTTP_MAX_RESPONSE_SIZE=10485760

# Set environment variables from build args
ENV DRAKEIFY_LLM_HOST=${LLM_HOST}
ENV DRAKEIFY_LLM_MODEL=${LLM_MODEL}
ENV DRAKEIFY_LLM_ENDPOINT=${LLM_ENDPOINT}
ENV DRAKEIFY_IDENTITY=${IDENTITY}
ENV DRAKEIFY_CONTEXT_SIZE=${CONTEXT_SIZE}
ENV DRAKEIFY_STREAM=${STREAM}
ENV DRAKEIFY_HEADLESS=${HEADLESS}
ENV DRAKEIFY_PROXY_PORT=${PROXY_PORT}
ENV DRAKEIFY_PROXY_HOST=${PROXY_HOST}
ENV DRAKEIFY_SYSTEM_PROMPT=${SYSTEM_PROMPT}
ENV DRAKEIFY_LOG_LEVEL=${LOG_LEVEL}
ENV DRAKEIFY_LOG_TO_FILE=${LOG_TO_FILE}
ENV DRAKEIFY_LOG_FILE=${LOG_FILE}
ENV DRAKEIFY_SESSIONS_DIR=${SESSIONS_DIR}
ENV DRAKEIFY_AUTO_SAVE=${AUTO_SAVE}
ENV DRAKEIFY_ALLOW_HTTP=${ALLOW_HTTP}
ENV DRAKEIFY_HTTP_TIMEOUT_SECS=${HTTP_TIMEOUT_SECS}
ENV DRAKEIFY_HTTP_MAX_RESPONSE_SIZE=${HTTP_MAX_RESPONSE_SIZE}

EXPOSE ${PROXY_PORT}
CMD ["/drakeify"]