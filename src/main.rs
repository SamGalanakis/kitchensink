mod agent;
mod auth;
mod config;
mod db;
mod documents;
mod extract;
mod model;
mod models;
mod routes;
mod storage;

use std::path::PathBuf;

use anyhow::Result;
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get_service,
};
use config::Config;
use routes::AppState;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("kitchensink_server=info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;
    let db = db::connect(&config).await?;
    db::apply_migrations(&db).await?;
    db::bootstrap_defaults(&db, &config).await?;
    let storage = storage::AssetStorage::from_config(&config).await?;
    let state = AppState::new(config.clone(), db, storage);

    let app = Router::new()
        .merge(routes::build_router())
        .merge(static_router(config.frontend_dir.clone()))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn static_router(frontend_dir: PathBuf) -> Router<AppState> {
    let index = frontend_dir.join("index.html");
    if index.exists() {
        Router::new()
            .nest_service(
                "/assets",
                get_service(ServeDir::new(frontend_dir.join("assets"))),
            )
            .route_service(
                "/favicon.svg",
                ServeFile::new(frontend_dir.join("favicon.svg")),
            )
            .fallback_service(ServeFile::new(index))
    } else {
        Router::new().fallback(fallback_missing_frontend)
    }
}

async fn fallback_missing_frontend(State(state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!(
            "{} frontend is not built yet. Run `npm --prefix web install && npm --prefix web run build`.",
            state.config.app_name
        ),
    )
}
