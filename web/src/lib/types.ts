export type AssetKind = "document" | "image" | "url";
export type NodeKind = "document" | "image" | "url" | "topic";

export interface SessionDto {
  username: string;
}

export interface ModelSettingsDto {
  base_url: string;
  model: string;
  api_key_present: boolean;
  updated_at: string;
}

export interface AssetDto {
  id: string;
  kind: AssetKind;
  source_kind: "upload" | "url";
  label: string;
  filename: string | null;
  source_url: string | null;
  content_type: string;
  byte_size: number;
  extraction_status: string;
  created_at: string;
  updated_at: string;
}

export interface ImportJobDto {
  id: string;
  asset_id: string;
  node_id: string;
  headline: string;
  detail: string | null;
  status: string;
  error: string | null;
  created_at: string;
  updated_at: string;
}

export interface GraphNodeDto {
  id: string;
  kind: NodeKind;
  node_id: string;
  label: string;
  summary: string | null;
  content: string | null;
  search_text: string | null;
  source: string | null;
  asset_id: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface GraphEdgeDto {
  id: string;
  relation: string;
  in: string;
  out: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface GraphDto {
  nodes: GraphNodeDto[];
  edges: GraphEdgeDto[];
}

export interface ChatMessageDto {
  id: string;
  role: "user" | "assistant" | "tool" | string;
  content: string;
  tool_name: string | null;
  meta: Record<string, unknown>;
  created_at: string;
}

export interface SearchResultDto {
  id: string;
  kind: NodeKind;
  node_id: string;
  label: string;
  summary: string | null;
  score: number;
}

export interface NodeDetailDto {
  node: GraphNodeDto;
  incoming: GraphEdgeDto[];
  outgoing: GraphEdgeDto[];
  asset: AssetDto | null;
}

export interface WorkspaceSnapshotDto {
  session: SessionDto;
  settings: ModelSettingsDto;
  assets: AssetDto[];
  import_jobs: ImportJobDto[];
  chat: ChatMessageDto[];
  graph: GraphDto;
}

export interface ServerEvent {
  kind: string;
  entity: string;
  payload: Record<string, unknown>;
}
