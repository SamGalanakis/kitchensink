import type {
  ModelSettingsDto,
  NodeDetailDto,
  SearchResultDto,
  ServerEvent,
  SessionDto,
  WorkspaceSnapshotDto,
} from "./types";

async function apiFetch(input: string, init: RequestInit = {}) {
  const response = await fetch(input, {
    ...init,
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...(init.headers ?? {}),
    },
  });

  if (!response.ok) {
    let message = `Request failed with ${response.status}`;
    try {
      const body = (await response.json()) as { error?: string };
      if (body.error) message = body.error;
    } catch {
      // ignore
    }
    throw new Error(message);
  }

  return response;
}

export async function getSession(): Promise<SessionDto> {
  const response = await apiFetch("/api/session");
  return response.json();
}

export async function login(password: string): Promise<SessionDto> {
  const response = await apiFetch("/api/auth/login", {
    method: "POST",
    body: JSON.stringify({ password }),
  });
  return response.json();
}

export async function logout(): Promise<void> {
  await apiFetch("/api/auth/logout", { method: "POST" });
}

export async function getWorkspace(): Promise<WorkspaceSnapshotDto> {
  const response = await apiFetch("/api/workspace");
  return response.json();
}

export async function getNodeDetail(id: string): Promise<NodeDetailDto> {
  const response = await apiFetch(`/api/nodes/${encodeURIComponent(id)}`);
  return response.json();
}

export async function searchNodes(query: string): Promise<SearchResultDto[]> {
  const response = await apiFetch(`/api/search?q=${encodeURIComponent(query)}`);
  return response.json();
}

export async function sendChat(content: string): Promise<void> {
  await apiFetch("/api/chat/send", {
    method: "POST",
    body: JSON.stringify({ content }),
  });
}

export async function stopChat(): Promise<void> {
  await apiFetch("/api/chat/stop", { method: "POST" });
}

export async function updateModelSettings(payload: {
  base_url: string;
  model: string;
  api_key?: string;
  clear_api_key?: boolean;
}): Promise<ModelSettingsDto> {
  const response = await apiFetch("/api/settings/model", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
  return response.json();
}

export async function importUrl(url: string): Promise<void> {
  await apiFetch("/api/import/url", {
    method: "POST",
    body: JSON.stringify({ url }),
  });
}

export async function importFile(file: File): Promise<void> {
  const formData = new FormData();
  formData.append("file", file);
  const response = await fetch("/api/import/upload", {
    method: "POST",
    body: formData,
    credentials: "include",
  });
  if (!response.ok) {
    let message = `Upload failed with ${response.status}`;
    try {
      const body = (await response.json()) as { error?: string };
      if (body.error) message = body.error;
    } catch {
      // ignore
    }
    throw new Error(message);
  }
}

export function openEvents(onEvent: (event: ServerEvent) => void) {
  const source = new EventSource("/api/events", { withCredentials: true });
  source.addEventListener("update", (event) => {
    const data = JSON.parse((event as MessageEvent<string>).data) as ServerEvent;
    onEvent(data);
  });
  return source;
}

export function assetFileUrl(id: string) {
  return `/api/assets/${encodeURIComponent(id)}/file`;
}
