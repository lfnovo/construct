import { useEffect, useMemo, useRef, useState } from "react";
import type { OkfConcept } from "./okf";

const GRAPH_WIDTH = 1200;
const GRAPH_HEIGHT = 700;
const MAX_NODES = 300;
const TYPE_COLORS = ["#8b8cf8", "#45bfa9", "#e4a85e", "#db6f91", "#63a9ee", "#a67de0", "#71b96b", "#dd7a62", "#5fc0db", "#c99b45"];

type GraphNode = OkfConcept & { x: number; y: number; radius: number; degree: number; color: string };
type GraphEdge = { source: GraphNode; target: GraphNode };

function colorForType(type: string) {
  let hash = 0;
  for (const character of type) hash = ((hash << 5) - hash + character.charCodeAt(0)) | 0;
  return TYPE_COLORS[Math.abs(hash) % TYPE_COLORS.length];
}

function layoutConcepts(concepts: OkfConcept[]) {
  const known = new Set(concepts.map((concept) => concept.path));
  const degrees = new Map(concepts.map((concept) => [
    concept.path,
    concept.outgoingPaths.filter((path) => known.has(path)).length + concept.incomingPaths.filter((path) => known.has(path)).length,
  ]));
  const visible = concepts.length <= MAX_NODES
    ? concepts
    : [...concepts].sort((left, right) => (degrees.get(right.path) || 0) - (degrees.get(left.path) || 0) || left.path.localeCompare(right.path)).slice(0, MAX_NODES);
  const visiblePaths = new Set(visible.map((concept) => concept.path));
  const types = [...new Set(visible.map((concept) => concept.type))].sort();
  const typeIndexes = new Map(types.map((type, index) => [type, index]));
  const positionsByType = new Map<string, number>();
  const nodes: GraphNode[] = visible.map((concept) => {
    const typeIndex = typeIndexes.get(concept.type) || 0;
    const position = positionsByType.get(concept.type) || 0;
    positionsByType.set(concept.type, position + 1);
    const clusterAngle = (typeIndex / Math.max(1, types.length)) * Math.PI * 2 - Math.PI / 2;
    const clusterRadius = types.length === 1 ? 0 : Math.min(GRAPH_WIDTH, GRAPH_HEIGHT) * .3;
    const clusterX = GRAPH_WIDTH / 2 + Math.cos(clusterAngle) * clusterRadius;
    const clusterY = GRAPH_HEIGHT / 2 + Math.sin(clusterAngle) * clusterRadius;
    const localAngle = position * 2.399963229728653;
    const localRadius = 22 + Math.sqrt(position) * 21;
    const degree = degrees.get(concept.path) || 0;
    return {
      ...concept,
      x: clusterX + Math.cos(localAngle) * localRadius,
      y: clusterY + Math.sin(localAngle) * localRadius,
      radius: Math.min(12, 5.5 + Math.sqrt(degree) * 1.8),
      degree,
      color: colorForType(concept.type),
    };
  });
  const nodesByPath = new Map(nodes.map((node) => [node.path, node]));
  const edges: GraphEdge[] = [];
  const edgeKeys = new Set<string>();
  for (const source of nodes) {
    for (const targetPath of source.outgoingPaths) {
      if (!visiblePaths.has(targetPath)) continue;
      const target = nodesByPath.get(targetPath);
      if (!target) continue;
      const key = [source.path, target.path].sort().join("\0");
      if (edgeKeys.has(key)) continue;
      edgeKeys.add(key);
      edges.push({ source, target });
    }
  }

  const velocities = new Map(nodes.map((node) => [node.path, { x: 0, y: 0 }]));
  for (let iteration = 0; iteration < 90; iteration += 1) {
    const forces = new Map(nodes.map((node) => [node.path, { x: 0, y: 0 }]));
    for (let leftIndex = 0; leftIndex < nodes.length; leftIndex += 1) {
      for (let rightIndex = leftIndex + 1; rightIndex < nodes.length; rightIndex += 1) {
        const left = nodes[leftIndex];
        const right = nodes[rightIndex];
        const deltaX = left.x - right.x || .01;
        const deltaY = left.y - right.y || .01;
        const distanceSquared = Math.max(36, deltaX * deltaX + deltaY * deltaY);
        const distance = Math.sqrt(distanceSquared);
        const push = Math.min(5, 900 / distanceSquared);
        const forceX = deltaX / distance * push;
        const forceY = deltaY / distance * push;
        forces.get(left.path)!.x += forceX;
        forces.get(left.path)!.y += forceY;
        forces.get(right.path)!.x -= forceX;
        forces.get(right.path)!.y -= forceY;
      }
    }
    for (const edge of edges) {
      const deltaX = edge.target.x - edge.source.x;
      const deltaY = edge.target.y - edge.source.y;
      const distance = Math.max(1, Math.sqrt(deltaX * deltaX + deltaY * deltaY));
      const pull = (distance - 90) * .006;
      const forceX = deltaX / distance * pull;
      const forceY = deltaY / distance * pull;
      forces.get(edge.source.path)!.x += forceX;
      forces.get(edge.source.path)!.y += forceY;
      forces.get(edge.target.path)!.x -= forceX;
      forces.get(edge.target.path)!.y -= forceY;
    }
    for (const node of nodes) {
      const typeIndex = typeIndexes.get(node.type) || 0;
      const angle = (typeIndex / Math.max(1, types.length)) * Math.PI * 2 - Math.PI / 2;
      const radius = types.length === 1 ? 0 : Math.min(GRAPH_WIDTH, GRAPH_HEIGHT) * .3;
      const targetX = GRAPH_WIDTH / 2 + Math.cos(angle) * radius;
      const targetY = GRAPH_HEIGHT / 2 + Math.sin(angle) * radius;
      const force = forces.get(node.path)!;
      force.x += (targetX - node.x) * .0025;
      force.y += (targetY - node.y) * .0025;
      const velocity = velocities.get(node.path)!;
      velocity.x = (velocity.x + force.x) * .82;
      velocity.y = (velocity.y + force.y) * .82;
      node.x = Math.max(36, Math.min(GRAPH_WIDTH - 36, node.x + velocity.x));
      node.y = Math.max(36, Math.min(GRAPH_HEIGHT - 36, node.y + velocity.y));
    }
  }

  const labeled = new Set([...nodes].sort((left, right) => right.degree - left.degree).slice(0, 24).map((node) => node.path));
  return { nodes, edges, types, labeled, truncated: concepts.length - nodes.length };
}

