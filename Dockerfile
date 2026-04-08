# syntax=docker/dockerfile:1.7

FROM node:25.2.1-bookworm-slim AS web-build
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM rust:1.93-bookworm AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS server-build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY --from=planner /app/recipe.json ./recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json
COPY src ./src
COPY db ./db
COPY --from=web-build /app/web/dist ./web/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/app/target \
    cargo clean --release -p kitchensink-server \
    && cargo build --release --bin kitchensink-server \
    && cp target/release/kitchensink-server /tmp/kitchensink-server

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=server-build /tmp/kitchensink-server /usr/local/bin/kitchensink-server
COPY --from=web-build /app/web/dist ./web/dist
COPY db ./db

ENV APP_ENV=production
ENV BIND_ADDR=0.0.0.0:8080
ENV FRONTEND_DIR=/app/web/dist

EXPOSE 8080
CMD ["kitchensink-server"]
