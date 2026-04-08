import type { GraphDto, GraphNodeDto } from "./types";

type GraphContext = {
  graph: GraphDto;
  nodesByKey: Map<string, GraphNodeDto>;
};

const graphContexts = new Map<string, GraphContext>();
const VALID_TONES = new Set(["default", "muted", "info", "success", "warning", "danger"]);

let nextTabsId = 0;

function normalizeTone(value: string | null): string {
  const tone = value?.trim().toLowerCase() ?? "";
  return VALID_TONES.has(tone) ? tone : "default";
}

function escapeHtml(value: string | null): string {
  const text = value ?? "";
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function nodeKey(kind: string, nodeId: string): string {
  return `${kind}:${nodeId}`;
}

function parseNodeAttr(value: string | null): { kind: string; nodeId: string } | null {
  const raw = value?.trim() ?? "";
  const [kind, nodeId] = raw.split(":", 2);
  if (!kind || !nodeId) {
    return null;
  }
  return {
    kind: kind.trim().toLowerCase(),
    nodeId: nodeId.trim().toLowerCase(),
  };
}

function renderNodeError(message: string): string {
  return `<div class="hirsel-code-empty" data-tone="danger">${escapeHtml(message)}</div>`;
}

function currentGraphContext(element: Element): GraphContext | null {
  const root = element.closest<HTMLElement>("[data-hirsel-graph-root]");
  const rootId = root?.dataset.hirselGraphRoot;
  if (!rootId) {
    return null;
  }
  return graphContexts.get(rootId) ?? null;
}

function resolveNode(element: Element, rawNode: string | null) {
  const context = currentGraphContext(element);
  if (!context) {
    return { error: "Missing graph context.", node: null, parsed: null } as const;
  }
  const parsed = parseNodeAttr(rawNode);
  if (!parsed) {
    return { error: "Invalid node reference. Expected kind:node_id.", node: null, parsed: null } as const;
  }
  const node = context.nodesByKey.get(nodeKey(parsed.kind, parsed.nodeId)) ?? null;
  if (!node) {
    return {
      error: `Unknown graph node ${parsed.kind}:${parsed.nodeId}.`,
      node: null,
      parsed,
    } as const;
  }
  return { error: null, node, parsed } as const;
}

function navigateTo(element: Element, kind: string, nodeId: string): void {
  element.dispatchEvent(
    new CustomEvent("hirsel-navigate-node", {
      bubbles: true,
      detail: { kind, nodeId },
    }),
  );
}

class HirselCardElement extends HTMLElement {
  private bodyHtml = "";
  private initialized = false;

  static get observedAttributes(): string[] {
    return ["tone", "eyebrow", "heading"];
  }

  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    if (this.initialized) this.render();
  }

  private captureBody(): void {
    if (this.initialized) return;
    this.bodyHtml = this.innerHTML.trim();
    this.initialized = true;
  }

  private render(): void {
    this.captureBody();
    const tone = normalizeTone(this.getAttribute("tone"));
    const eyebrow = this.getAttribute("eyebrow");
    const heading = this.getAttribute("heading");
    const header = eyebrow || heading
      ? `
        <div class="hirsel-card-header">
          ${eyebrow ? `<div class="hirsel-card-eyebrow">${escapeHtml(eyebrow)}</div>` : ""}
          ${heading ? `<div class="hirsel-card-heading">${escapeHtml(heading)}</div>` : ""}
        </div>
      `
      : "";

    this.dataset.hirselReady = "true";
    this.innerHTML = `
      <div class="hirsel-card-shell" data-tone="${tone}">
        ${header}
        <div class="hirsel-card-body">${this.bodyHtml}</div>
      </div>
    `;
  }
}

class HirselCalloutElement extends HTMLElement {
  private bodyHtml = "";
  private initialized = false;