export function KnowledgeGraph({ concepts, onOpen }: { concepts: OkfConcept[]; onOpen: (path: string) => void }) {
  const graph = useMemo(() => layoutConcepts(concepts), [concepts]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [viewport, setViewport] = useState({ x: 0, y: 0, scale: 1 });
  const drag = useRef<{ x: number; y: number; originX: number; originY: number } | null>(null);
  const selected = graph.nodes.find((node) => node.path === selectedPath) || null;
  const relatedPaths = useMemo(() => selected ? new Set([...selected.outgoingPaths, ...selected.incomingPaths, selected.path]) : null, [selected]);

  useEffect(() => {
    if (selectedPath && !graph.nodes.some((node) => node.path === selectedPath)) setSelectedPath(null);
  }, [graph.nodes, selectedPath]);

  if (!graph.nodes.length) return <div className="knowledge-graph-empty">No concepts match the current filters.</div>;

  return <div className="knowledge-graph">
    <div className="graph-toolbar">
      <span>Drag to pan · scroll to zoom · double-click a node to open</span>
      {graph.truncated > 0 && <strong>Showing the 300 most connected concepts · {graph.truncated} hidden</strong>}
      <button onClick={() => setViewport({ x: 0, y: 0, scale: 1 })}>Reset view</button>
    </div>
    <div className="graph-stage">
      <svg
        viewBox={`0 0 ${GRAPH_WIDTH} ${GRAPH_HEIGHT}`}
        role="img"
        aria-label={`Knowledge graph with ${graph.nodes.length} concepts and ${graph.edges.length} relationships`}
        onWheel={(event) => {
          event.preventDefault();
          const factor = event.deltaY < 0 ? 1.12 : .89;
          setViewport((current) => ({ ...current, scale: Math.max(.45, Math.min(3.2, current.scale * factor)) }));
        }}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = { x: event.clientX, y: event.clientY, originX: viewport.x, originY: viewport.y };
        }}
        onPointerMove={(event) => {
          if (!drag.current) return;
          const scale = viewport.scale || 1;
          setViewport((current) => ({ ...current, x: drag.current!.originX + (event.clientX - drag.current!.x) / scale, y: drag.current!.originY + (event.clientY - drag.current!.y) / scale }));
        }}
        onPointerUp={() => { drag.current = null; }}
        onPointerCancel={() => { drag.current = null; }}
      >
        <g transform={`translate(${viewport.x} ${viewport.y}) scale(${viewport.scale})`}>
          <g className="graph-edges">
            {graph.edges.map((edge) => {
              const highlighted = selected ? edge.source.path === selected.path || edge.target.path === selected.path : false;
              const dimmed = Boolean(selected && !highlighted);
              return <line key={`${edge.source.path}\0${edge.target.path}`} x1={edge.source.x} y1={edge.source.y} x2={edge.target.x} y2={edge.target.y} className={highlighted ? "highlighted" : dimmed ? "dimmed" : ""} />;
            })}
          </g>
          <g className="graph-nodes">
            {graph.nodes.map((node) => {
              const isSelected = node.path === selectedPath;
              const dimmed = Boolean(relatedPaths && !relatedPaths.has(node.path));
              return <g
                key={node.path}
                className={`${isSelected ? "selected" : ""} ${dimmed ? "dimmed" : ""} ${graph.labeled.has(node.path) ? "labeled" : ""}`}
                role="button"
                tabIndex={0}
                aria-label={`${node.title}, ${node.type}`}
                transform={`translate(${node.x} ${node.y})`}
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => setSelectedPath(node.path)}
                onDoubleClick={() => onOpen(node.path)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    isSelected ? onOpen(node.path) : setSelectedPath(node.path);
                  }
                }}
              >
                <circle r={node.radius + (isSelected ? 4 : 0)} fill={node.color} />
                <text x={node.radius + 7} y={4}>{node.title}</text>
                <title>{node.title} · {node.type} · {node.degree} relationships</title>
              </g>;
            })}
          </g>
        </g>
      </svg>
      <div className="graph-legend">{graph.types.map((type) => <span key={type}><i style={{ background: colorForType(type) }} />{type}</span>)}</div>
      {selected && <aside className="graph-selection">
        <button className="graph-selection-close" aria-label="Close concept details" onClick={() => setSelectedPath(null)}>×</button>
        <span className="graph-selection-type" style={{ color: selected.color }}>{selected.type}</span>
        <h3>{selected.title}</h3>
        {selected.description && <p>{selected.description}</p>}
        <small>{selected.relativePath}</small>
        <div>{selected.outgoingPaths.length} outgoing · {selected.incomingPaths.length} incoming</div>
        {!!selected.tags.length && <footer>{selected.tags.slice(0, 6).map((tag) => <span key={tag}>#{tag}</span>)}</footer>}
        <button className="toolbar-button" onClick={() => onOpen(selected.path)}>Open document</button>
      </aside>}
    </div>
  </div>;
}
