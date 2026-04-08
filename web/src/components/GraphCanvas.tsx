import { createEffect, onCleanup, onMount } from "solid-js";
import * as THREE from "three";

import type { GraphDto, GraphEdgeDto, GraphNodeDto, NodeKind } from "../lib/types";

type SimNode = {
  id: string;
  raw: GraphNodeDto;
  mesh: THREE.Mesh;
  ring: THREE.LineLoop;
  label: THREE.Sprite;
  x: number;
  y: number;
  vx: number;
  vy: number;
};

type SimEdge = {
  raw: GraphEdgeDto;
  line: THREE.Line;
};

const NODE_STYLES: Record<NodeKind, {
  color: number;
  labelColor: string;
  radius: number;
  ringRadius: number;
  segments: number;
}> = {
  document: { color: 0xc5a35f, labelColor: "#f2c36e", radius: 0.56, ringRadius: 0.82, segments: 6 },
  image: { color: 0x7db5c9, labelColor: "#9fd2e6", radius: 0.66, ringRadius: 0.92, segments: 32 },
  url: { color: 0x5db7aa, labelColor: "#8ce1d7", radius: 0.6, ringRadius: 0.86, segments: 4 },
  topic: { color: 0xd88a7f, labelColor: "#f3b2a7", radius: 0.58, ringRadius: 0.84, segments: 5 },
};