  static get observedAttributes(): string[] {
    return ["tone", "title"];
  }

  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    if (this.initialized) this.render();
  }

  private captureBody(): void {
    if (this.initialized) return;
    this.bodyHtml = this.innerHTML.trim();
    this.initialized = true;
  }

  private render(): void {
    this.captureBody();
    const tone = normalizeTone(this.getAttribute("tone"));
    const title = this.getAttribute("title");
    this.dataset.hirselReady = "true";
    this.innerHTML = `
      <div class="hirsel-callout-shell" data-tone="${tone}">
        ${title ? `<div class="hirsel-callout-title">${escapeHtml(title)}</div>` : ""}
        <div class="hirsel-callout-body">${this.bodyHtml}</div>
      </div>
    `;
  }
}

class HirselStatGridElement extends HTMLElement {
  static get observedAttributes(): string[] {
    return ["min"];
  }

  connectedCallback(): void {
    this.applyAttributes();
  }

  attributeChangedCallback(): void {
    this.applyAttributes();
  }

  private applyAttributes(): void {
    const min = Number(this.getAttribute("min"));
    if (Number.isFinite(min) && min > 0) {
      this.style.setProperty("--hirsel-stat-min", `${min}px`);
    } else {
      this.style.removeProperty("--hirsel-stat-min");
    }
    this.dataset.hirselReady = "true";
  }
}

class HirselStatElement extends HTMLElement {
  private bodyHtml = "";
  private initialized = false;

  static get observedAttributes(): string[] {
    return ["tone", "label", "value", "detail"];
  }

  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    if (this.initialized) this.render();
  }

  private captureBody(): void {
    if (this.initialized) return;
    this.bodyHtml = this.innerHTML.trim();
    this.initialized = true;
  }

  private render(): void {
    this.captureBody();
    const tone = normalizeTone(this.getAttribute("tone"));
    const label = this.getAttribute("label");
    const value = this.getAttribute("value") ?? "";
    const detail = this.getAttribute("detail");
    const footer = detail || this.bodyHtml
      ? `<div class="hirsel-stat-detail">${detail ? escapeHtml(detail) : this.bodyHtml}</div>`
      : "";

    this.dataset.hirselReady = "true";
    this.innerHTML = `
      <div class="hirsel-stat-shell" data-tone="${tone}">
        ${label ? `<div class="hirsel-stat-label">${escapeHtml(label)}</div>` : ""}
        <div class="hirsel-stat-value">${escapeHtml(value)}</div>
        ${footer}
      </div>
    `;
  }
}

type TabsPanel = {
  label: string;
  body: string;
  value: string;
};

class HirselTabsElement extends HTMLElement {
  private panels: TabsPanel[] = [];
  private activeIndex = 0;
  private initialized = false;
  private listening = false;
  private readonly tabsId = `hirsel-tabs-${++nextTabsId}`;

  static get observedAttributes(): string[] {
    return ["active", "tone"];
  }

  connectedCallback(): void {
    if (!this.initialized) {
      this.capturePanels();
      this.initialized = true;
    }
    if (!this.listening) {
      this.addEventListener("click", this.handleClick);
      this.addEventListener("keydown", this.handleKeyDown);
      this.listening = true;
    }
    this.syncActiveFromAttribute();
    this.render();
  }

  disconnectedCallback(): void {
    this.removeEventListener("click", this.handleClick);
    this.removeEventListener("keydown", this.handleKeyDown);
    this.listening = false;
  }

  attributeChangedCallback(): void {
    if (!this.initialized) return;
    this.syncActiveFromAttribute();
    this.render();
  }

  private capturePanels(): void {
    const sourcePanels = Array.from(this.children);
    this.panels = sourcePanels
      .map((element, index) => {
        const label =
          element.getAttribute("label") ||
          element.getAttribute("data-label") ||
          element.getAttribute("title") ||
          `Section ${index + 1}`;
        const value =
          element.getAttribute("value") ||
          label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") ||
          `tab-${index + 1}`;
        return {
          label,
          value,
          body: element.innerHTML.trim(),
        };
      })
      .filter((panel) => panel.label);
  }

