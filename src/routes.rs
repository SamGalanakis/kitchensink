use std::convert::Infallible;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use bytes::Bytes;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{Mutex, broadcast};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::agent::{append_chat_message, ensure_agent_session, run_chat_turn};
use crate::auth::{expired_session_cookie, session_cookie, verify_password};
use crate::config::Config;
use crate::db::SurrealDb;
use crate::documents::{
    content_search_text, ensure_document_markup, extract_document_references,
    graph_node_record_key, imported_document_markup, imported_url_markup, normalize_node_id,
    slugify_node_id,
};
use crate::extract::{clean_text, extract_text, extract_text_from_html, make_summary};
use crate::model::describe_image;
use crate::models::{
    AssetDto, ChatMessageDto, ChatSendRequest, GraphDto, GraphEdgeDto, GraphNodeDto, ImportJobDto,
    KIND_DOCUMENT, KIND_IMAGE, KIND_TOPIC, KIND_URL, LoginRequest, ModelSettingsDto, NodeDetailDto,
    SOURCE_UPLOAD, SOURCE_URL, SearchResultDto, ServerEvent, SessionDto, StoredAsset,
    StoredAuthSession, StoredAuthUser, StoredChatMessage, StoredGraphEdge, StoredGraphNode,
    StoredImportJob, StoredModelSettings, UpdateModelSettingsRequest, UrlImportRequest,
    WorkspaceSnapshotDto, record_id_string,
};
use crate::storage::AssetStorage;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: SurrealDb,
    pub storage: AssetStorage,
    pub http: Client,
    pub events: broadcast::Sender<ServerEvent>,
    pub active_turn: Arc<Mutex<Option<ActiveTurn>>>,
}

#[derive(Clone)]
pub struct ActiveTurn {
    pub id: String,
    pub cancel: CancellationToken,
}

#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    message: String,
}

type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
}

impl AppState {
    pub fn new(config: Config, db: SurrealDb, storage: AssetStorage) -> Self {
        let http = Client::builder()
            .user_agent("kitchensink/0.1")
            .build()
            .expect("build reqwest client");
        let (events, _) = broadcast::channel(256);
        Self {
            config,
            db,
            storage,
            http,
            events,
            active_turn: Arc::new(Mutex::new(None)),
        }
    }
}

impl AppError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn internal(error: impl Into<anyhow::Error>) -> Self {
        let error = error.into();
        tracing::error!(error = %error, "request failed");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": self.message,
            })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        AppError::internal(value)
    }
}

pub fn build_router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/session", get(session))
        .route("/api/workspace", get(workspace))
        .route("/api/events", get(events))
        .route("/api/graph", get(graph))
        .route("/api/search", get(search))
        .route("/api/nodes/{id}", get(node_detail))
        .route("/api/assets/{id}/file", get(asset_file))
        .route("/api/import/upload", post(import_upload))
        .route("/api/import/url", post(import_url))
        .route("/api/import/jobs", get(import_jobs))
        .route(
            "/api/settings/model",
            get(get_model_settings).put(update_model_settings),
        )
        .route("/api/chat/history", get(chat_history))
        .route("/api/chat/send", post(chat_send))
        .route("/api/chat/stop", post(chat_stop))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> AppResult<impl IntoResponse> {
    assert_origin(&state, &headers)?;
    let user = load_admin_user(&state)
        .await?
        .ok_or_else(|| AppError::unauthorized("no admin user is configured"))?;
    let valid =
        verify_password(&user.password_hash, &payload.password).map_err(AppError::internal)?;
    if !valid {
        return Err(AppError::unauthorized("invalid password"));
    }

    let token = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::days(30);
    state
        .db
        .client()
        .query(
            "CREATE type::record('auth_session', $id) CONTENT {
                user_id: $user_id,
                token: $token,
                expires_at: $expires_at,
                created_at: $created_at,
                last_seen_at: $last_seen_at
            };",
        )
        .bind(("id", token.clone()))
        .bind(("user_id", record_id_string(&user.id)))
        .bind(("token", token.clone()))
        .bind(("expires_at", expires_at))
        .bind(("created_at", now))
        .bind(("last_seen_at", now))
        .await
        .map_err(AppError::internal)?;

    let jar = jar.add(session_cookie(
        &state.config.session_cookie_name,
        &token,
        state.config.secure_cookies(),
    ));
    Ok((
        jar,
        Json(json!({
            "username": user.username,
        })),
    ))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    assert_origin(&state, &headers)?;
    if let Some(cookie) = jar.get(&state.config.session_cookie_name) {
        let _ = state
            .db
            .client()
            .query("DELETE auth_session WHERE token = $token;")
            .bind(("token", cookie.value().to_string()))
            .await;
    }
    let jar = jar.add(expired_session_cookie(
        &state.config.session_cookie_name,
        state.config.secure_cookies(),
    ));
    Ok((jar, StatusCode::NO_CONTENT))
}

