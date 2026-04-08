import { createEffect, onCleanup, onMount } from "solid-js";

import {
  clearHirselDocumentContext,
  setHirselDocumentContext,
} from "../lib/hirsel-document-components";
import type { GraphDto } from "../lib/types";

let nextSurfaceId = 0;

export function DocumentSurface(props: {
  graph: GraphDto;
  html: string;
  class?: string;
  onNavigate: (id: string) => void;
}) {
  const surfaceId = `hirsel-surface-${++nextSurfaceId}`;
  let hostRef: HTMLDivElement | undefined;

  const handleNavigate = (event: Event) => {
    const detail = (event as CustomEvent<{ kind: string; nodeId: string }>).detail;
    const node = props.graph.nodes.find(
      (candidate) => candidate.kind === detail.kind && candidate.node_id === detail.nodeId,
    );
    if (node) {
      props.onNavigate(node.id);
    }
  };

  onMount(() => {
    hostRef?.addEventListener("hirsel-navigate-node", handleNavigate as EventListener);
  });

  createEffect(() => {
    setHirselDocumentContext(surfaceId, props.graph);
    if (hostRef) {
      hostRef.dataset.hirselGraphRoot = surfaceId;
      hostRef.innerHTML = props.html;
    }
  });

  onCleanup(() => {
    hostRef?.removeEventListener("hirsel-navigate-node", handleNavigate as EventListener);
    clearHirselDocumentContext(surfaceId);
  });

  return <div ref={hostRef} class={`document-html ${props.class ?? ""}`.trim()} />;
}
