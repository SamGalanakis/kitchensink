import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";

import {
  ArrowUpRight,
  FileText,
  Globe,
  Image,
  Link,
  Lock,
  MessageSquare,
  Search,
  Send,
  Settings,
  Square,
  Upload,
  X,
} from "lucide-solid";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Kbd } from "@/components/ui/kbd";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Alert, AlertTitle } from "@/components/ui/alert";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription, SheetFooter } from "@/components/ui/sheet";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";

import { DocumentSurface } from "./components/DocumentSurface";
import { GraphCanvas } from "./components/GraphCanvas";
import {
  assetFileUrl,
  getNodeDetail,
  getWorkspace,
  importFile,
  importUrl,
  login,
  logout,
  openEvents,
  searchNodes,
  sendChat,
  stopChat,
  updateModelSettings,
} from "./lib/api";
import type {
  AssetDto,
  NodeDetailDto,
  NodeKind,
  SearchResultDto,
  WorkspaceSnapshotDto,
} from "./lib/types";

type PanelMode = "settings" | "import-url" | null;

function formatTime(value: string) {
  return new Date(value).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function fileSizeLabel(bytes: number) {
  if (bytes > 1_000_000) return `${(bytes / 1_000_000).toFixed(1)} MB`;
  if (bytes > 1_000) return `${Math.round(bytes / 1_000)} KB`;
  return `${bytes} B`;
}

function NodeKindGlyph(props: { kind: NodeKind; class?: string }) {
  switch (props.kind) {
    case "image":
      return <Image class={props.class} />;
    case "url":
      return <Globe class={props.class} />;
    case "topic":
      return <Link class={props.class} />;
    default:
      return <FileText class={props.class} />;
  }
}

function graphLegendText(kind: NodeKind) {
  switch (kind) {
    case "document":
      return "brass";
    case "image":
      return "blue";
    case "url":
      return "teal";
    case "topic":
      return "coral";
  }
}

export default function App() {
  const [workspace, setWorkspace] = createSignal<WorkspaceSnapshotDto | null>(null);
  const [selectedId, setSelectedId] = createSignal<string | null>(null);
  const [selectedDetail, setSelectedDetail] = createSignal<NodeDetailDto | null>(null);
  const [bootError, setBootError] = createSignal("");
  const [loginPassword, setLoginPassword] = createSignal("");
  const [loginError, setLoginError] = createSignal("");
  const [chatInput, setChatInput] = createSignal("");
  const [running, setRunning] = createSignal(false);
  const [submittingChat, setSubmittingChat] = createSignal(false);
  const [panelMode, setPanelMode] = createSignal<PanelMode>(null);
  const [commandOpen, setCommandOpen] = createSignal(false);
  const [commandQuery, setCommandQuery] = createSignal("");
  const [commandResults, setCommandResults] = createSignal<SearchResultDto[]>([]);
  const [commandLoading, setCommandLoading] = createSignal(false);
  const [importing, setImporting] = createSignal(false);
  const [urlValue, setUrlValue] = createSignal("");
  const [urlError, setUrlError] = createSignal("");
  const [settingsDraft, setSettingsDraft] = createSignal({
    base_url: "",
    model: "",
    api_key: "",
    clear_api_key: false,
  });
  let refreshTimer: number | undefined;
  let fileInputRef: HTMLInputElement | undefined;
  let eventSource: EventSource | undefined;
  let commandInputRef: HTMLInputElement | undefined;
  let chatEndRef: HTMLDivElement | undefined;

  const assetsById = createMemo(() => {
    const map = new Map<string, AssetDto>();
    for (const asset of workspace()?.assets ?? []) map.set(asset.id, asset);
    return map;
  });

  const selectedAsset = createMemo(() => {
    const detail = selectedDetail();
    return detail?.asset ?? (detail?.node.asset_id ? assetsById().get(detail.node.asset_id) ?? null : null);
  });

  const latestJobs = createMemo(() => (workspace()?.import_jobs ?? []).slice(0, 5));
  const visibleMessages = createMemo(() => workspace()?.chat ?? []);
  const graphSearchQuery = createMemo(() => (commandOpen() ? commandQuery() : ""));

  const scheduleRefresh = () => {
    window.clearTimeout(refreshTimer);
    refreshTimer = window.setTimeout(() => {
      void refreshWorkspace();
    }, 120);
  };

  const disconnectEvents = () => {
    eventSource?.close();
    eventSource = undefined;
  };

  const connectEvents = () => {
    disconnectEvents();
    eventSource = openEvents((event) => {
      if (event.kind === "chat.turn_started") setRunning(true);
      if (event.kind === "chat.turn_finished" || event.kind === "chat.turn_cancelled") {
        setRunning(false);
        setSubmittingChat(false);
      }
      scheduleRefresh();
    });
    eventSource.onerror = () => {
      scheduleRefresh();
    };
  };

  const refreshWorkspace = async () => {
    try {
      const next = await getWorkspace();
      setWorkspace(next);
      if (!selectedId() && next.graph.nodes.length > 0) {
        setSelectedId(next.graph.nodes[0].id);
      }
      if (!eventSource) connectEvents();
    } catch (error) {
      setWorkspace(null);
      disconnectEvents();
      setBootError(error instanceof Error ? error.message : "Failed to load workspace");
    }
  };

  const refreshSelectedDetail = async () => {
    const id = selectedId();
    if (!id) {
      setSelectedDetail(null);
      return;
    }
    try {
      setSelectedDetail(await getNodeDetail(id));
    } catch {
      setSelectedDetail(null);
    }
  };

  const openSettings = () => {
    const settings = workspace()?.settings;
    if (!settings) return;
    setSettingsDraft({
      base_url: settings.base_url,
      model: settings.model,
      api_key: "",
      clear_api_key: false,
    });
    setPanelMode("settings");
  };

  const openUrlImporter = () => {
    setUrlValue("");
    setUrlError("");
    setPanelMode("import-url");
  };

  const commandActions = createMemo(() => [
    {
      id: "action-import-file",
      label: "Import a file",
      description: "Upload a document or image into the graph.",
      icon: Upload,
      run: () => fileInputRef?.click(),
    },
    {
      id: "action-import-url",
      label: "Import from URL",
      description: "Fetch a webpage or image by URL.",
      icon: Globe,
      run: () => openUrlImporter(),
    },
    {
      id: "action-settings",
      label: "Model settings",
      description: "Edit the OpenAI-compatible base URL and model.",
      icon: Settings,
      run: () => openSettings(),
    },
  ]);

  const submitLogin = async () => {
    setLoginError("");
    try {
      await login(loginPassword());
      setBootError("");
      setLoginPassword("");
      await refreshWorkspace();
    } catch (error) {
      setLoginError(error instanceof Error ? error.message : "Login failed");
    }
  };

  const submitChat = async () => {
    const content = chatInput().trim();
    if (!content || running() || submittingChat()) return;
    setSubmittingChat(true);
    setRunning(true);
    try {
      await sendChat(content);
      setChatInput("");
      scheduleRefresh();
    } catch (error) {
      setRunning(false);
      setSubmittingChat(false);
      setBootError(error instanceof Error ? error.message : "Chat failed");
    }
  };

  const submitUrlImport = async () => {
    const url = urlValue().trim();
    if (!url) return;
    setImporting(true);
    setUrlError("");
    try {
      await importUrl(url);
      setPanelMode(null);
      scheduleRefresh();
    } catch (error) {
      setUrlError(error instanceof Error ? error.message : "Import failed");
    } finally {
      setImporting(false);
    }
  };

  const submitSettings = async () => {
    const draft = settingsDraft();
    try {
      const next = await updateModelSettings(draft);
      setWorkspace((current) =>
        current ? { ...current, settings: next } : current,
      );
      setPanelMode(null);
    } catch (error) {
      setBootError(error instanceof Error ? error.message : "Failed to save settings");
    }
  };

  const onFilePicked = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    setImporting(true);
    try {
      await importFile(file);
      scheduleRefresh();
    } catch (error) {
      setBootError(error instanceof Error ? error.message : "Upload failed");
    } finally {
      input.value = "";
      setImporting(false);
    }
  };

  const onLogout = async () => {
    await logout();
    disconnectEvents();
    setWorkspace(null);
    setSelectedId(null);
    setSelectedDetail(null);
  };

  onMount(() => {
    void refreshWorkspace();

    const handleKeydown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen((open) => !open);
      }
      if (event.key === "Escape") {
        setCommandOpen(false);
        setPanelMode(null);
      }
    };

    window.addEventListener("keydown", handleKeydown);
    onCleanup(() => {
      window.removeEventListener("keydown", handleKeydown);
      disconnectEvents();
      window.clearTimeout(refreshTimer);
    });
  });

  createEffect(() => {
    if (!commandOpen()) return;
    queueMicrotask(() => commandInputRef?.focus());
  });

  createEffect(() => {
    const query = commandQuery().trim();
    if (!commandOpen()) return;
    if (!query) {
      setCommandResults([]);
      return;
    }
    const handle = window.setTimeout(async () => {
      setCommandLoading(true);
      try {
        setCommandResults(await searchNodes(query));
      } finally {
        setCommandLoading(false);
      }
    }, 120);
    onCleanup(() => window.clearTimeout(handle));
  });

  createEffect(() => {
    selectedId();
    void refreshSelectedDetail();
  });

  createEffect(() => {
    visibleMessages();
    queueMicrotask(() => chatEndRef?.scrollIntoView({ behavior: "smooth" }));
  });

  const nodeLabel = (id: string) => {
    const node = workspace()?.graph.nodes.find((n) => n.id === id);
    return node?.label ?? id.split(":").pop() ?? id;
  };

  return (
    <div class="app-shell">
      <input
        ref={fileInputRef}
        type="file"
        class="sr-only"
        onChange={onFilePicked}
        accept=".pdf,.txt,.md,.html,.htm,image/*"
      />

      <Show
        when={workspace()}
        fallback={
          /* ── Login Screen ── */
          <div class="flex min-h-screen items-center justify-center p-6">
            <div class="w-full max-w-md space-y-8">
              <div class="text-center space-y-3">
                <div class="eyebrow">Private Graph Surface</div>
                <h1 class="text-5xl sm:text-6xl font-serif font-semibold tracking-tight text-foreground leading-none">
                  Kitchensink
                </h1>
                <p class="text-muted-foreground text-sm leading-relaxed max-w-xs mx-auto">
                  One assistant, one atlas, one persistent knowledge graph for documents, images, URLs, and topics.
                </p>
              </div>

              <Card class="bg-card/80 backdrop-blur-xl">
                <CardContent class="space-y-4 pt-2">
                  <div class="space-y-2">
                    <Label>Password</Label>
                    <Input
                      type="password"
                      value={loginPassword()}
                      onInput={(e) => setLoginPassword(e.currentTarget.value)}
                      onKeyDown={(e) => { if (e.key === "Enter") void submitLogin(); }}
                      placeholder="Enter the workspace password"
                      class="h-10"
                    />
                  </div>
                  <Button
                    class="w-full h-10"
                    onClick={() => void submitLogin()}
                  >
                    <Lock class="size-4" />
                    Enter workspace
                  </Button>
                  <Show when={loginError() || bootError()}>
                    <Alert variant="destructive">
                      <AlertTitle>{loginError() || bootError()}</AlertTitle>
                    </Alert>
                  </Show>
                </CardContent>
              </Card>
            </div>
          </div>
        }
      >
        {(data) => (
          <>
            {/* ── Top Bar ── */}
            <header class="flex items-center gap-3 px-4 py-2.5 border-b border-border/60 bg-background/70 backdrop-blur-lg">
              <div class="flex items-center gap-3 mr-auto">
                <div>
                  <div class="eyebrow leading-none">Knowledge Atlas</div>
                  <span class="text-sm font-semibold font-serif tracking-tight">{data().session.username}</span>
                </div>
                <Separator orientation="vertical" class="h-6 mx-1" />
                <Badge variant="outline" class="gap-1.5 h-6 text-xs">
                  <Settings class="size-3" />
                  {data().settings.model}
                </Badge>
                <Badge variant="secondary" class="gap-1.5 h-6 text-xs">
                  {data().import_jobs.length} imports
                </Badge>
              </div>

              <div class="flex items-center gap-1">
                <Tooltip>
                  <TooltipTrigger as={Button} variant="ghost" size="sm" onClick={() => setCommandOpen(true)}>
                    <Search class="size-3.5" />
                    Search
                    <Kbd class="ml-1">Ctrl K</Kbd>
                  </TooltipTrigger>
                  <TooltipContent>Search nodes or run actions</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger as={Button} variant="ghost" size="icon-sm" onClick={() => fileInputRef?.click()}>
                    <Upload class="size-3.5" />
                  </TooltipTrigger>
                  <TooltipContent>Upload file</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger as={Button} variant="ghost" size="icon-sm" onClick={openUrlImporter}>
                    <Globe class="size-3.5" />
                  </TooltipTrigger>
                  <TooltipContent>Import from URL</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger as={Button} variant="ghost" size="icon-sm" onClick={openSettings}>
                    <Settings class="size-3.5" />
                  </TooltipTrigger>
                  <TooltipContent>Model settings</TooltipContent>
                </Tooltip>
                <Separator orientation="vertical" class="h-5 mx-1" />
                <Tooltip>
                  <TooltipTrigger as={Button} variant="ghost" size="icon-sm" onClick={() => void onLogout()}>
                    <Lock class="size-3.5" />
                  </TooltipTrigger>
                  <TooltipContent>Lock workspace</TooltipContent>
                </Tooltip>
              </div>
            </header>

            {/* ── Main Grid ── */}
            <main class="flex-1 min-h-0 grid grid-cols-[340px_minmax(0,1fr)_340px] gap-3 p-3 max-lg:grid-cols-[300px_minmax(0,1fr)_300px] max-md:grid-cols-1 max-md:h-auto">

              {/* ── Chat Panel ── */}
              <section class="flex flex-col min-h-0 rounded-xl ring-1 ring-foreground/8 bg-card/60 backdrop-blur-md overflow-hidden max-md:min-h-[420px]">
                <div class="flex items-center justify-between px-4 pt-3 pb-2">
                  <div>
                    <div class="eyebrow leading-none">Assistant</div>
                    <h3 class="text-sm font-semibold font-serif tracking-tight mt-0.5">Single thread</h3>
                  </div>
                  <Show when={running()}>
                    <Badge variant="outline" class="gap-1.5 text-blue border-blue/30 bg-blue/8">
                      <Spinner class="size-3 text-blue" />
                      running
                    </Badge>
                  </Show>
                </div>

                <Separator />

                <div class="flex-1 min-h-0 overflow-y-auto px-3 py-3 space-y-2">
                  <For each={visibleMessages()}>
                    {(message) => {
                      const isUser = () => message.role === "user";
                      const isAssistant = () => message.role === "assistant";
                      const isTool = () => message.role === "tool";
                      return (
                        <div class={`rounded-lg px-3 py-2.5 text-sm ring-1 ${
                          isUser()
                            ? "ring-blue/15 bg-blue/5"
                            : isAssistant()
                              ? "ring-gold/15 bg-gold/5"
                              : "ring-foreground/6 bg-muted/30 opacity-80"
                        } ${isTool() ? "border-dashed" : ""}`}>
                          <div class="flex items-center justify-between mb-1.5">
                            <div class="flex items-center gap-1.5">
                              <Show when={isUser()}>
                                <MessageSquare class="size-3 text-blue" />
                              </Show>
                              <Show when={isAssistant()}>
                                <MessageSquare class="size-3 text-gold" />
                              </Show>
                              <span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                                {isTool() ? message.tool_name ?? "tool" : message.role}
                              </span>
                            </div>
                            <span class="font-mono text-[10px] text-muted-foreground">{formatTime(message.created_at)}</span>
                          </div>
                          <pre class="whitespace-pre-wrap text-[13px] leading-relaxed text-foreground m-0 font-sans">{message.content}</pre>
                        </div>
                      );
                    }}
                  </For>
                  <div ref={chatEndRef} />
                </div>

                <div class="border-t border-border/60 p-3 bg-background/60">
                  <Textarea
                    rows={3}
                    value={chatInput()}
                    onInput={(e) => setChatInput(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.shiftKey) {
                        e.preventDefault();
                        void submitChat();
                      }
                    }}
                    placeholder="Ask the assistant to inspect, connect, or annotate..."
                    class="min-h-[80px] text-sm resize-none"
                  />
                  <div class="flex items-center justify-between mt-2">
                    <Show when={bootError()}>
                      <span class="text-danger text-xs truncate mr-2">{bootError()}</span>
                    </Show>
                    <div class="flex items-center gap-1.5 ml-auto">
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={!running()}
                        onClick={() => void stopChat()}
                      >
                        <Square class="size-3" />
                        Stop
                      </Button>
                      <Button
                        size="sm"
                        disabled={running() || submittingChat() || !chatInput().trim()}
                        onClick={() => void submitChat()}
                      >
                        <Send class="size-3" />
                        Send
                      </Button>
                    </div>
                  </div>
                </div>
              </section>

              {/* ── Graph Panel ── */}
              <section class="relative min-h-0 rounded-xl ring-1 ring-foreground/8 overflow-hidden bg-gradient-to-b from-card/50 to-background/80 max-md:min-h-[420px]">
                <div class="absolute inset-x-0 top-0 z-10 flex justify-between items-start p-3 pointer-events-none">
                  <div class="pointer-events-auto rounded-lg bg-background/70 backdrop-blur-md ring-1 ring-foreground/6 px-3 py-2.5 max-w-[280px]">
                    <div class="eyebrow leading-none">Graph</div>
                    <h3 class="text-sm font-semibold font-serif tracking-tight mt-0.5">{data().graph.nodes.length} nodes</h3>
                    <p class="text-xs text-muted-foreground mt-1 leading-relaxed">
                      Documents in {graphLegendText("document")}, images in {graphLegendText("image")}, URLs in {graphLegendText("url")}, topics in {graphLegendText("topic")}. Pan, zoom, inspect.
                    </p>
                  </div>
                </div>

                <GraphCanvas
                  graph={data().graph}
                  selectedId={selectedId()}
                  searchQuery={graphSearchQuery()}
                  onSelect={setSelectedId}
                />

                <div class="absolute inset-x-0 bottom-0 z-10 flex flex-wrap gap-1.5 p-3 pointer-events-none">
                  <For each={latestJobs()}>
                    {(job) => (
                      <button
                        class="pointer-events-auto rounded-lg bg-background/75 backdrop-blur-md ring-1 ring-foreground/8 px-3 py-2 text-left min-w-[160px] max-w-[220px] transition-all hover:ring-gold/30 hover:bg-background/85"
                        type="button"
                        onClick={() => setSelectedId(job.node_id)}
                      >
                        <Badge variant="outline" class="mb-1 text-[10px] h-4">{job.status}</Badge>
                        <div class="text-xs font-medium text-foreground truncate">{job.headline}</div>
                        <div class="text-[10px] text-muted-foreground truncate">{job.detail ?? "Processing..."}</div>
                      </button>
                    )}
                  </For>
                </div>
              </section>

              {/* ── Inspector Panel ── */}
              <aside class="flex flex-col min-h-0 rounded-xl ring-1 ring-foreground/8 bg-card/60 backdrop-blur-md overflow-hidden max-md:min-h-[420px]">
                <Show
                  when={selectedDetail()}
                  fallback={
                    <div class="flex-1 flex flex-col items-center justify-center p-6 text-center">
                      <div class="rounded-xl bg-muted/40 p-4 mb-4">
                        <Search class="size-6 text-muted-foreground" />
                      </div>
                      <div class="eyebrow mb-1">Inspector</div>
                      <h3 class="text-sm font-semibold font-serif tracking-tight">Select a node</h3>
                      <p class="text-xs text-muted-foreground mt-1 max-w-[200px]">Search from the command bar or click a node on the canvas.</p>
                    </div>
                  }
                >
                  {(detail) => (
                    <>
                      <div class="flex items-center justify-between px-4 pt-3 pb-2">
                        <div class="min-w-0">
                          <Badge variant={detail().node.kind === "document" ? "default" : "outline"} class="mb-1 text-[10px] h-4">
                            <NodeKindGlyph kind={detail().node.kind} class="size-2.5" />
                            {detail().node.kind}
                          </Badge>
                          <h3 class="text-sm font-semibold font-serif tracking-tight truncate">{detail().node.label}</h3>
                          <div class="mt-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                            {detail().node.kind}:{detail().node.node_id}
                          </div>
                        </div>
                        <Button variant="ghost" size="icon-xs" onClick={() => setSelectedId(null)}>
                          <X class="size-3.5" />
                        </Button>
                      </div>

                      <Separator />

                      <div class="flex-1 min-h-0 overflow-y-auto p-3 space-y-3">
                        {/* Summary */}
                        <Card size="sm">
                          <CardHeader>
                            <CardTitle class="text-xs font-mono uppercase tracking-wider text-gold">Summary</CardTitle>
                          </CardHeader>
                          <CardContent>
                            <p class="text-sm text-muted-foreground leading-relaxed">{detail().node.summary ?? "No summary yet."}</p>
                          </CardContent>
                        </Card>

                        <Show when={detail().node.content}>
                          <Card size="sm">
                            <CardHeader>
                              <CardTitle class="text-xs font-mono uppercase tracking-wider text-gold">Document</CardTitle>
                              <CardDescription>
                                Rich content with references and embeds.
                              </CardDescription>
                            </CardHeader>
                            <CardContent>
                              <DocumentSurface
                                graph={data().graph}
                                html={detail().node.content!}
                                onNavigate={setSelectedId}
                              />
                            </CardContent>
                          </Card>
                        </Show>

                        {/* Source */}
                        <Card size="sm">
                          <CardHeader>
                            <CardTitle class="text-xs font-mono uppercase tracking-wider text-gold">Source</CardTitle>
                          </CardHeader>
                          <CardContent class="space-y-2">
                            <p class="text-sm text-muted-foreground">{detail().node.source ?? "No source metadata"}</p>
                            <Show when={selectedAsset()?.source_url}>
                              <a
                                href={selectedAsset()!.source_url!}
                                target="_blank"
                                rel="noreferrer"
                                class="inline-flex items-center gap-1 text-xs text-gold hover:underline"
                              >
                                <ArrowUpRight class="size-3" />
                                Open original
                              </a>
                            </Show>
                          </CardContent>
                        </Card>

                        {/* Asset */}
                        <Show when={selectedAsset()}>
                          {(asset) => (
                            <Card size="sm">
                              <CardHeader>
                                <CardTitle class="text-xs font-mono uppercase tracking-wider text-gold">Asset</CardTitle>
                                <CardDescription>
                                  <div class="flex flex-wrap gap-1.5 mt-1">
                                    <Badge variant="outline" class="text-[10px] h-4">{asset().content_type}</Badge>
                                    <Badge variant="outline" class="text-[10px] h-4">{fileSizeLabel(asset().byte_size)}</Badge>
                                    <Badge variant={asset().extraction_status === "ready" ? "default" : "outline"} class="text-[10px] h-4">
                                      {asset().extraction_status}
                                    </Badge>
                                  </div>
                                </CardDescription>
                              </CardHeader>
                              <CardContent>
                                <Show
                                  when={asset().kind === "image"}
                                  fallback={
                                    <div class="space-y-2">
                                      <Show when={!detail().node.content}>
                                        <pre class="text-xs text-muted-foreground whitespace-pre-wrap leading-relaxed max-h-[200px] overflow-y-auto rounded-lg bg-muted/30 p-2.5 m-0 font-sans">
                                          {detail().node.search_text ?? "No extracted text yet."}
                                        </pre>
                                      </Show>
                                      <div class="flex flex-wrap gap-2">
                                        <a
                                          href={assetFileUrl(asset().id)}
                                          target="_blank"
                                          rel="noreferrer"
                                          class="inline-flex items-center gap-1 text-xs text-gold hover:underline"
                                        >
                                          <ArrowUpRight class="size-3" />
                                          Open file
                                        </a>
                                        <Show when={asset().source_url}>
                                          <a
                                            href={asset().source_url!}
                                            target="_blank"
                                            rel="noreferrer"
                                            class="inline-flex items-center gap-1 text-xs text-gold hover:underline"
                                          >
                                            <ArrowUpRight class="size-3" />
                                            Open source
                                          </a>
                                        </Show>
                                      </div>
                                    </div>
                                  }
                                >
                                  <img
                                    class="w-full max-h-[240px] object-cover rounded-lg ring-1 ring-foreground/6"
                                    src={assetFileUrl(asset().id)}
                                    alt={asset().label}
                                  />
                                </Show>
                              </CardContent>
                            </Card>
                          )}
                        </Show>

                        {/* Connections */}
                        <Card size="sm">
                          <CardHeader>
                            <CardTitle class="text-xs font-mono uppercase tracking-wider text-gold">Connections</CardTitle>
                            <CardDescription>
                              {[...detail().incoming, ...detail().outgoing].length} edges
                            </CardDescription>
                          </CardHeader>
                          <CardContent>
                            <div class="flex flex-wrap gap-1.5">
                              <For each={[...detail().incoming, ...detail().outgoing]}>
                                {(edge) => {
                                  const targetId = () => edge.in === detail().node.id ? edge.out : edge.in;
                                  return (
                                    <button
                                      class="inline-flex items-center gap-1.5 rounded-md ring-1 ring-foreground/8 bg-muted/30 px-2 py-1 text-xs transition-colors hover:ring-gold/30 hover:bg-muted/50"
                                      type="button"
                                      onClick={() => setSelectedId(targetId())}
                                    >
                                      <Link class="size-2.5 text-muted-foreground" />
                                      <span class="text-muted-foreground">{edge.relation}</span>
                                      <span class="font-medium text-foreground truncate max-w-[120px]">{nodeLabel(targetId())}</span>
                                    </button>
                                  );
                                }}
                              </For>
                            </div>
                          </CardContent>
                        </Card>
                      </div>
                    </>
                  )}
                </Show>
              </aside>
            </main>

            {/* ── Command Palette ── */}
            <Dialog open={commandOpen()} onOpenChange={setCommandOpen}>
              <DialogContent showCloseButton={false} class="sm:max-w-2xl top-[18vh] -translate-y-0">
                <div class="relative">
                  <Search class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
                  <Input
                    ref={commandInputRef}
                    value={commandQuery()}
                    onInput={(e) => setCommandQuery(e.currentTarget.value)}
                    placeholder="Search nodes or run an action..."
                    class="pl-9 h-11 text-base rounded-lg"
                  />
                </div>

                <div class="max-h-[50vh] overflow-y-auto space-y-4">
                  {/* Actions */}
                  <div>
                    <div class="flex items-center justify-between mb-2">
                      <span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">Actions</span>
                    </div>
                    <div class="space-y-1">
                      <For each={commandActions()}>
                        {(action) => (
                          <button
                            class="w-full flex items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-muted/50 ring-1 ring-transparent hover:ring-foreground/8"
                            type="button"
                            onClick={() => {
                              action.run();
                              setCommandOpen(false);
                            }}
                          >
                            <div class="flex items-center justify-center size-7 rounded-md bg-muted/50">
                              <action.icon class="size-3.5 text-muted-foreground" />
                            </div>
                            <div class="min-w-0">
                              <div class="text-sm font-medium text-foreground">{action.label}</div>
                              <div class="text-xs text-muted-foreground truncate">{action.description}</div>
                            </div>
                          </button>
                        )}
                      </For>
                    </div>
                  </div>

                  {/* Search Results */}
                  <div>
                    <div class="flex items-center justify-between mb-2">
                      <span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">Search results</span>
                      <Show when={commandLoading()}>
                        <Spinner class="size-3" />
                      </Show>
                    </div>
                    <div class="space-y-1">
                      <For each={commandResults()}>
                        {(result) => (
                          <button
                            class="w-full flex items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-muted/50 ring-1 ring-transparent hover:ring-foreground/8"
                            type="button"
                            onClick={() => {
                              setSelectedId(result.id);
                              setCommandOpen(false);
                            }}
                          >
                            <div class="flex items-center justify-center size-7 rounded-md bg-muted/50">
                              <NodeKindGlyph
                                kind={result.kind}
                                class={`size-3.5 ${
                                  result.kind === "image"
                                    ? "text-blue"
                                    : result.kind === "url"
                                      ? "text-cyan-300"
                                      : result.kind === "topic"
                                        ? "text-rose-300"
                                        : "text-gold-soft"
                                }`}
                              />
                            </div>
                            <div class="min-w-0">
                              <div class="text-sm font-medium text-foreground truncate">{result.label}</div>
                              <div class="text-xs text-muted-foreground truncate">{result.summary ?? `${result.kind}:${result.node_id}`}</div>
                            </div>
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                </div>
              </DialogContent>
            </Dialog>

            {/* ── Settings Sheet ── */}
            <Sheet open={panelMode() === "settings"} onOpenChange={(open) => { if (!open) setPanelMode(null); }}>
              <SheetContent>
                <SheetHeader>
                  <div class="eyebrow">Model Settings</div>
                  <SheetTitle>OpenAI-compatible runtime</SheetTitle>
                  <SheetDescription>Configure the LLM endpoint the assistant uses for reasoning.</SheetDescription>
                </SheetHeader>

                <div class="flex-1 overflow-y-auto px-5 space-y-4">
                  <div class="space-y-2">
                    <Label>Base URL</Label>
                    <Input
                      value={settingsDraft().base_url}
                      onInput={(e) => setSettingsDraft((d) => ({ ...d, base_url: e.currentTarget.value }))}
                      class="h-9"
                    />
                  </div>
                  <div class="space-y-2">
                    <Label>Model</Label>
                    <Input
                      value={settingsDraft().model}
                      onInput={(e) => setSettingsDraft((d) => ({ ...d, model: e.currentTarget.value }))}
                      class="h-9"
                    />
                  </div>
                  <div class="space-y-2">
                    <Label>API key</Label>
                    <Input
                      type="password"
                      placeholder={workspace()?.settings.api_key_present ? "Leave blank to keep saved key" : "Paste API key"}
                      value={settingsDraft().api_key}
                      onInput={(e) => setSettingsDraft((d) => ({ ...d, api_key: e.currentTarget.value }))}
                      class="h-9"
                    />
                  </div>
                  <div class="flex items-center gap-3">
                    <Switch
                      checked={settingsDraft().clear_api_key}
                      onChange={(checked) => setSettingsDraft((d) => ({ ...d, clear_api_key: checked }))}
                    />
                    <Label class="text-sm text-muted-foreground cursor-pointer">Clear the saved API key</Label>
                  </div>
                </div>

                <SheetFooter class="flex-row justify-end gap-2">
                  <Button variant="ghost" onClick={() => setPanelMode(null)}>Cancel</Button>
                  <Button onClick={() => void submitSettings()}>Save settings</Button>
                </SheetFooter>
              </SheetContent>
            </Sheet>

            {/* ── URL Import Sheet ── */}
            <Sheet open={panelMode() === "import-url"} onOpenChange={(open) => { if (!open) setPanelMode(null); }}>
              <SheetContent>
                <SheetHeader>
                  <div class="eyebrow">Remote Import</div>
                  <SheetTitle>Pull a webpage or image URL</SheetTitle>
                  <SheetDescription>The page will be fetched, parsed, and added as a new node in your knowledge graph.</SheetDescription>
                </SheetHeader>

                <div class="flex-1 overflow-y-auto px-5 space-y-4">
                  <div class="space-y-2">
                    <Label>URL</Label>
                    <Input
                      value={urlValue()}
                      onInput={(e) => setUrlValue(e.currentTarget.value)}
                      placeholder="https://example.com/article"
                      class="h-9"
                    />
                  </div>
                  <Show when={urlError()}>
                    <Alert variant="destructive">
                      <AlertTitle>{urlError()}</AlertTitle>
                    </Alert>
                  </Show>
                </div>

                <SheetFooter class="flex-row justify-end gap-2">
                  <Button variant="ghost" onClick={() => setPanelMode(null)}>Cancel</Button>
                  <Button disabled={importing()} onClick={() => void submitUrlImport()}>
                    <Show when={importing()} fallback={<><Globe class="size-3.5" /> Import URL</>}>
                      <Spinner class="size-3.5" /> Importing...
                    </Show>
                  </Button>
                </SheetFooter>
              </SheetContent>
            </Sheet>
          </>
        )}
      </Show>
    </div>
  );
}