async fn session(State(state): State<AppState>, jar: CookieJar) -> AppResult<Json<SessionDto>> {
    let user = require_user(&state, &jar).await?;
    Ok(Json(SessionDto {
        username: user.username,
    }))
}

async fn workspace(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Json<WorkspaceSnapshotDto>> {
    let user = require_user(&state, &jar).await?;
    ensure_agent_session(&state)
        .await
        .map_err(AppError::internal)?;
    let settings = load_model_settings(&state).await?;
    let assets = load_assets(&state, 32).await?;
    let jobs = load_import_jobs(&state, 32).await?;
    let chat = load_chat_messages(&state, 120).await?;
    let graph = load_graph(&state).await?;
    Ok(Json(WorkspaceSnapshotDto {
        session: SessionDto {
            username: user.username,
        },
        settings: settings.into(),
        assets: assets.into_iter().map(AssetDto::from).collect(),
        import_jobs: jobs.into_iter().map(ImportJobDto::from).collect(),
        chat: chat.into_iter().map(ChatMessageDto::from).collect(),
        graph,
    }))
}

async fn events(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, Infallible>>>> {
    let _ = require_user(&state, &jar).await?;
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|event| match event {
        Ok(event) => {
            let data = serde_json::to_string(&event).ok()?;
            Some(Ok(axum::response::sse::Event::default()
                .event("update")
                .data(data)))
        }
        Err(_) => None,
    });
    Ok(Sse::new(stream))
}

async fn graph(State(state): State<AppState>, jar: CookieJar) -> AppResult<Json<GraphDto>> {
    let _ = require_user(&state, &jar).await?;
    Ok(Json(load_graph(&state).await?))
}

async fn search(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<SearchResultDto>>> {
    let _ = require_user(&state, &jar).await?;
    Ok(Json(search_nodes(&state, &query.q).await?))
}

async fn node_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> AppResult<Json<NodeDetailDto>> {
    let _ = require_user(&state, &jar).await?;
    Ok(Json(load_node_detail(&state, &id).await?))
}

async fn asset_file(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let _ = require_user(&state, &jar).await?;
    let asset = load_asset(&state, &id)
        .await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "asset not found"))?;
    let bytes = state
        .storage
        .get_bytes(&asset.storage_key)
        .await
        .map_err(AppError::internal)?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    Ok(response)
}

async fn import_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let _ = require_user(&state, &jar).await?;
    assert_origin(&state, &headers)?;

    while let Some(field) = multipart.next_field().await.map_err(AppError::internal)? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .or_else(|| {
                filename
                    .as_ref()
                    .and_then(|name| mime_guess::from_path(name).first())
                    .map(|mime| mime.to_string())
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let bytes = field.bytes().await.map_err(AppError::internal)?;
        let label = filename
            .clone()
            .unwrap_or_else(|| format!("Upload {}", Uuid::new_v4()));
        let kind = if content_type.starts_with("image/") {
            KIND_IMAGE
        } else {
            KIND_DOCUMENT
        };
        let ids = register_import(
            &state,
            kind,
            SOURCE_UPLOAD,
            &label,
            filename,
            None,
            &content_type,
            bytes,
            json!({}),
        )
        .await?;
        return Ok(Json(json!({
            "asset_id": ids.asset_id,
            "node_id": ids.node_id,
            "job_id": ids.job_id,
        })));
    }

    Err(AppError::bad_request("no file field was provided"))
}

async fn import_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<UrlImportRequest>,
) -> AppResult<Json<Value>> {
    let _ = require_user(&state, &jar).await?;
    assert_origin(&state, &headers)?;
    let url = Url::parse(&payload.url).map_err(|error| AppError::bad_request(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::bad_request(
            "only http and https URLs are supported",
        ));
    }

    let response = state
        .http
        .get(url.clone())
        .send()
        .await
        .map_err(AppError::internal)?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "url fetch failed with {}",
            response.status()
        )));
    }

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
        .or_else(|| {
            mime_guess::from_path(url.path())
                .first()
                .map(|mime| mime.to_string())
        })
        .unwrap_or_else(|| "text/html".to_string());
    let bytes = response.bytes().await.map_err(AppError::internal)?;
    let label = url
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .unwrap_or_else(|| url.host_str().unwrap_or("Imported URL").to_string());
    let kind = if content_type.starts_with("image/") {
        KIND_IMAGE
    } else {
        KIND_URL
    };
    let ids = register_import(
        &state,
        kind,
        SOURCE_URL,
        &label,
        None,
        Some(payload.url.clone()),
        &content_type,
        bytes,
        json!({ "url": payload.url }),
    )
    .await?;
    Ok(Json(json!({
        "asset_id": ids.asset_id,
        "node_id": ids.node_id,
        "job_id": ids.job_id,
    })))
}