  private syncActiveFromAttribute(): void {
    if (this.panels.length === 0) return;
    const active = this.getAttribute("active");
    if (!active) {
      this.activeIndex = clamp(this.activeIndex, 0, this.panels.length - 1);
      return;
    }

    const numeric = Number.parseInt(active, 10);
    if (Number.isFinite(numeric)) {
      this.activeIndex = clamp(numeric, 0, this.panels.length - 1);
      return;
    }

    const byValue = this.panels.findIndex((panel) => panel.value === active);
    if (byValue >= 0) {
      this.activeIndex = byValue;
      return;
    }

    const byLabel = this.panels.findIndex(
      (panel) => panel.label.toLowerCase() === active.toLowerCase(),
    );
    if (byLabel >= 0) {
      this.activeIndex = byLabel;
    }
  }

  private render(): void {
    if (this.panels.length === 0) {
      this.dataset.hirselReady = "true";
      return;
    }

    const tone = normalizeTone(this.getAttribute("tone"));
    const buttons = this.panels
      .map((panel, index) => {
        const selected = index === this.activeIndex;
        return `
          <button
            type="button"
            class="hirsel-tab-button"
            data-index="${index}"
            id="${this.tabsId}-tab-${index}"
            role="tab"
            aria-selected="${selected}"
            aria-controls="${this.tabsId}-panel-${index}"
            tabindex="${selected ? "0" : "-1"}"
          >
            ${escapeHtml(panel.label)}
          </button>
        `;
      })
      .join("");

    const panels = this.panels
      .map((panel, index) => {
        const selected = index === this.activeIndex;
        return `
          <section
            class="hirsel-tabs-panel"
            id="${this.tabsId}-panel-${index}"
            role="tabpanel"
            aria-labelledby="${this.tabsId}-tab-${index}"
            ${selected ? "" : "hidden"}
          >
            ${panel.body}
          </section>
        `;
      })
      .join("");

    this.dataset.hirselReady = "true";
    this.innerHTML = `
      <div class="hirsel-tabs-shell" data-tone="${tone}">
        <div class="hirsel-tabs-nav" role="tablist" aria-label="Document sections">
          ${buttons}
        </div>
        <div class="hirsel-tabs-body">
          ${panels}
        </div>
      </div>
    `;
  }

  private handleClick = (event: Event): void => {
    const target = event.target as HTMLElement | null;
    const button = target?.closest<HTMLButtonElement>(".hirsel-tab-button[data-index]");
    if (!button) return;
    const index = Number.parseInt(button.dataset.index ?? "", 10);
    if (!Number.isFinite(index)) return;
    this.activeIndex = clamp(index, 0, this.panels.length - 1);
    this.render();
  };

  private handleKeyDown = (event: Event): void => {
    const keyboardEvent = event as KeyboardEvent;
    if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(keyboardEvent.key)) {
      return;
    }

    keyboardEvent.preventDefault();
    if (keyboardEvent.key === "Home") {
      this.activeIndex = 0;
    } else if (keyboardEvent.key === "End") {
      this.activeIndex = this.panels.length - 1;
    } else if (keyboardEvent.key === "ArrowRight") {
      this.activeIndex = (this.activeIndex + 1) % this.panels.length;
    } else if (keyboardEvent.key === "ArrowLeft") {
      this.activeIndex = (this.activeIndex - 1 + this.panels.length) % this.panels.length;
    }

    this.render();
    queueMicrotask(() => {
      const activeButton = this.querySelector<HTMLButtonElement>(
        `.hirsel-tab-button[data-index="${this.activeIndex}"]`,
      );
      activeButton?.focus();
    });
  };
}

class HirselDisclosureElement extends HTMLElement {
  private bodyHtml = "";
  private initialized = false;

