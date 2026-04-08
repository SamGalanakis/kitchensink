use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use lash::plugin::StaticPluginFactory;
use lash::{
    EventSink, ExecutionMode, HostProfile, InputItem, LashRuntime, Message, MessageRole, Part,
    PartKind, PluginHost, PluginSpec, Provider, ProviderOptions, PruneState, RuntimeHostConfig,
    RuntimeServices, SessionEvent, SessionPolicy, SessionStateEnvelope, ToolDefinition, ToolParam,
    ToolProvider, ToolResult, TurnInput,
};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::models::{
    AssetDto, ServerEvent, StoredAgentSession, StoredChatMessage, StoredModelSettings,
};
use crate::routes::{
    AppState, create_edge, create_graph_node, load_asset, load_assets, load_model_settings,
    load_node_detail, search_nodes, update_node_fields,
};

const ROOT_SESSION_ID: &str = "root";
const MAX_TURNS_PER_REQUEST: usize = 6;

pub(crate) const SYSTEM_PROMPT: &str = "You are the single assistant inside a private knowledge-graph workspace. The graph has four node kinds: document, image, url, and topic. Be concise, factual, and proactive about using tools when the graph, assets, or the web need to be inspected. Durable knowledge should be stored in graph nodes, especially document nodes with rich `content`. Document content may use Hirsel-style tags such as `<hirsel-callout>`, `<hirsel-card>`, `<hirsel-tabs>`, `<hirsel-disclosure>`, `<hirsel-progress>`, `<hirsel-stat-grid>`, `<hirsel-doc-link node=\"document:foo\">`, `<hirsel-doc-embed node=\"document:foo\">`, `<hirsel-node-ref node=\"topic:bar\">`, and `<hirsel-node-list node=\"topic:bar\" relation=\"references\">`. Inside those tags, reference nodes as `kind:node_id`, not by record id. Use the tool-returned record `id` when another tool needs a node identifier.";

pub(crate) async fn ensure_agent_session(state: &AppState) -> Result<()> {
    let _ = load_or_initialize_session_state(state).await?;
    Ok(())
}

pub(crate) async fn append_chat_message(
    state: &AppState,
    role: &str,
    content: &str,
    tool_name: Option<String>,
    meta: Value,
) -> Result<StoredChatMessage> {
    let message_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let mut response = state
        .db
        .client()
        .query(
            "CREATE type::record('chat_message', $id) CONTENT {
                role: $role,
                content: $content,
                tool_name: $tool_name,
                meta: $meta,
                created_at: $created_at
            };",
        )
        .bind(("id", message_id))
        .bind(("role", role.to_string()))
        .bind(("content", content.to_string()))
        .bind(("tool_name", tool_name))
        .bind(("meta", meta))
        .bind(("created_at", now))
        .await?;
    let row: Option<StoredChatMessage> = response.take(0)?;
    row.ok_or_else(|| anyhow!("failed to create chat message"))
}