async fn import_jobs(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Json<Vec<ImportJobDto>>> {
    let _ = require_user(&state, &jar).await?;
    Ok(Json(
        load_import_jobs(&state, 32)
            .await?
            .into_iter()
            .map(ImportJobDto::from)
            .collect(),
    ))
}

async fn get_model_settings(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Json<ModelSettingsDto>> {
    let _ = require_user(&state, &jar).await?;
    Ok(Json(load_model_settings(&state).await?.into()))
}

async fn update_model_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<UpdateModelSettingsRequest>,
) -> AppResult<Json<ModelSettingsDto>> {
    let _ = require_user(&state, &jar).await?;
    assert_origin(&state, &headers)?;

    let current = load_model_settings(&state).await?;
    let next_api_key = if payload.clear_api_key {
        None
    } else if let Some(api_key) = payload.api_key.as_ref() {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            current.api_key
        } else {
            Some(trimmed.to_string())
        }
    } else {
        current.api_key
    };
    let now = Utc::now();
    state
        .db
        .client()
        .query(
            "UPDATE type::record('app_setting', 'model') MERGE {
                base_url: $base_url,
                api_key: $api_key,
                model: $model,
                updated_at: $updated_at
            };",
        )
        .bind(("base_url", payload.base_url.trim().to_string()))
        .bind(("api_key", next_api_key))
        .bind(("model", payload.model.trim().to_string()))
        .bind(("updated_at", now))
        .await
        .map_err(AppError::internal)?;

    emit(
        &state,
        "settings.updated",
        "app_setting",
        json!({ "id": "model" }),
    );
    Ok(Json(load_model_settings(&state).await?.into()))
}

async fn chat_history(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Json<Vec<ChatMessageDto>>> {
    let _ = require_user(&state, &jar).await?;
    ensure_agent_session(&state)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(
        load_chat_messages(&state, 120)
            .await?
            .into_iter()
            .map(ChatMessageDto::from)
            .collect(),
    ))
}

async fn chat_send(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<ChatSendRequest>,
) -> AppResult<Json<Value>> {
    let _ = require_user(&state, &jar).await?;
    assert_origin(&state, &headers)?;
    ensure_agent_session(&state)
        .await
        .map_err(AppError::internal)?;
    let content = clean_text(&payload.content);
    if content.is_empty() {
        return Err(AppError::bad_request("message cannot be empty"));
    }

    {
        let guard = state.active_turn.lock().await;
        if guard.is_some() {
            return Err(AppError::new(
                StatusCode::CONFLICT,
                "an assistant turn is already running",
            ));
        }
    }

    let user_message = append_chat_message(&state, "user", &content, None, json!({}))
        .await
        .map_err(AppError::internal)?;
    emit(
        &state,
        "chat.updated",
        "chat_message",
        json!({ "id": record_id_string(&user_message.id), "role": "user" }),
    );

    let cancel = CancellationToken::new();
    let turn_id = Uuid::new_v4().to_string();
    {
        let mut guard = state.active_turn.lock().await;
        *guard = Some(ActiveTurn {
            id: turn_id.clone(),
            cancel: cancel.clone(),
        });
    }
    emit(
        &state,
        "chat.turn_started",
        "chat",
        json!({ "turn_id": turn_id }),
    );

    let state_clone = state.clone();
    let content_clone = content.clone();
    tokio::spawn(async move {
        if let Err(error) =
            run_chat_turn(state_clone.clone(), turn_id.clone(), content_clone, cancel).await
        {
            tracing::error!(turn_id = %turn_id, error = %error, "chat turn failed");
            let _ = append_chat_message(
                &state_clone,
                "assistant",
                &format!("The assistant failed: {error}"),
                None,
                json!({ "error": true }),
            )
            .await;
            emit(
                &state_clone,
                "chat.updated",
                "chat_message",
                json!({ "role": "assistant", "error": true }),
            );
        }
        clear_active_turn(&state_clone, &turn_id).await;
        emit(
            &state_clone,
            "chat.turn_finished",
            "chat",
            json!({ "turn_id": turn_id }),
        );
    });

    Ok(Json(json!({ "ok": true })))
}