export function GraphCanvas(props: {
  graph: GraphDto;
  selectedId: string | null;
  searchQuery: string;
  onSelect: (id: string | null) => void;
}) {
  let hostRef: HTMLDivElement | undefined;
  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let camera: THREE.OrthographicCamera | null = null;
  let animationFrame = 0;
  let resizeObserver: ResizeObserver | undefined;
  let simNodes: SimNode[] = [];
  let simEdges: SimEdge[] = [];
  let dragPointer: { x: number; y: number } | null = null;
  let cameraOrigin = new THREE.Vector3();

  const makeLabel = (text: string, color: string) => {
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("2d")!;
    context.font = `500 28px "IBM Plex Mono", monospace`;
    const metrics = context.measureText(text);
    const width = Math.ceil(metrics.width) + 32;
    const height = 48;
    canvas.width = width;
    canvas.height = height;

    context.font = `500 28px "IBM Plex Mono", monospace`;
    context.fillStyle = "rgba(11, 13, 18, 0.88)";
    context.fillRect(0, 0, width, height);
    context.strokeStyle = color;
    context.lineWidth = 2;
    context.strokeRect(1, 1, width - 2, height - 2);
    context.fillStyle = color;
    context.textBaseline = "middle";
    context.fillText(text, 16, height / 2);

    const texture = new THREE.CanvasTexture(canvas);
    texture.minFilter = THREE.LinearFilter;
    const material = new THREE.SpriteMaterial({ map: texture, transparent: true, depthTest: false });
    const sprite = new THREE.Sprite(material);
    sprite.scale.set(width / 28, height / 28, 1);
    return sprite;
  };

  const rebuildScene = () => {
    const currentScene = scene;
    if (!currentScene) return;
    for (const node of simNodes) {
      currentScene.remove(node.mesh);
      currentScene.remove(node.ring);
      currentScene.remove(node.label);
      node.mesh.geometry.dispose();
      (node.mesh.material as THREE.Material).dispose();
      node.ring.geometry.dispose();
      (node.ring.material as THREE.Material).dispose();
      (node.label.material as THREE.SpriteMaterial).dispose();
    }
    for (const edge of simEdges) {
      currentScene.remove(edge.line);
      edge.line.geometry.dispose();
      (edge.line.material as THREE.Material).dispose();
    }
    simNodes = [];
    simEdges = [];

    const nodeById = new Map<string, SimNode>();
    props.graph.nodes.forEach((node, index) => {
      const style = NODE_STYLES[node.kind];
      const mesh = new THREE.Mesh(
        new THREE.CircleGeometry(style.radius, style.segments),
        new THREE.MeshBasicMaterial({ color: style.color, transparent: true, opacity: 0.9 }),
      );
      const ring = new THREE.LineLoop(
        new THREE.CircleGeometry(style.ringRadius, Math.max(style.segments + 8, 16)),
        new THREE.LineBasicMaterial({ color: 0xf4d7a5, transparent: true, opacity: 0.2 }),
      );
      const label = makeLabel(node.label, style.labelColor);
      const simNode: SimNode = {
        id: node.id,
        raw: node,
        mesh,
        ring,
        label,
        x: Math.cos(index * 0.7) * (6 + index * 0.15),
        y: Math.sin(index * 0.7) * (6 + index * 0.15),
        vx: 0,
        vy: 0,
      };
      mesh.userData.nodeId = node.id;
      ring.userData.nodeId = node.id;
      currentScene.add(mesh);
      currentScene.add(ring);
      currentScene.add(label);
      simNodes.push(simNode);
      nodeById.set(node.id, simNode);
    });

    props.graph.edges.forEach((edge) => {
      const from = nodeById.get(edge.out);
      const to = nodeById.get(edge.in);
      if (!from || !to) return;
      const geometry = new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(from.x, from.y, 0),
        new THREE.Vector3(to.x, to.y, 0),
      ]);
      const line = new THREE.Line(
        geometry,
        new THREE.LineBasicMaterial({ color: 0x435569, transparent: true, opacity: 0.65 }),
      );
      currentScene.add(line);
      simEdges.push({ raw: edge, line });
    });
  };

  const fitCamera = () => {
    if (!hostRef || !camera || !renderer) return;
    const width = hostRef.clientWidth;
    const height = hostRef.clientHeight;
    renderer.setSize(width, height);
    const aspect = width / Math.max(height, 1);
    const viewSize = 18;
    camera.left = (-viewSize * aspect) / 2;
    camera.right = (viewSize * aspect) / 2;
    camera.top = viewSize / 2;
    camera.bottom = -viewSize / 2;
    camera.updateProjectionMatrix();
  };

  const updateVisualState = () => {
    const selected = props.selectedId;
    const search = props.searchQuery.trim().toLowerCase();
    const related = new Set<string>();
    if (selected) {
      props.graph.edges.forEach((edge) => {
        if (edge.in === selected) related.add(edge.out);
        if (edge.out === selected) related.add(edge.in);
      });
    }

    for (const node of simNodes) {
      const matchesSearch =
        !search ||
        node.raw.node_id.toLowerCase().includes(search) ||
        node.raw.label.toLowerCase().includes(search) ||
        node.raw.summary?.toLowerCase().includes(search) ||
        node.raw.content?.toLowerCase().includes(search) ||
        node.raw.search_text?.toLowerCase().includes(search) ||
        node.raw.source?.toLowerCase().includes(search);
      const isSelected = selected === node.id;
      const isRelated = related.has(node.id);
      const opacity = matchesSearch ? (selected && !isSelected && !isRelated ? 0.28 : 0.95) : 0.08;
      (node.mesh.material as THREE.MeshBasicMaterial).opacity = opacity;
      (node.ring.material as THREE.LineBasicMaterial).opacity = isSelected ? 0.95 : isRelated ? 0.45 : opacity * 0.4;
      node.label.material.opacity = matchesSearch ? (isSelected ? 1 : 0.8) : 0.15;
      node.label.scale.setScalar(isSelected ? 1.12 : 1);
      node.ring.scale.setScalar(isSelected ? 1.15 : 1);
    }

    for (const edge of simEdges) {
      const active = !selected || edge.raw.in === selected || edge.raw.out === selected;
      (edge.line.material as THREE.LineBasicMaterial).opacity = active ? 0.65 : 0.12;
    }
  };

  const animate = () => {
    animationFrame = requestAnimationFrame(animate);
    const repulsion = 0.015;
    const spring = 0.0024;
    const damping = 0.92;

    for (let i = 0; i < simNodes.length; i += 1) {
      for (let j = i + 1; j < simNodes.length; j += 1) {
        const a = simNodes[i];
        const b = simNodes[j];
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        const distance = Math.max(Math.sqrt(dx * dx + dy * dy), 0.2);
        const force = repulsion / (distance * distance);
        dx *= force / distance;
        dy *= force / distance;
        a.vx += dx;
        a.vy += dy;
        b.vx -= dx;
        b.vy -= dy;
      }
    }

    const byId = new Map(simNodes.map((node) => [node.id, node]));
    for (const edge of simEdges) {
      const a = byId.get(edge.raw.out);
      const b = byId.get(edge.raw.in);
      if (!a || !b) continue;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const distance = Math.max(Math.sqrt(dx * dx + dy * dy), 0.1);
      const force = distance * spring;
      a.vx += (dx / distance) * force;
      a.vy += (dy / distance) * force;
      b.vx -= (dx / distance) * force;
      b.vy -= (dy / distance) * force;
    }

    for (const node of simNodes) {
      node.vx -= node.x * 0.0012;
      node.vy -= node.y * 0.0012;
      node.vx *= damping;
      node.vy *= damping;
      node.x += node.vx;
      node.y += node.vy;
      node.mesh.position.set(node.x, node.y, 0);
      node.ring.position.set(node.x, node.y, 0);
      node.label.position.set(node.x, node.y + 1.1, 0);
    }

    for (const edge of simEdges) {
      const a = byId.get(edge.raw.out);
      const b = byId.get(edge.raw.in);
      if (!a || !b) continue;
      const position = edge.line.geometry.getAttribute("position") as THREE.BufferAttribute;
      position.setXYZ(0, a.x, a.y, 0);
      position.setXYZ(1, b.x, b.y, 0);
      position.needsUpdate = true;
    }

    updateVisualState();
    renderer?.render(scene!, camera!);
  };

  onMount(() => {
    if (!hostRef) return;
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.domElement.className = "graph-canvas__surface";
    hostRef.append(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.OrthographicCamera(-10, 10, 10, -10, 0.1, 100);
    camera.position.set(0, 0, 20);

    const raycaster = new THREE.Raycaster();
    const pointer = new THREE.Vector2();

    const pointerToNode = (event: PointerEvent) => {
      if (!hostRef || !camera) return null;
      const rect = hostRef.getBoundingClientRect();
      pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
      pointer.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
      raycaster.setFromCamera(pointer, camera);
      const intersects = raycaster.intersectObjects(simNodes.map((node) => node.mesh));
      return (intersects[0]?.object.userData.nodeId as string | undefined) ?? null;
    };

    const onPointerDown = (event: PointerEvent) => {
      dragPointer = { x: event.clientX, y: event.clientY };
      cameraOrigin.copy(camera!.position);
    };

    const onPointerMove = (event: PointerEvent) => {
      if (!dragPointer || !camera || !hostRef) return;
      const dx = ((event.clientX - dragPointer.x) / hostRef.clientWidth) * (camera.right - camera.left);
      const dy = ((event.clientY - dragPointer.y) / hostRef.clientHeight) * (camera.top - camera.bottom);
      camera.position.set(cameraOrigin.x - dx, cameraOrigin.y + dy, camera.position.z);
    };

    const onPointerUp = (event: PointerEvent) => {
      if (dragPointer && Math.abs(event.clientX - dragPointer.x) < 4 && Math.abs(event.clientY - dragPointer.y) < 4) {
        props.onSelect(pointerToNode(event));
      }
      dragPointer = null;
    };

    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      if (!camera) return;
      const zoom = event.deltaY > 0 ? 1.08 : 0.92;
      camera.zoom = Math.min(2.4, Math.max(0.45, camera.zoom / zoom));
      camera.updateProjectionMatrix();
    };

    hostRef.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    hostRef.addEventListener("wheel", onWheel, { passive: false });

    resizeObserver = new ResizeObserver(() => fitCamera());
    resizeObserver.observe(hostRef);
    rebuildScene();
    fitCamera();
    animate();

    onCleanup(() => {
      cancelAnimationFrame(animationFrame);
      resizeObserver?.disconnect();
      hostRef?.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      hostRef?.removeEventListener("wheel", onWheel);
      renderer?.dispose();
    });
  });

  createEffect(() => {
    props.graph;
    rebuildScene();
  });

  createEffect(() => {
    props.selectedId;
    props.searchQuery;
    updateVisualState();
  });

  return <div ref={hostRef} class="graph-canvas" />;
}