  static get observedAttributes(): string[] {
    return ["title", "tone", "open"];
  }

  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    if (this.initialized) this.render();
  }

  private captureBody(): void {
    if (this.initialized) return;
    this.bodyHtml = this.innerHTML.trim();
    this.initialized = true;
  }

  private render(): void {
    this.captureBody();
    const tone = normalizeTone(this.getAttribute("tone"));
    const title = this.getAttribute("title") || "Details";
    const open = this.hasAttribute("open") ? "open" : "";

    this.dataset.hirselReady = "true";
    this.innerHTML = `
      <details class="hirsel-disclosure-shell" data-tone="${tone}" ${open}>
        <summary class="hirsel-disclosure-summary">
          <span class="hirsel-disclosure-title">${escapeHtml(title)}</span>
          <span class="hirsel-disclosure-chevron" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </span>
        </summary>
        <div class="hirsel-disclosure-body">${this.bodyHtml}</div>
      </details>
    `;
  }
}

class HirselProgressElement extends HTMLElement {
  private bodyHtml = "";
  private initialized = false;

  static get observedAttributes(): string[] {
    return ["tone", "label", "detail", "value", "max"];
  }

  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    if (this.initialized) this.render();
  }

  private captureBody(): void {
    if (this.initialized) return;
    this.bodyHtml = this.innerHTML.trim();
    this.initialized = true;
  }

  private render(): void {
    this.captureBody();
    const tone = normalizeTone(this.getAttribute("tone"));
    const label = this.getAttribute("label");
    const detail = this.getAttribute("detail");
    const rawValue = Number.parseFloat(this.getAttribute("value") ?? "0");
    const rawMax = Number.parseFloat(this.getAttribute("max") ?? "100");
    const max = Number.isFinite(rawMax) && rawMax > 0 ? rawMax : 100;
    const value = Number.isFinite(rawValue) ? clamp(rawValue, 0, max) : 0;
    const percent = max === 0 ? 0 : (value / max) * 100;
    const footer = this.bodyHtml
      ? `<div class="hirsel-progress-footer">${this.bodyHtml}</div>`
      : "";

    this.dataset.hirselReady = "true";
    this.innerHTML = `
      <div class="hirsel-progress-shell" data-tone="${tone}">
        ${(label || detail) ? `
          <div class="hirsel-progress-meta">
            ${label ? `<span class="hirsel-progress-label">${escapeHtml(label)}</span>` : ""}
            ${detail ? `<span class="hirsel-progress-detail">${escapeHtml(detail)}</span>` : ""}
          </div>
        ` : ""}
        <div class="hirsel-progress-track">
          <div class="hirsel-progress-fill" style="width:${percent}%"></div>
        </div>
        ${footer}
      </div>
    `;
  }
}

class HirselNodeRefElement extends HTMLElement {
  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    this.render();
  }

  static get observedAttributes(): string[] {
    return ["node"];
  }

  protected render(): void {
    const resolved = resolveNode(this, this.getAttribute("node"));
    if (resolved.error || !resolved.node || !resolved.parsed) {
      this.innerHTML = renderNodeError(resolved.error ?? "Unknown graph node.");
      return;
    }

    const { node, parsed } = resolved;
    this.dataset.hirselReady = "true";
    if (parsed.kind === "document") {
      const summary = node.summary ? escapeHtml(node.summary) : "";
      this.innerHTML = `<div class="hirsel-doc-link-card" data-node="${escapeHtml(this.getAttribute("node"))}">
        <div class="hirsel-doc-link-head">
          <span class="hirsel-doc-link-kind">${escapeHtml(parsed.kind)}</span>
          <span class="hirsel-doc-link-title">${escapeHtml(node.label || parsed.nodeId)}</span>
        </div>
        ${summary ? `<div class="hirsel-doc-link-summary">${summary}</div>` : ""}
      </div>`;
      this.querySelector(".hirsel-doc-link-card")?.addEventListener("click", () => {
        navigateTo(this, parsed.kind, parsed.nodeId);
      });
      return;
    }

    this.innerHTML = `<button type="button" class="hirsel-fileref-pill">
      <span class="hirsel-fileref-label">${escapeHtml(node.label || parsed.nodeId)}</span>
      <span class="hirsel-fileref-path">${escapeHtml(parsed.kind)}</span>
    </button>`;
    this.querySelector(".hirsel-fileref-pill")?.addEventListener("click", () => {
      navigateTo(this, parsed.kind, parsed.nodeId);
    });
  }
}