async fn chat_stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> AppResult<Json<Value>> {
    let _ = require_user(&state, &jar).await?;
    assert_origin(&state, &headers)?;
    let guard = state.active_turn.lock().await;
    if let Some(turn) = guard.as_ref() {
        turn.cancel.cancel();
        emit(
            &state,
            "chat.turn_cancelled",
            "chat",
            json!({ "turn_id": turn.id }),
        );
    }
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn create_graph_node(
    state: &AppState,
    kind: &str,
    label: &str,
    requested_node_id: Option<&str>,
    summary: Option<&str>,
    content: Option<&str>,
    search_text: Option<&str>,
    source: Option<&str>,
) -> Result<GraphNodeDto> {
    ensure_allowed_kind(kind)?;
    let node_id = allocate_node_id(state, kind, requested_node_id.unwrap_or(label)).await?;
    let record_key = graph_node_record_key(kind, &node_id);
    let content = content
        .map(ensure_document_markup)
        .filter(|value| !value.trim().is_empty());
    let derived_search = content
        .as_deref()
        .map(content_search_text)
        .or_else(|| search_text.map(clean_text))
        .filter(|value| !value.is_empty());
    let summary = summary
        .map(clean_text)
        .filter(|value| !value.is_empty())
        .or_else(|| derived_search.as_deref().and_then(make_summary));
    let now = Utc::now();

    state
        .db
        .client()
        .query(
            "CREATE type::record('kg_node', $record_key) CONTENT {
                kind: $kind,
                node_id: $node_id,
                label: $label,
                summary: $summary,
                content: $content,
                search_text: $search_text,
                source: $source,
                asset_id: NONE,
                metadata: {},
                created_at: $created_at,
                updated_at: $updated_at
             };",
        )
        .bind(("record_key", record_key.clone()))
        .bind(("kind", kind.to_string()))
        .bind(("node_id", node_id))
        .bind(("label", label.trim().to_string()))
        .bind(("summary", summary))
        .bind(("content", content.clone()))
        .bind(("search_text", derived_search))
        .bind((
            "source",
            source.map(clean_text).filter(|value| !value.is_empty()),
        ))
        .bind(("created_at", now))
        .bind(("updated_at", now))
        .await?;

    let node = load_node(state, &record_key).await?;
    let node_ref = record_id_string(&node.id);
    sync_content_reference_edges(state, &node_ref, node.content.as_deref()).await?;
    Ok(node.into())
}

