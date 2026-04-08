# Kitchensink

Browser-first knowledge graph workspace with one assistant, one full-screen graph canvas, and a Surreal-backed graph for documents, images, URLs, and topics.

## Stack

- Rust 2024 + Axum backend
- `lash` embedded agent runtime pinned to the latest `main` commit from `SamGalanakis/lash`
- SurrealDB Cloud for auth, chat, graph, search, and settings
- Solid + Vite frontend
- Fly.io deployment target
- S3/Tigris-compatible object storage for imported file bytes

## Local development

1. Copy `.env.example` to `.env` and fill in at least:
   - `APP_PASSWORD`
   - `SURREALDB_URL`
   - Surreal auth (`SURREALDB_TOKEN` or username/password)
   - `OPENAI_API_KEY`
2. Install frontend dependencies:

```bash
npm --prefix web install
```

3. Build the frontend once or run it in dev mode:

```bash
npm --prefix web run build
# or
npm --prefix web run dev
```

4. Start the backend:

```bash
cargo run
```

The backend listens on `BIND_ADDR` and serves the built frontend from `FRONTEND_DIR`.

## Checks

```bash
cargo check
npm --prefix web run build
```

## Deploy

Set the Fly secrets from `.env.example`, then deploy:

```bash
flyctl deploy
```
