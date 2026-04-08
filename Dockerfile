FROM node:25-bookworm-slim AS web-build
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm install --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM rust:1.93-bookworm AS server-build
WORKDIR /app
COPY Cargo.toml ./
COPY Cargo.lock ./
COPY src ./src
COPY db ./db
COPY --from=web-build /app/web/dist ./web/dist
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=server-build /app/target/release/kitchensink-server /usr/local/bin/kitchensink-server
COPY --from=web-build /app/web/dist ./web/dist
COPY db ./db

ENV APP_ENV=production
ENV BIND_ADDR=0.0.0.0:8080
ENV FRONTEND_DIR=/app/web/dist

EXPOSE 8080
CMD ["kitchensink-server"]