pub(crate) async fn update_node_fields(
    state: &AppState,
    node_id: &str,
    label: Option<String>,
    summary: Option<String>,
    content: Option<String>,
    search_text: Option<String>,
    source: Option<String>,
) -> Result<GraphNodeDto> {
    let existing = load_node(state, node_id).await?;
    let next_content = content
        .map(|value| ensure_document_markup(&value))
        .filter(|value| !value.trim().is_empty())
        .or(existing.content.clone());
    let next_search = search_text
        .map(|value| clean_text(&value))
        .filter(|value| !value.is_empty())
        .or_else(|| next_content.as_deref().map(content_search_text))
        .or(existing.search_text.clone());
    let next_summary = summary
        .map(|value| clean_text(&value))
        .filter(|value| !value.is_empty())
        .or_else(|| next_search.as_deref().and_then(make_summary))
        .or(existing.summary.clone());
    let next_label = label
        .map(|value| clean_text(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or(existing.label.clone());
    let next_source = source
        .map(|value| clean_text(&value))
        .filter(|value| !value.is_empty())
        .or(existing.source.clone());
    let now = Utc::now();

    state
        .db
        .client()
        .query(
            "UPDATE type::record('kg_node', $node_id) MERGE {
                label: $label,
                summary: $summary,
                content: $content,
                search_text: $search_text,
                source: $source,
                updated_at: $updated_at
            };",
        )
        .bind(("node_id", external_id_key(node_id).to_string()))
        .bind(("label", next_label))
        .bind(("summary", next_summary))
        .bind(("content", next_content.clone()))
        .bind(("search_text", next_search))
        .bind(("source", next_source))
        .bind(("updated_at", now))
        .await?;

    sync_content_reference_edges(state, node_id, next_content.as_deref()).await?;
    Ok(load_node(state, node_id).await?.into())
}

pub(crate) async fn create_edge(
    state: &AppState,
    from: &str,
    to: &str,
    relation: &str,
) -> Result<GraphEdgeDto> {
    let now = Utc::now();
    let mut response = state
        .db
        .client()
        .query(
            "RELATE type::record('kg_node', $from)->kg_edge->type::record('kg_node', $to)
             CONTENT {
                relation: $relation,
                metadata: {},
                created_at: $created_at
             };",
        )
        .bind(("from", external_id_key(from).to_string()))
        .bind(("to", external_id_key(to).to_string()))
        .bind(("relation", relation.to_string()))
        .bind(("created_at", now))
        .await?;
    let edge: Option<StoredGraphEdge> = response.take(0)?;
    Ok(edge.ok_or_else(|| anyhow!("failed to create edge"))?.into())
}

struct ImportIds {
    asset_id: String,
    node_id: String,
    job_id: String,
}

async fn register_import(
    state: &AppState,
    kind: &str,
    source_kind: &str,
    label: &str,
    filename: Option<String>,
    source_url: Option<String>,
    content_type: &str,
    bytes: Bytes,
    metadata: Value,
) -> AppResult<ImportIds> {
    let asset_id = Uuid::new_v4().to_string();
    let node_id = allocate_node_id(state, kind, label)
        .await
        .map_err(AppError::internal)?;
    let node_record_key = graph_node_record_key(kind, &node_id);
    let job_id = Uuid::new_v4().to_string();
    let extension = mime_guess::get_mime_extensions_str(content_type)
        .and_then(|exts| exts.first())
        .copied()
        .unwrap_or("bin");
    let storage_key = format!("{source_kind}/{asset_id}.{extension}");
    state
        .storage
        .put_bytes(&storage_key, content_type, bytes.clone())
        .await
        .map_err(AppError::internal)?;

    let now = Utc::now();
    state
        .db
        .client()
        .query(
            "BEGIN TRANSACTION;
             CREATE type::record('asset', $asset_id) CONTENT {
                kind: $kind,
                source_kind: $source_kind,
                label: $label,
                filename: $filename,
                source_url: $source_url,
                content_type: $content_type,
                byte_size: $byte_size,
                storage_key: $storage_key,
                extraction_status: 'pending',
                extracted_text: NONE,
                image_description: NONE,
                metadata: $metadata,
                created_at: $created_at,
                updated_at: $updated_at
             };
             CREATE type::record('kg_node', $node_record_key) CONTENT {
                kind: $kind,
                node_id: $node_id,
                label: $label,
                summary: NONE,
                content: NONE,
                search_text: NONE,
                source: $source,
                asset_id: type::record('asset', $asset_id),
                metadata: $metadata,
                created_at: $created_at,
                updated_at: $updated_at
             };
             CREATE type::record('import_job', $job_id) CONTENT {
                asset_id: $asset_ref,
                node_id: $node_ref,
                headline: $headline,
                detail: $detail,
                status: 'pending',
                error: NONE,
                created_at: $created_at,
                updated_at: $updated_at
             };
             COMMIT TRANSACTION;",
        )
        .bind(("asset_id", asset_id.clone()))
        .bind(("node_id", node_id.clone()))
        .bind(("node_record_key", node_record_key.clone()))
        .bind(("job_id", job_id.clone()))
        .bind(("kind", kind.to_string()))
        .bind(("source_kind", source_kind.to_string()))
        .bind(("label", label.trim().to_string()))
        .bind(("filename", filename.clone()))
        .bind(("source_url", source_url.clone()))
        .bind(("content_type", content_type.to_string()))
        .bind(("byte_size", bytes.len() as i64))
        .bind(("storage_key", storage_key.clone()))
        .bind(("metadata", metadata.clone()))
        .bind(("source", source_url.clone().or(filename.clone())))
        .bind(("asset_ref", format!("asset:{asset_id}")))
        .bind(("node_ref", format!("kg_node:{node_id}")))
        .bind(("headline", format!("Importing {label}")))
        .bind((
            "detail",
            Some(match source_kind {
                SOURCE_URL => "Fetching and extracting the remote source".to_string(),
                _ => "Extracting text and indexing the asset".to_string(),
            }),
        ))
        .bind(("created_at", now))
        .bind(("updated_at", now))
        .await
        .map_err(AppError::internal)?;

    emit(
        state,
        "import.updated",
        "import_job",
        json!({ "id": format!("import_job:{job_id}") }),
    );
    emit(
        state,
        "graph.updated",
        "kg_node",
        json!({ "id": format!("kg_node:{node_record_key}") }),
    );

    let state_clone = state.clone();
    let spawn_asset_id = asset_id.clone();
    let spawn_node_id = node_id.clone();
    let spawn_job_id = job_id.clone();
    tokio::spawn(async move {
        if let Err(error) = process_import(
            state_clone.clone(),
            &spawn_asset_id,
            &spawn_node_id,
            &spawn_job_id,
        )
        .await
        {
            tracing::error!(asset_id = %spawn_asset_id, error = %error, "import processing failed");
            let _ = mark_import_failed(
                &state_clone,
                &spawn_asset_id,
                &spawn_job_id,
                &error.to_string(),
            )
            .await;
        }
    });

    Ok(ImportIds {
        asset_id: format!("asset:{asset_id}"),
        node_id: format!("kg_node:{node_record_key}"),
        job_id: format!("import_job:{job_id}"),
    })
}

async fn process_import(
    state: AppState,
    asset_id: &str,
    node_id: &str,
    job_id: &str,
) -> Result<()> {
    let asset = load_asset(&state, asset_id)
        .await?
        .ok_or_else(|| anyhow!("asset not found"))?;
    let now = Utc::now();
    state
        .db
        .client()
        .query(
            "UPDATE type::record('asset', $asset_id) MERGE {
                extraction_status: 'processing',
                updated_at: $updated_at
            };
            UPDATE type::record('import_job', $job_id) MERGE {
                status: 'processing',
                detail: 'Running extraction and graph indexing',
                updated_at: $updated_at
            };",
        )
        .bind(("asset_id", external_id_key(asset_id).to_string()))
        .bind(("job_id", external_id_key(job_id).to_string()))
        .bind(("updated_at", now))
        .await?;
    emit(
        &state,
        "import.updated",
        "import_job",
        json!({ "id": format!("import_job:{}", external_id_key(job_id)) }),
    );

    let bytes = state.storage.get_bytes(&asset.storage_key).await?;
    let (search_text, image_description) = if asset.kind == KIND_IMAGE {
        let settings = load_model_settings(&state).await?;
        match describe_image(&state.http, &settings, &asset.content_type, &bytes).await {
            Ok(description) => (Some(clean_text(&description)), Some(description)),
            Err(error) => {
                tracing::warn!(asset_id = %asset_id, error = %error, "image description fallback");
                let fallback = clean_text(&asset.label);
                (Some(fallback.clone()), Some(fallback))
            }
        }
    } else {
        let text = extract_text(&asset.content_type, &bytes).or_else(|error| {
            if asset.content_type.contains("html") {
                let html = String::from_utf8_lossy(&bytes);
                Ok(extract_text_from_html(&html))
            } else {
                Err(error)
            }
        })?;
        (Some(text), None)
    };

    let search_text = search_text.unwrap_or_default();
    let summary = make_summary(&search_text);
    let content = match asset.kind.as_str() {
        KIND_DOCUMENT => imported_document_markup(&asset.label, &search_text),
        KIND_URL => imported_url_markup(
            &asset.label,
            asset.source_url.as_deref().unwrap_or_default(),
            &search_text,
        ),
        _ => None,
    };
    let node_source = asset
        .source_url
        .clone()
        .or(asset.filename.clone())
        .or_else(|| Some(asset.label.clone()));
    let updated_at = Utc::now();
    state
        .db
        .client()
        .query(
            "BEGIN TRANSACTION;
             UPDATE type::record('asset', $asset_id) MERGE {
                extraction_status: 'ready',
                extracted_text: $extracted_text,
                image_description: $image_description,
                updated_at: $updated_at
             };
             UPDATE type::record('kg_node', $node_id) MERGE {
                summary: $summary,
                content: $content,
                search_text: $search_text,
                source: $source,
                updated_at: $updated_at
             };
             UPDATE type::record('import_job', $job_id) MERGE {
                status: 'ready',
                detail: 'Indexed and available in graph search',
                error: NONE,
                updated_at: $updated_at
             };
             COMMIT TRANSACTION;",
        )
        .bind(("asset_id", external_id_key(asset_id).to_string()))
        .bind(("node_id", external_id_key(node_id).to_string()))
        .bind(("job_id", external_id_key(job_id).to_string()))
        .bind((
            "extracted_text",
            if asset.kind == KIND_IMAGE {
                None
            } else {
                Some(search_text.clone())
            },
        ))
        .bind(("image_description", image_description))
        .bind(("summary", summary))
        .bind(("content", content.clone()))
        .bind(("search_text", search_text))
        .bind(("source", node_source))
        .bind(("updated_at", updated_at))
        .await?;
    sync_content_reference_edges(&state, node_id, content.as_deref()).await?;
    emit(
        &state,
        "import.updated",
        "import_job",
        json!({ "id": format!("import_job:{}", external_id_key(job_id)) }),
    );
    emit(
        &state,
        "graph.updated",
        "kg_node",
        json!({ "id": format!("kg_node:{}", external_id_key(node_id)) }),
    );
    Ok(())
}