class HirselNodeFieldElement extends HTMLElement {
  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    this.render();
  }

  static get observedAttributes(): string[] {
    return ["field", "node"];
  }

  private render(): void {
    const resolved = resolveNode(this, this.getAttribute("node"));
    const field = this.getAttribute("field")?.trim() ?? "";
    if (resolved.error || !resolved.node || !resolved.parsed) {
      this.innerHTML = renderNodeError(resolved.error ?? "Unknown graph node.");
      return;
    }
    if (!field) {
      this.innerHTML = renderNodeError("hirsel-node-field requires node and field attributes.");
      return;
    }

    const value = (resolved.node as unknown as Record<string, unknown>)[field];
    if (value == null) {
      this.innerHTML = renderNodeError(`Field '${field}' not found on ${resolved.parsed.kind}:${resolved.parsed.nodeId}.`);
      return;
    }

    const text = typeof value === "string" ? value : JSON.stringify(value);
    this.dataset.hirselReady = "true";
    if (field === "content" && text.trimStart().startsWith("<")) {
      this.innerHTML = `<div class="hirsel-doc-embed-shell">
        <div class="hirsel-doc-embed-header">
          <span class="hirsel-doc-link-kind">${escapeHtml(resolved.parsed.kind)}</span>
          ${escapeHtml(resolved.node.label || resolved.parsed.nodeId)}
        </div>
        <div class="hirsel-doc-embed-body">${text}</div>
      </div>`;
      return;
    }

    this.innerHTML = `<span>${escapeHtml(text)}</span>`;
  }
}

class HirselNodeListElement extends HTMLElement {
  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    this.render();
  }

  static get observedAttributes(): string[] {
    return ["node", "relation"];
  }

  private render(): void {
    const context = currentGraphContext(this);
    if (!context) {
      this.innerHTML = renderNodeError("Missing graph context.");
      return;
    }

    const resolved = resolveNode(this, this.getAttribute("node"));
    const relation = this.getAttribute("relation")?.trim() ?? "";
    if (resolved.error || !resolved.node || !resolved.parsed) {
      this.innerHTML = renderNodeError(resolved.error ?? "Unknown graph node.");
      return;
    }
    if (!relation) {
      this.innerHTML = renderNodeError("hirsel-node-list requires node and relation attributes.");
      return;
    }

    const related = context.graph.edges
      .filter((edge) => edge.relation === relation)
      .flatMap((edge) => {
        if (edge.out === resolved.node!.id) return [context.graph.nodes.find((node) => node.id === edge.in)];
        if (edge.in === resolved.node!.id) return [context.graph.nodes.find((node) => node.id === edge.out)];
        return [];
      })
      .filter((node): node is GraphNodeDto => Boolean(node));

    this.dataset.hirselReady = "true";
    if (related.length === 0) {
      this.innerHTML = '<div class="hirsel-code-empty">No related nodes.</div>';
      return;
    }

    this.innerHTML = `<ul class="hirsel-node-list">${related
      .map(
        (node) => `<li><button type="button" class="hirsel-node-list-link" data-node="${escapeHtml(nodeKey(node.kind, node.node_id))}">${escapeHtml(node.label)}</button></li>`,
      )
      .join("")}</ul>`;
    this.querySelectorAll<HTMLButtonElement>(".hirsel-node-list-link").forEach((button) => {
      button.addEventListener("click", () => {
        const parsed = parseNodeAttr(button.dataset.node ?? null);
        if (parsed) {
          navigateTo(this, parsed.kind, parsed.nodeId);
        }
      });
    });
  }
}

class HirselDocTargetElement extends HirselNodeRefElement {}