pub(crate) async fn run_chat_turn(
    state: AppState,
    turn_id: String,
    content: String,
    cancel: CancellationToken,
) -> Result<()> {
    let mut runtime = build_runtime(&state).await?;
    let sink = WorkspaceEventSink {
        state: state.clone(),
        turn_id: turn_id.clone(),
    };
    let assembled = runtime
        .stream_turn(
            TurnInput {
                items: vec![InputItem::Text { text: content }],
                image_blobs: HashMap::new(),
                mode: None,
            },
            &sink,
            cancel,
        )
        .await
        .context("run lash turn")?;

    save_session_state(&state, &assembled.state).await?;

    if matches!(assembled.status, lash::TurnStatus::Interrupted) {
        return Ok(());
    }

    let assistant_text = if assembled.assistant_output.safe_text.trim().is_empty() {
        if assembled.errors.is_empty() {
            "The assistant finished without returning text.".to_string()
        } else {
            assembled
                .errors
                .iter()
                .map(|issue| issue.message.trim())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        assembled.assistant_output.safe_text.trim().to_string()
    };

    let assistant = append_chat_message(
        &state,
        "assistant",
        &assistant_text,
        None,
        json!({
            "turn_id": turn_id,
            "status": assembled.status,
            "done_reason": assembled.done_reason,
            "token_usage": assembled.token_usage,
            "had_tool_calls": assembled.execution.had_tool_calls,
            "errors": assembled.errors,
        }),
    )
    .await?;
    emit_server_event(
        &state,
        "chat.updated",
        "chat_message",
        json!({ "id": record_id_string(&assistant.id), "role": "assistant" }),
    );
    Ok(())
}

fn build_policy(state: &AppState, settings: &StoredModelSettings) -> Result<SessionPolicy> {
    let api_key = settings
        .api_key
        .clone()
        .ok_or_else(|| anyhow!("model API key is not configured"))?;
    Ok(SessionPolicy {
        model: settings.model.clone(),
        provider: Provider::OpenAiGeneric {
            api_key,
            base_url: settings.base_url.clone(),
            options: ProviderOptions::default(),
        },
        max_context_tokens: Some(state.config.default_model_max_context_tokens),
        model_variant: None,
        session_id: Some(ROOT_SESSION_ID.to_string()),
        max_turns: Some(MAX_TURNS_PER_REQUEST),
        execution_mode: ExecutionMode::Standard,
        context_strategy: lash::default_context_strategy(),
    })
}

async fn build_runtime(state: &AppState) -> Result<LashRuntime> {
    let settings = load_model_settings(state).await?;
    let policy = build_policy(state, &settings)?;
    let mut session_state = load_or_initialize_session_state(state).await?;
    session_state.session_id = ROOT_SESSION_ID.to_string();
    session_state.policy = policy.clone();
    sync_system_prompt(&mut session_state);

    let tools: Arc<dyn ToolProvider> = Arc::new(WorkspaceTools {
        state: state.clone(),
    });
    let plugins = PluginHost::new(vec![Arc::new(StaticPluginFactory::new(
        "kitchensink_tools",
        PluginSpec::new().with_tool_provider(tools),
    ))])
    .build_session(ROOT_SESSION_ID, ExecutionMode::Standard, None)
    .map_err(|error| anyhow!("build lash plugin session: {error}"))?;

    let host = RuntimeHostConfig {
        host_profile: HostProfile::Embedded,
        user_prompts_enabled: false,
        base_dir: Some(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))),
        ..RuntimeHostConfig::default()
    };

    LashRuntime::from_state(policy, host, RuntimeServices::new(plugins), session_state)
        .await
        .map_err(|error| anyhow!("create lash runtime: {error}"))
}

async fn load_or_initialize_session_state(state: &AppState) -> Result<SessionStateEnvelope> {
    let mut response = state
        .db
        .client()
        .query("SELECT * FROM type::record('agent_session', $id);")
        .bind(("id", ROOT_SESSION_ID))
        .await?;
    let session: Option<StoredAgentSession> = response.take(0)?;

    if let Some(session) = session {
        let mut state_value: SessionStateEnvelope =
            serde_json::from_value(session.state).context("decode stored lash session state")?;
        sync_system_prompt(&mut state_value);
        return Ok(state_value);
    }

    let mut fresh = SessionStateEnvelope {
        session_id: ROOT_SESSION_ID.to_string(),
        ..SessionStateEnvelope::default()
    };
    sync_system_prompt(&mut fresh);
    state
        .db
        .client()
        .query("DELETE chat_message;")
        .await
        .context("reset legacy chat transcript during lash cutover")?;
    save_session_state(state, &fresh).await?;
    Ok(fresh)
}

async fn save_session_state(state: &AppState, session: &SessionStateEnvelope) -> Result<()> {
    let now = Utc::now();
    state
        .db
        .client()
        .query(
            "UPSERT type::record('agent_session', $id) CONTENT {
                state: $state,
                updated_at: $updated_at
            };",
        )
        .bind(("id", ROOT_SESSION_ID))
        .bind(("state", serde_json::to_value(session)?))
        .bind(("updated_at", now))
        .await?;
    Ok(())
}

fn sync_system_prompt(state: &mut SessionStateEnvelope) {
    let prompt_part = Part {
        id: "m0.p0".to_string(),
        kind: PartKind::Text,
        content: SYSTEM_PROMPT.to_string(),
        attachment: None,
        tool_call_id: None,
        tool_name: None,
        prune_state: PruneState::Intact,
    };

    if let Some(first) = state.messages.first_mut()
        && matches!(first.role, MessageRole::System)
    {
        first.id = "m0".to_string();
        first.parts = vec![prompt_part];
        renumber_messages(&mut state.messages);
        return;
    }

    state.messages.insert(
        0,
        Message {
            id: "m0".to_string(),
            role: MessageRole::System,
            parts: vec![prompt_part],
            origin: None,
        },
    );
    renumber_messages(&mut state.messages);
}

fn renumber_messages(messages: &mut [Message]) {
    for (message_index, message) in messages.iter_mut().enumerate() {
        message.id = format!("m{message_index}");
        for (part_index, part) in message.parts.iter_mut().enumerate() {
            part.id = format!("m{message_index}.p{part_index}");
        }
    }
}

fn emit_server_event(state: &AppState, kind: &str, entity: &str, payload: Value) {
    let _ = state.events.send(ServerEvent {
        kind: kind.to_string(),
        entity: entity.to_string(),
        payload,
    });
}