async fn mark_import_failed(
    state: &AppState,
    asset_id: &str,
    job_id: &str,
    error: &str,
) -> Result<()> {
    let now = Utc::now();
    state
        .db
        .client()
        .query(
            "UPDATE type::record('asset', $asset_id) MERGE {
                extraction_status: 'failed',
                updated_at: $updated_at
             };
             UPDATE type::record('import_job', $job_id) MERGE {
                status: 'failed',
                error: $error,
                detail: 'Import failed',
                updated_at: $updated_at
             };",
        )
        .bind(("asset_id", external_id_key(asset_id).to_string()))
        .bind(("job_id", external_id_key(job_id).to_string()))
        .bind(("error", error.to_string()))
        .bind(("updated_at", now))
        .await?;
    emit(
        state,
        "import.updated",
        "import_job",
        json!({ "id": format!("import_job:{}", external_id_key(job_id)), "error": error }),
    );
    Ok(())
}

async fn clear_active_turn(state: &AppState, turn_id: &str) {
    let mut guard = state.active_turn.lock().await;
    if guard.as_ref().map(|turn| turn.id.as_str()) == Some(turn_id) {
        *guard = None;
    }
}

async fn load_admin_user(state: &AppState) -> AppResult<Option<StoredAuthUser>> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM type::record('auth_user', $id);")
        .bind(("id", state.config.admin_username.clone()))
        .await
        .map_err(AppError::internal)?;
    response.take(0).map_err(AppError::internal)
}