class HirselDocLinkElement extends HTMLElement {
  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    this.render();
  }

  static get observedAttributes(): string[] {
    return ["node"];
  }

  private render(): void {
    const resolved = resolveNode(this, this.getAttribute("node"));
    if (resolved.error || !resolved.node || !resolved.parsed) {
      this.innerHTML = renderNodeError(resolved.error ?? "Unknown graph node.");
      return;
    }

    const label = escapeHtml(resolved.node.label || resolved.parsed.nodeId);
    const summary = resolved.node.summary ? escapeHtml(resolved.node.summary) : "";
    const kind = escapeHtml(resolved.parsed.kind);
    this.dataset.hirselReady = "true";
    this.innerHTML = `<div class="hirsel-doc-link-card">
      <div class="hirsel-doc-link-head">
        <span class="hirsel-doc-link-kind">${kind}</span>
        <span class="hirsel-doc-link-title">${label}</span>
      </div>
      ${summary ? `<div class="hirsel-doc-link-summary">${summary}</div>` : ""}
    </div>`;
    this.querySelector(".hirsel-doc-link-card")?.addEventListener("click", () => {
      navigateTo(this, resolved.parsed!.kind, resolved.parsed!.nodeId);
    });
  }
}

class HirselDocEmbedElement extends HTMLElement {
  connectedCallback(): void {
    this.render();
  }

  attributeChangedCallback(): void {
    this.render();
  }

  static get observedAttributes(): string[] {
    return ["node"];
  }

  private render(): void {
    const resolved = resolveNode(this, this.getAttribute("node"));
    if (resolved.error || !resolved.node || !resolved.parsed) {
      this.innerHTML = renderNodeError(resolved.error ?? "Unknown graph node.");
      return;
    }

    const bodyHtml = resolved.node.content ?? "";
    const label = escapeHtml(resolved.node.label || resolved.parsed.nodeId);
    const kind = escapeHtml(resolved.parsed.kind);
    this.dataset.hirselReady = "true";
    if (!bodyHtml.trim()) {
      this.innerHTML = `<div class="hirsel-doc-embed-shell">
        <div class="hirsel-doc-embed-header"><span class="hirsel-doc-link-kind">${kind}</span> ${label}</div>
        <div class="hirsel-code-empty">This document has no content yet.</div>
      </div>`;
      return;
    }

    this.innerHTML = `<div class="hirsel-doc-embed-shell">
      <div class="hirsel-doc-embed-header" style="cursor:pointer"><span class="hirsel-doc-link-kind">${kind}</span> ${label}</div>
      <div class="hirsel-doc-embed-body">${bodyHtml}</div>
    </div>`;
    this.querySelector(".hirsel-doc-embed-header")?.addEventListener("click", () => {
      navigateTo(this, resolved.parsed!.kind, resolved.parsed!.nodeId);
    });
  }
}

function defineElement(name: string, ctor: CustomElementConstructor): void {
  if (!customElements.get(name)) {
    customElements.define(name, ctor);
  }
}

export function registerHirselDocumentComponents(): void {
  defineElement("hirsel-card", HirselCardElement);
  defineElement("hirsel-callout", HirselCalloutElement);
  defineElement("hirsel-stat-grid", HirselStatGridElement);
  defineElement("hirsel-stat", HirselStatElement);
  defineElement("hirsel-tabs", HirselTabsElement);
  defineElement("hirsel-disclosure", HirselDisclosureElement);
  defineElement("hirsel-progress", HirselProgressElement);
  defineElement("hirsel-node-ref", HirselNodeRefElement);
  defineElement("hirsel-node-field", HirselNodeFieldElement);
  defineElement("hirsel-node-list", HirselNodeListElement);
  defineElement("hirsel-doc-target", HirselDocTargetElement);
  defineElement("hirsel-doc-link", HirselDocLinkElement);
  defineElement("hirsel-doc-embed", HirselDocEmbedElement);
}

export function setHirselDocumentContext(rootId: string, graph: GraphDto): void {
  graphContexts.set(rootId, {
    graph,
    nodesByKey: new Map(graph.nodes.map((node) => [nodeKey(node.kind, node.node_id), node])),
  });
}

export function clearHirselDocumentContext(rootId: string): void {
  graphContexts.delete(rootId);
}