fn render_tool_result(result: &Value, success: bool) -> String {
    match result {
        Value::String(text) => text.trim().to_string(),
        value => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
    .chars()
    .take(if success { 6_000 } else { 3_000 })
    .collect()
}

struct WorkspaceEventSink {
    state: AppState,
    turn_id: String,
}

#[async_trait::async_trait]
impl EventSink for WorkspaceEventSink {
    async fn emit(&self, event: SessionEvent) {
        match event {
            SessionEvent::ToolCall {
                call_id,
                name,
                args,
                result,
                success,
                duration_ms,
            } => {
                let content = render_tool_result(&result, success);
                match append_chat_message(
                    &self.state,
                    "tool",
                    &content,
                    Some(name.clone()),
                    json!({
                        "turn_id": self.turn_id,
                        "call_id": call_id,
                        "args": args,
                        "result": result,
                        "success": success,
                        "duration_ms": duration_ms,
                    }),
                )
                .await
                {
                    Ok(message) => emit_server_event(
                        &self.state,
                        "chat.updated",
                        "chat_message",
                        json!({
                            "id": record_id_string(&message.id),
                            "role": "tool",
                            "name": name,
                        }),
                    ),
                    Err(error) => {
                        tracing::error!(turn_id = %self.turn_id, error = %error, "persist tool transcript")
                    }
                }
            }
            SessionEvent::Prompt {
                request,
                response_tx,
            } => {
                let _ = response_tx.send(request.empty_response());
            }
            SessionEvent::Error { message, envelope } => {
                tracing::warn!(
                    turn_id = %self.turn_id,
                    message = %message,
                    envelope = ?envelope,
                    "lash turn surfaced an event error"
                );
            }
            _ => {}
        }
    }
}

struct WorkspaceTools {
    state: AppState,
}

#[async_trait::async_trait]
impl ToolProvider for WorkspaceTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "list_assets".into(),
                description: "List recent imported assets available to the assistant.".into(),
                params: vec![],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: Some(json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                })),
                output_schema_override: None,
            },
            ToolDefinition {
                name: "read_asset".into(),
                description: "Read extracted text or image description for an asset by id.".into(),
                params: vec![ToolParam::typed("asset_id", "str")],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: None,
                output_schema_override: None,
            },
            ToolDefinition {
                name: "fetch_url".into(),
                description: "Fetch a webpage and return a compact extracted-text preview.".into(),
                params: vec![ToolParam::typed("url", "str")],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: None,
                output_schema_override: None,
            },
            ToolDefinition {
                name: "search_web".into(),
                description: "Run a web search when current external information is required.".into(),
                params: vec![ToolParam::typed("query", "str")],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: None,
                output_schema_override: None,
            },
            ToolDefinition {
                name: "search_graph".into(),
                description: "Search graph nodes by keyword.".into(),
                params: vec![ToolParam::typed("query", "str")],
                returns: "list".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: None,
                output_schema_override: None,
            },
            ToolDefinition {
                name: "get_node".into(),
                description: "Load a node with connected incoming and outgoing edges.".into(),
                params: vec![ToolParam::typed("node_id", "str")],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: None,
                output_schema_override: None,
            },
            ToolDefinition {
                name: "create_node".into(),
                description: "Create a new graph node. Use document for durable rich content, topic for concepts, image for visual assets, and url for external sources represented as nodes.".into(),
                params: vec![],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: Some(json!({
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["document", "image", "url", "topic"] },
                        "label": { "type": "string" },
                        "node_id": { "type": "string" },
                        "summary": { "type": "string" },
                        "content": { "type": "string" },
                        "search_text": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["kind", "label"],
                    "additionalProperties": false
                })),
                output_schema_override: None,
            },
            ToolDefinition {
                name: "update_node".into(),
                description: "Update top-level graph node fields. For documents, update `content` with rich Hirsel-style markup when durable structure matters.".into(),
                params: vec![],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: Some(json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "string" },
                        "label": { "type": "string" },
                        "summary": { "type": "string" },
                        "content": { "type": "string" },
                        "search_text": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["node_id"],
                    "additionalProperties": false
                })),
                output_schema_override: None,
            },
            ToolDefinition {
                name: "relate_nodes".into(),
                description: "Create a graph edge between two nodes.".into(),
                params: vec![],
                returns: "dict".into(),
                examples: vec![],
                enabled: true,
                injected: true,
                input_schema_override: Some(json!({
                    "type": "object",
                    "properties": {
                        "from_node_id": { "type": "string" },
                        "to_node_id": { "type": "string" },
                        "relation": { "type": "string" }
                    },
                    "required": ["from_node_id", "to_node_id", "relation"],
                    "additionalProperties": false
                })),
                output_schema_override: None,
            },
        ]
    }

    async fn execute(&self, name: &str, args: &Value) -> ToolResult {
        match self.run(name, args).await {
            Ok(value) => ToolResult::ok(value),
            Err(error) => ToolResult::err_fmt(error),
        }
    }
}