async fn require_user(state: &AppState, jar: &CookieJar) -> AppResult<StoredAuthUser> {
    let Some(cookie) = jar.get(&state.config.session_cookie_name) else {
        return Err(AppError::unauthorized("not authenticated"));
    };
    let token = cookie.value();
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM auth_session WHERE token = $token LIMIT 1;")
        .bind(("token", token.to_string()))
        .await
        .map_err(AppError::internal)?;
    let sessions: Vec<StoredAuthSession> = response.take(0).map_err(AppError::internal)?;
    let session = sessions
        .into_iter()
        .next()
        .ok_or_else(|| AppError::unauthorized("session not found"))?;
    if session.expires_at < Utc::now() {
        return Err(AppError::unauthorized("session expired"));
    }
    let user_id = external_id_key(&session.user_id).to_string();
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM type::record('auth_user', $id);")
        .bind(("id", user_id))
        .await
        .map_err(AppError::internal)?;
    let user: Option<StoredAuthUser> = response.take(0).map_err(AppError::internal)?;
    let user = user.ok_or_else(|| AppError::unauthorized("user not found"))?;
    let _ = state
        .db
        .client()
        .query("UPDATE auth_session SET last_seen_at = $now WHERE token = $token;")
        .bind(("now", Utc::now()))
        .bind(("token", token.to_string()))
        .await;
    Ok(user)
}

fn assert_origin(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin
        .to_str()
        .map_err(|_| AppError::bad_request("invalid origin header"))?;
    if state
        .config
        .allowed_origins()
        .iter()
        .any(|allowed| allowed == origin)
    {
        Ok(())
    } else {
        Err(AppError::new(
            StatusCode::FORBIDDEN,
            format!("origin {origin} is not allowed"),
        ))
    }
}

pub(crate) async fn load_model_settings(state: &AppState) -> Result<StoredModelSettings> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM type::record('app_setting', 'model');")
        .await?;
    let settings: Option<StoredModelSettings> = response.take(0)?;
    let settings = settings.ok_or_else(|| anyhow!("model settings are missing"))?;
    Ok(apply_model_defaults(&state.config, settings))
}

fn apply_model_defaults(config: &Config, mut settings: StoredModelSettings) -> StoredModelSettings {
    if settings.base_url.trim().is_empty() {
        settings.base_url = config.default_model_base_url.clone();
    }

    if settings.model.trim().is_empty() {
        settings.model = config.default_model_name.clone();
    }

    let api_key_missing = settings
        .api_key
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true);
    if api_key_missing {
        settings.api_key = config.default_model_api_key.clone();
    }

    settings
}

pub(crate) async fn load_assets(state: &AppState, limit: usize) -> Result<Vec<StoredAsset>> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM asset ORDER BY updated_at DESC LIMIT $limit;")
        .bind(("limit", limit as i64))
        .await?;
    response.take(0).map_err(Into::into)
}

pub(crate) async fn load_asset(state: &AppState, id: &str) -> Result<Option<StoredAsset>> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM type::record('asset', $id);")
        .bind(("id", external_id_key(id).to_string()))
        .await?;
    response.take(0).map_err(Into::into)
}

async fn load_import_jobs(state: &AppState, limit: usize) -> Result<Vec<StoredImportJob>> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM import_job ORDER BY updated_at DESC LIMIT $limit;")
        .bind(("limit", limit as i64))
        .await?;
    response.take(0).map_err(Into::into)
}

async fn load_chat_messages(state: &AppState, limit: usize) -> Result<Vec<StoredChatMessage>> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM chat_message ORDER BY created_at ASC LIMIT $limit;")
        .bind(("limit", limit as i64))
        .await?;
    response.take(0).map_err(Into::into)
}

async fn load_graph(state: &AppState) -> Result<GraphDto> {
    let mut response = state
        .db
        .client()
        .query(
            "SELECT * FROM kg_node ORDER BY updated_at DESC;
             SELECT * FROM kg_edge ORDER BY created_at DESC;",
        )
        .await?;
    let nodes: Vec<StoredGraphNode> = response.take(0)?;
    let edges: Vec<StoredGraphEdge> = response.take(1)?;
    Ok(GraphDto {
        nodes: nodes.into_iter().map(GraphNodeDto::from).collect(),
        edges: edges.into_iter().map(GraphEdgeDto::from).collect(),
    })
}

async fn load_node(state: &AppState, id: &str) -> Result<StoredGraphNode> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM type::record('kg_node', $id);")
        .bind(("id", external_id_key(id).to_string()))
        .await?;
    let node: Option<StoredGraphNode> = response.take(0)?;
    node.ok_or_else(|| anyhow!("node not found"))
}

