use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue, ToSql};

pub const KIND_DOCUMENT: &str = "document";
pub const KIND_IMAGE: &str = "image";
pub const KIND_URL: &str = "url";
pub const KIND_TOPIC: &str = "topic";

pub const SOURCE_UPLOAD: &str = "upload";
pub const SOURCE_URL: &str = "url";

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredAuthUser {
    pub id: RecordId,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredAuthSession {
    pub id: RecordId,
    pub user_id: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredModelSettings {
    pub id: RecordId,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredAgentSession {
    pub id: RecordId,
    pub state: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredAsset {
    pub id: RecordId,
    pub kind: String,
    pub source_kind: String,
    pub label: String,
    pub filename: Option<String>,
    pub source_url: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    pub storage_key: String,
    pub extraction_status: String,
    pub extracted_text: Option<String>,
    pub image_description: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredImportJob {
    pub id: RecordId,
    pub asset_id: String,
    pub node_id: String,
    pub headline: String,
    pub detail: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredGraphNode {
    pub id: RecordId,
    pub kind: String,
    pub node_id: String,
    pub label: String,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub search_text: Option<String>,
    pub source: Option<String>,
    pub asset_id: Option<RecordId>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredGraphEdge {
    pub id: RecordId,
    pub relation: String,
    #[serde(rename = "in")]
    pub in_record: RecordId,
    pub out: RecordId,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct StoredChatMessage {
    pub id: RecordId,
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub meta: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
#[surreal(crate = "surrealdb::types")]
pub struct CountRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDto {
    pub username: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelSettingsDto {
    pub base_url: String,
    pub model: String,
    pub api_key_present: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetDto {
    pub id: String,
    pub kind: String,
    pub source_kind: String,
    pub label: String,
    pub filename: Option<String>,
    pub source_url: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    pub extraction_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportJobDto {
    pub id: String,
    pub asset_id: String,
    pub node_id: String,
    pub headline: String,
    pub detail: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNodeDto {
    pub id: String,
    pub kind: String,
    pub node_id: String,
    pub label: String,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub search_text: Option<String>,
    pub source: Option<String>,
    pub asset_id: Option<String>,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdgeDto {
    pub id: String,
    pub relation: String,
    #[serde(rename = "in")]
    pub in_record: String,
    pub out: String,
    pub metadata: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub meta: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResultDto {
    pub id: String,
    pub kind: String,
    pub node_id: String,
    pub label: String,
    pub summary: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceSnapshotDto {
    pub session: SessionDto,
    pub settings: ModelSettingsDto,
    pub assets: Vec<AssetDto>,
    pub import_jobs: Vec<ImportJobDto>,
    pub chat: Vec<ChatMessageDto>,
    pub graph: GraphDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphDto {
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<GraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeDetailDto {
    pub node: GraphNodeDto,
    pub incoming: Vec<GraphEdgeDto>,
    pub outgoing: Vec<GraphEdgeDto>,
    pub asset: Option<AssetDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateModelSettingsRequest {
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrlImportRequest {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatSendRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvent {
    pub kind: String,
    pub entity: String,
    pub payload: Value,
}

impl From<StoredModelSettings> for ModelSettingsDto {
    fn from(value: StoredModelSettings) -> Self {
        Self {
            base_url: value.base_url,
            model: value.model,
            api_key_present: value.api_key.is_some(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<StoredAsset> for AssetDto {
    fn from(value: StoredAsset) -> Self {
        Self {
            id: record_id_string(&value.id),
            kind: value.kind,
            source_kind: value.source_kind,
            label: value.label,
            filename: value.filename,
            source_url: value.source_url,
            content_type: value.content_type,
            byte_size: value.byte_size,
            extraction_status: value.extraction_status,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<StoredImportJob> for ImportJobDto {
    fn from(value: StoredImportJob) -> Self {
        Self {
            id: record_id_string(&value.id),
            asset_id: value.asset_id,
            node_id: value.node_id,
            headline: value.headline,
            detail: value.detail,
            status: value.status,
            error: value.error,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<StoredGraphNode> for GraphNodeDto {
    fn from(value: StoredGraphNode) -> Self {
        Self {
            id: record_id_string(&value.id),
            kind: value.kind,
            node_id: value.node_id,
            label: value.label,
            summary: value.summary,
            content: value.content,
            search_text: value.search_text,
            source: value.source,
            asset_id: value.asset_id.map(|id| record_id_string(&id)),
            metadata: value.metadata,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<StoredGraphEdge> for GraphEdgeDto {
    fn from(value: StoredGraphEdge) -> Self {
        Self {
            id: record_id_string(&value.id),
            relation: value.relation,
            in_record: record_id_string(&value.in_record),
            out: record_id_string(&value.out),
            metadata: value.metadata,
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

impl From<StoredChatMessage> for ChatMessageDto {
    fn from(value: StoredChatMessage) -> Self {
        Self {
            id: record_id_string(&value.id),
            role: value.role,
            content: value.content,
            tool_name: value.tool_name,
            meta: value.meta,
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

pub fn record_id_string(record_id: &RecordId) -> String {
    format!(
        "{}:{}",
        record_id.table.as_str(),
        record_key_string(&record_id.key)
    )
}

pub fn record_key_string(key: &RecordIdKey) -> String {
    match key {
        RecordIdKey::Number(value) => value.to_string(),
        RecordIdKey::String(value) => value.clone(),
        RecordIdKey::Uuid(value) => value.to_string(),
        other => other.to_sql(),
    }
}