impl WorkspaceTools {
    async fn run(&self, name: &str, args: &Value) -> Result<Value> {
        match name {
            "list_assets" => Ok(json!({
                "assets": load_assets(&self.state, 20)
                    .await?
                    .into_iter()
                    .map(AssetDto::from)
                    .collect::<Vec<_>>()
            })),
            "read_asset" => {
                let asset_id = require_str(args, "asset_id")?;
                let asset = load_asset(&self.state, asset_id)
                    .await?
                    .ok_or_else(|| anyhow!("asset not found"))?;
                Ok(json!({
                    "asset": AssetDto::from(asset.clone()),
                    "text": asset.extracted_text.unwrap_or_default(),
                    "image_description": asset.image_description,
                }))
            }
            "fetch_url" => {
                let url = require_str(args, "url")?;
                fetch_url_preview(&self.state, url).await
            }
            "search_web" => {
                let query = require_str(args, "query")?;
                search_web(&self.state, query).await
            }
            "search_graph" => {
                let query = require_str(args, "query")?;
                Ok(json!({
                    "results": search_nodes(&self.state, query).await?
                }))
            }
            "get_node" => {
                let node_id = require_str(args, "node_id")?;
                Ok(serde_json::to_value(
                    load_node_detail(&self.state, node_id).await?,
                )?)
            }
            "create_node" => {
                let kind = require_str(args, "kind")?;
                let label = require_str(args, "label")?;
                let node_id = optional_str(args, "node_id");
                let summary = optional_str(args, "summary");
                let content = optional_str(args, "content");
                let search_text = optional_str(args, "search_text");
                let source = optional_str(args, "source");
                let node = create_graph_node(
                    &self.state,
                    kind,
                    label,
                    node_id,
                    summary,
                    content,
                    search_text,
                    source,
                )
                .await?;
                emit_server_event(
                    &self.state,
                    "graph.updated",
                    "kg_node",
                    json!({ "id": node.id }),
                );
                Ok(json!({ "node": node }))
            }
            "update_node" => {
                let node_id = require_str(args, "node_id")?;
                let node = update_node_fields(
                    &self.state,
                    node_id,
                    optional_str(args, "label").map(str::to_string),
                    optional_str(args, "summary").map(str::to_string),
                    optional_str(args, "content").map(str::to_string),
                    optional_str(args, "search_text").map(str::to_string),
                    optional_str(args, "source").map(str::to_string),
                )
                .await?;
                emit_server_event(
                    &self.state,
                    "graph.updated",
                    "kg_node",
                    json!({ "id": node.id }),
                );
                Ok(json!({ "node": node }))
            }
            "relate_nodes" => {
                let from_node_id = require_str(args, "from_node_id")?;
                let to_node_id = require_str(args, "to_node_id")?;
                let relation = require_str(args, "relation")?;
                let edge = create_edge(&self.state, from_node_id, to_node_id, relation).await?;
                emit_server_event(
                    &self.state,
                    "graph.updated",
                    "kg_edge",
                    json!({ "id": edge.id }),
                );
                Ok(json!({ "edge": edge }))
            }
            other => Err(anyhow!("unknown tool: {other}")),
        }
    }
}

async fn fetch_url_preview(state: &AppState, url: &str) -> Result<Value> {
    let parsed = url::Url::parse(url)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(anyhow!("unsupported url scheme"));
    }
    let response = state.http.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!("fetch failed with {}", response.status()));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("text/plain")
        .to_string();
    let body = response.text().await?;
    let preview = if content_type.contains("html") {
        crate::extract::extract_text_from_html(&body)
    } else {
        crate::extract::clean_text(&body)
    };
    Ok(json!({
        "url": url,
        "content_type": content_type,
        "preview": preview.chars().take(4_000).collect::<String>(),
    }))
}

async fn search_web(state: &AppState, query: &str) -> Result<Value> {
    let api_key = state
        .config
        .tavily_api_key
        .clone()
        .ok_or_else(|| anyhow!("TAVILY_API_KEY is not configured"))?;
    let response = state
        .http
        .post("https://api.tavily.com/search")
        .json(&json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "advanced",
            "max_results": 5,
        }))
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("web search failed with {status}: {body}"));
    }
    response.json::<Value>().await.map_err(Into::into)
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{key} is required"))
}

fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn record_id_string(record_id: &surrealdb::types::RecordId) -> String {
    crate::models::record_id_string(record_id)
}