pub(crate) async fn load_node_detail(state: &AppState, id: &str) -> Result<NodeDetailDto> {
    let key = external_id_key(id).to_string();
    let mut response = state
        .db
        .client()
        .query(
            "SELECT * FROM type::record('kg_node', $id);
             SELECT * FROM kg_edge WHERE out = type::record('kg_node', $id);
             SELECT * FROM kg_edge WHERE in = type::record('kg_node', $id);",
        )
        .bind(("id", key))
        .await?;
    let node: Option<StoredGraphNode> = response.take(0)?;
    let incoming: Vec<StoredGraphEdge> = response.take(1)?;
    let outgoing: Vec<StoredGraphEdge> = response.take(2)?;
    let node = node.ok_or_else(|| anyhow!("node not found"))?;
    let asset = if let Some(asset_id) = node.asset_id.clone() {
        load_asset(state, &record_id_string(&asset_id)).await?
    } else {
        None
    };
    Ok(NodeDetailDto {
        node: node.into(),
        incoming: incoming.into_iter().map(GraphEdgeDto::from).collect(),
        outgoing: outgoing.into_iter().map(GraphEdgeDto::from).collect(),
        asset: asset.map(AssetDto::from),
    })
}

pub(crate) async fn search_nodes(state: &AppState, query: &str) -> Result<Vec<SearchResultDto>> {
    let query = clean_text(query);
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut response = state
        .db
        .client()
        .query(
            "SELECT
                id,
                kind,
                node_id,
                label,
                summary,
                (search::score(0) * 5)
                    + (search::score(1) * 2.5)
                    + (search::score(2) * 2)
                    + (search::score(3) * 1.5)
                    + search::score(4) AS score
             FROM kg_node
             WHERE label @0@ $query
                OR summary @1@ $query
                OR content @2@ $query
                OR search_text @3@ $query
                OR source @4@ $query
             ORDER BY score DESC, updated_at DESC
             LIMIT 24;",
        )
        .bind(("query", query))
        .await?;
    let rows: Vec<Value> = response.take(0)?;
    let mut results = Vec::new();
    for row in rows {
        results.push(SearchResultDto {
            id: row
                .get("id")
                .map(value_record_id_string)
                .unwrap_or_default(),
            kind: row
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            node_id: row
                .get("node_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            label: row
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            summary: row
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string),
            score: row.get("score").and_then(Value::as_f64).unwrap_or_default(),
        });
    }
    Ok(results)
}

fn ensure_allowed_kind(kind: &str) -> Result<()> {
    match normalize_node_id(kind).as_deref() {
        Some(KIND_DOCUMENT | KIND_IMAGE | KIND_URL | KIND_TOPIC) => Ok(()),
        _ => Err(anyhow!("unsupported node kind: {kind}")),
    }
}

async fn allocate_node_id(state: &AppState, kind: &str, candidate: &str) -> Result<String> {
    let base = normalize_node_id(candidate).unwrap_or_else(|| slugify_node_id(candidate));
    let mut suffix = 1;

    loop {
        let node_id = if suffix == 1 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        let record_key = graph_node_record_key(kind, &node_id);
        let mut response = state
            .db
            .client()
            .query("SELECT id FROM type::record('kg_node', $id);")
            .bind(("id", record_key))
            .await?;
        let existing: Option<Value> = response.take(0)?;
        if existing.is_none() {
            return Ok(node_id);
        }
        suffix += 1;
    }
}

async fn sync_content_reference_edges(
    state: &AppState,
    node_id: &str,
    content: Option<&str>,
) -> Result<()> {
    let from = external_id_key(node_id).to_string();
    state
        .db
        .client()
        .query(
            "DELETE kg_edge
             WHERE out = type::record('kg_node', $from)
               AND relation INSIDE ['references', 'documents'];",
        )
        .bind(("from", from.clone()))
        .await?;

    let references = content.map(extract_document_references).unwrap_or_default();
    for reference in references {
        let target = graph_node_record_key(&reference.kind, &reference.node_id);
        let mut response = state
            .db
            .client()
            .query("SELECT id FROM type::record('kg_node', $id);")
            .bind(("id", target.clone()))
            .await?;
        let existing: Option<Value> = response.take(0)?;
        if existing.is_none() {
            continue;
        }

        state
            .db
            .client()
            .query(
                "RELATE type::record('kg_node', $from)->kg_edge->type::record('kg_node', $to)
                 CONTENT {
                    relation: $relation,
                    metadata: {},
                    created_at: $created_at
                 };",
            )
            .bind(("from", from.clone()))
            .bind(("to", target))
            .bind(("relation", reference.relation))
            .bind(("created_at", Utc::now()))
            .await?;
    }

    Ok(())
}

fn emit(state: &AppState, kind: &str, entity: &str, payload: Value) {
    let _ = state.events.send(ServerEvent {
        kind: kind.to_string(),
        entity: entity.to_string(),
        payload,
    });
}

fn external_id_key(id: &str) -> &str {
    id.rsplit_once(':').map(|(_, key)| key).unwrap_or(id)
}

fn value_record_id_string(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(object) = value.as_object()
        && let Some(table) = object.get("tb").and_then(Value::as_str)
        && let Some(id) = object.get("id")
    {
        return format!("{table}:{}", value_record_key_string(id));
    }
    value.to_string().trim_matches('"').to_string()
}

fn value_record_key_string(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(number) = value.as_i64() {
        return number.to_string();
    }
    value.to_string().trim_matches('"').to_string()
}
