import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import {
  ArrowRightIcon as ArrowRight,
  CaretDownIcon as CaretDown,
  CircleIcon as Circle,
  CompassRoseIcon as CompassRose,
  CrosshairIcon as Crosshair,
  FunnelIcon as Funnel,
  GlobeHemisphereWestIcon as GlobeHemisphereWest,
  HexagonIcon as Hexagon,
  KeyboardIcon as Keyboard,
  MagnifyingGlassIcon as MagnifyingGlass,
  MapTrifoldIcon as MapTrifold,
  MinusIcon as Minus,
  PathIcon as Path,
  PlusIcon as Plus,
  RectangleIcon as Rectangle,
  TargetIcon as Target,
} from "@phosphor-icons/react";
import { Link, NavLink, useSearchParams } from "react-router-dom";

import type {
  MapWorkspaceLayer,
  MapWorkspaceNode,
  MapWorkspaceQuery,
  MapWorkspaceSnapshot,
} from "../contracts/core";
import { commandKeys, getMapWorkspace, normalizeCommandError } from "../lib/commands";

type MapView = "current" | "prediction" | "global";

const PHASES = [
  "undetermined",
  "acquisition",
  "consolidation",
  "transfer",
  "generation",
  "phantom",
  "settling",
  "regression",
];

const VIEW_COPY: Record<MapView, { label: string; description: string }> = {
  current: { label: "当前地图", description: "定位正在形成与需要补强的知识。" },
  prediction: { label: "跨域预测", description: "分开查看观测、预测与先验，并选择迁移路径。" },
  global: { label: "全局总览", description: "比较 Pack 与能力维度，不绘制巨型概念图。" },
};

const PRODUCT_DESTINATIONS = [
  { to: "/inbox", label: "Inbox", description: "学习线索与证据" },
  { to: "/profile", label: "Profile", description: "画像与验证状态" },
  { to: "/reports", label: "Reports", description: "可追溯学习报告" },
  { to: "/trust", label: "Trust", description: "实验门与数据主权" },
  { to: "/settings", label: "Settings", description: "本地体验设置" },
] as const;

function nodeUnderstanding(node: MapWorkspaceNode): "known" | "fuzzy" | "new" {
  if (node.phase === "transfer" || node.phase === "generation") return "known";
  if (node.attempt_count === 0) return "new";
  return "fuzzy";
}

function understandingLabel(node: MapWorkspaceNode) {
  const state = nodeUnderstanding(node);
  return state === "known" ? "会" : state === "new" ? "未学" : "模糊";
}

function primaryLayer(node: MapWorkspaceNode): MapWorkspaceLayer | undefined {
  return node.layers.find((layer) => layer.source === "observed") ??
    node.layers.find((layer) => layer.source === "latent_prediction") ??
    node.layers[0];
}

function layerLabel(source: string) {
  if (source === "observed") return "观测";
  if (source === "latent_prediction") return "预测";
  return "先验";
}

function edgeLabel(kind: string) {
  if (kind === "prerequisite") return "先修";
  if (kind === "maps_to") return "迁移";
  if (kind === "contrast") return "对照";
  return kind;
}

function atlasNodePositions(nodes: MapWorkspaceNode[]): Record<string, { x: number; y: number }> {
  const positions: Record<string, { x: number; y: number }> = {};
  const focus = nodes.find((node) => node.due_status === "due" && nodeUnderstanding(node) === "fuzzy");
  const namedPositions: Partial<Record<string, { x: number; y: number }>> = {
    "graph-representation": { x: 255, y: 155 },
    adjacency: { x: 175, y: 330 },
    "priority-queue": { x: 665, y: 135 },
    dijkstra: { x: 505, y: 330 },
    astar: { x: 820, y: 340 },
    greedy: { x: 455, y: 540 },
    "dynamic-programming": { x: 670, y: 565 },
    "union-find": { x: 930, y: 545 },
    "bellman-ford": { x: 1035, y: 315 },
    johnson: { x: 1115, y: 500 },
    mst: { x: 1180, y: 455 },
    "shortest-path-schema": { x: 100, y: 585 },
  };
  for (const node of nodes) {
    const position = namedPositions[node.id];
    if (position !== undefined) positions[node.id] = position;
  }
  if (focus && !Object.hasOwn(positions, focus.id)) positions[focus.id] = { x: 505, y: 330 };

  const slots = {
    known: [{ x: 110, y: 140 }, { x: 160, y: 300 }, { x: 300, y: 100 }, { x: 330, y: 470 }],
    fuzzy: [{ x: 560, y: 100 }, { x: 690, y: 410 }, { x: 430, y: 500 }, { x: 710, y: 190 }],
    new: [{ x: 900, y: 100 }, { x: 1030, y: 240 }, { x: 890, y: 410 }, { x: 1080, y: 500 }, { x: 760, y: 510 }],
  };
  const counters = { known: 0, fuzzy: 0, new: 0 };
  for (const node of nodes) {
    if (Object.hasOwn(positions, node.id)) continue;
    const state = nodeUnderstanding(node);
    const index = counters[state]++;
    const slot = slots[state].at(index);
    positions[node.id] = slot ?? { x: 180 + (index % 5) * 205, y: 150 + Math.floor(index / 5) * 170 };
  }
  return positions;
}

export function graphElements(snapshot: MapWorkspaceSnapshot, visibleIds: Set<string>, selectedId: string | null = null): ElementDefinition[] {
  const nodes: ElementDefinition[] = snapshot.nodes
    .filter((node) => visibleIds.has(node.id))
    .map((node) => {
      const layer = primaryLayer(node);
      return {
        data: {
          id: node.id,
          label: node.name,
          kind: node.kind,
          understanding: understandingLabel(node),
        },
        classes: [
          node.kind === "schema" ? "schema" : "concept",
          nodeUnderstanding(node),
          layer?.source ?? "inherited_prior",
          layer?.gate_status === "active" ? "confident" : "low-confidence",
          node.due_status === "due" ? "due" : "",
          node.id === selectedId ? "atlas-selected" : "",
        ].join(" "),
      };
    });
  const edges: ElementDefinition[] = snapshot.edges
    .filter((edge) => visibleIds.has(edge.source_id) && visibleIds.has(edge.target_id))
    .map((edge) => ({
      data: {
        id: edge.id,
        source: edge.source_id,
        target: edge.target_id,
        label: edgeLabel(edge.kind),
        kind: edge.kind,
      },
      classes: edge.kind,
    }));
  return [...nodes, ...edges];
}

function MapCanvas({
  snapshot,
  visibleIds,
  selectedId,
  onSelect,
}: {
  snapshot: MapWorkspaceSnapshot;
  visibleIds: Set<string>;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const graph = useRef<Core | null>(null);
  const [selectedLabelPosition, setSelectedLabelPosition] = useState<{ x: number; y: number } | null>(null);
  const selectedNode = snapshot.nodes.find((node) => node.id === selectedId) ?? null;
  const elements = useMemo(() => graphElements(snapshot, visibleIds, selectedId), [snapshot, visibleIds, selectedId]);
  const positions = useMemo(() => atlasNodePositions(snapshot.nodes.filter((node) => visibleIds.has(node.id))), [snapshot.nodes, visibleIds]);

  useEffect(() => {
    if (!container.current) return;
    const instance = cytoscape({
      container: container.current,
      elements,
      minZoom: 0.35,
      maxZoom: 2.5,
      autoungrabify: true,
      layout: {
        name: "preset",
        animate: false,
        fit: true,
        padding: 76,
        positions,
      },
      style: [
        {
          selector: "node",
          style: {
            label: "data(label)",
            "font-family": "Inter, system-ui, sans-serif",
            "font-size": 12,
            "font-weight": 500,
            color: "#173b72",
            "text-valign": "bottom",
            "text-margin-y": 15,
            "text-wrap": "wrap",
            "text-max-width": "110px",
            width: 38,
            height: 38,
            "background-color": "#afc5d6",
            "border-width": 2.5,
            "border-color": "#6f8ca9",
            "overlay-opacity": 0,
          },
        },
        { selector: "node.known", style: { shape: "hexagon", "background-color": "#1e5aa7", "border-color": "#163f76", color: "#173b72" } },
        { selector: "node.fuzzy", style: { shape: "hexagon", "background-color": "#c2613d", "border-color": "#8a3e29", color: "#7f321f" } },
        { selector: "node.new", style: { "background-color": "#d8e0e3", "border-color": "#8ea2b4", color: "#5f7284" } },
        { selector: "node.schema", style: { shape: "round-rectangle", width: 82, height: 60, "background-color": "#f5f0e7", "border-color": "#c79a3b", color: "#86651f" } },
        { selector: "node.latent_prediction", style: { "border-style": "dashed" } },
        { selector: "node.inherited_prior", style: { "border-style": "dotted" } },
        { selector: "node.low-confidence", style: { opacity: 0.7, "border-width": 4 } },
        { selector: "node.due", style: { "underlay-color": "#b45a3c", "underlay-opacity": 0.17, "underlay-padding": 13 } },
        { selector: "node.atlas-selected", style: { label: "", width: 54, height: 54, "background-color": "#173f78", "border-color": "#f7f2e8", "border-width": 7, "underlay-color": "#164f96", "underlay-opacity": 0.2, "underlay-padding": 15, opacity: 1, "z-index": 20 } },
        {
          selector: "edge",
          style: {
            width: 2,
            "line-color": "#5d8fc4",
            "target-arrow-color": "#5d8fc4",
            "target-arrow-shape": "triangle",
            "arrow-scale": 0.9,
            "curve-style": "unbundled-bezier",
            "control-point-distances": 28,
            "control-point-weights": 0.5,
            label: "data(label)",
            "font-size": 8,
            color: "#596b78",
            "text-background-color": "#f5f0e7",
            "text-background-opacity": 0.76,
            "text-background-padding": "3px",
          },
        },
        { selector: "edge.maps_to", style: { "line-style": "dashed", "line-color": "#d49a31", "target-arrow-color": "#d49a31" } },
        { selector: "edge.contrast", style: { "line-style": "dashed", "line-color": "#b85a3d", "target-arrow-color": "#b85a3d" } },
      ],
    });
    instance.on("tap", "node", (event: cytoscape.EventObjectNode) => {
      onSelect(event.target.id());
    });
    const updateSelectedLabel = () => {
      if (!selectedId) {
        setSelectedLabelPosition(null);
        return;
      }
      const selectedElement = instance.$id(selectedId);
      if (selectedElement.empty()) {
        setSelectedLabelPosition(null);
        return;
      }
      const position = selectedElement.renderedPosition();
      setSelectedLabelPosition({ x: position.x, y: position.y });
    };
    instance.ready(() => {
      instance.fit(undefined, 86);
      instance.panBy({ x: 0, y: -112 });
      updateSelectedLabel();
    });
    instance.on("pan zoom resize", updateSelectedLabel);
    graph.current = instance;
    return () => {
      graph.current = null;
      instance.off("pan zoom resize", updateSelectedLabel);
      instance.destroy();
    };
  }, [elements, onSelect, positions]);

  useEffect(() => {
    if (!graph.current || !selectedId) return;
    graph.current.$id(selectedId).select();
  }, [selectedId]);

  return (
    <div className="atlas-canvas-wrap">
      <div
        ref={container}
        className="map-canvas"
        role="img"
        aria-label={`知识地图，当前显示 ${String(visibleIds.size)} 个节点。键盘浏览面板提供等价访问。`}
      />
      {selectedNode && selectedLabelPosition && (
        <div className="atlas-selected-label" style={{ left: selectedLabelPosition.x, top: selectedLabelPosition.y }} aria-hidden="true">
          <strong>{selectedNode.name}</strong>
          <span>{understandingLabel(selectedNode)}{primaryLayer(selectedNode)?.gate_status !== "active" ? " · 低置信" : ""}</span>
        </div>
      )}
      <div className="atlas-zoom" aria-label="地图缩放">
        <button type="button" aria-label="放大地图" onClick={() => graph.current?.zoom(graph.current.zoom() * 1.2)}><Plus size={18} /></button>
        <button type="button" aria-label="缩小地图" onClick={() => graph.current?.zoom(graph.current.zoom() / 1.2)}><Minus size={18} /></button>
        <button type="button" onClick={() => graph.current?.fit(undefined, 64)}>Fit</button>
        <button type="button" aria-label="居中选中节点" onClick={() => selectedId && graph.current?.center(graph.current.$id(selectedId))}><Crosshair size={18} /></button>
      </div>
    </div>
  );
}

function ConfidenceWhisker({ layer }: { layer?: MapWorkspaceLayer }) {
  const value = Math.round((layer?.value ?? 0) * 100);
  const lower = Math.round((layer?.lower ?? Math.max(0, (layer?.value ?? 0) - 0.14)) * 100);
  const upper = Math.round((layer?.upper ?? Math.min(1, (layer?.value ?? 0) + 0.14)) * 100);
  return (
    <div className="confidence-whisker" aria-label={`估计 ${String(value)}%，区间 ${String(lower)}% 到 ${String(upper)}%`}>
      <span className="confidence-whisker__range" style={{ left: `${String(lower)}%`, width: `${String(Math.max(2, upper - lower))}%` }} />
      <span className="confidence-whisker__point" style={{ left: `${String(value)}%` }} />
      <span>低</span><span>中</span><span>高</span>
    </div>
  );
}

function DecisionDeck({
  node,
  paths,
  anchors,
  onFocus,
}: {
  node: MapWorkspaceNode;
  paths: MapWorkspaceSnapshot["paths"];
  anchors: MapWorkspaceSnapshot["anchors"];
  onFocus: () => void;
}) {
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const layer = primaryLayer(node);
  const lowConfidence = layer?.gate_status !== "active";
  const nextPath = paths.at(0);
  const relevantAnchor = anchors.find((anchor) => anchor.target_id === node.id || anchor.source_concept_id === node.id);
  return (
    <aside className="atlas-decision" aria-labelledby="inspector-title">
      <div className="atlas-decision__handle" aria-hidden="true" />
      <section className="atlas-decision__judgement">
        <span className="atlas-section-label"><CompassRose size={20} /> 判断</span>
        <h2 id="inspector-title">{node.name}</h2>
        <small className="atlas-metric-label">掌握度（本页分布）</small>
        <ConfidenceWhisker layer={layer} />
        <p className="atlas-verdict">{understandingLabel(node)}{lowConfidence ? " · 低置信" : " · 置信稳定"}</p>
        <p className="atlas-judgement-summary">{node.phase_summary || "证据仍不足以支持稳定迁移。"}</p>
        <small className="atlas-judgement-note">近 {node.attempt_count} 次练习 · {node.evidence_count} 条可追溯证据</small>
        <button type="button" className="atlas-text-button" onClick={onFocus}>只看两跳邻域 <ArrowRight size={14} /></button>
      </section>
      <section className="atlas-decision__evidence">
        <span className="atlas-section-label"><Path size={20} /> 证据</span>
        <small className="atlas-metric-label">相关练习表现（近 10 次）</small>
        <div className="atlas-evidence-trace" role="img" aria-label={`最近 ${String(Math.min(10, node.attempt_count))} 次练习的证据分布`}>
          {Array.from({ length: 10 }, (_, index) => <i key={index} data-active={index < node.attempt_count || undefined} />)}
        </div>
        <p className="atlas-source-summary">来源 · {node.attempt_count} 道练习 · {node.evidence_count} 个笔记/证据 · 1 个知识链接</p>
        {node.layers.length > 1 && (
          <div className="atlas-layer-comparison" aria-label="观测、预测与先验比较">
            {node.layers.map((item) => (
              <div key={item.source}>
                <span>{layerLabel(item.source)}</span>
                <progress max="1" value={item.value}>{Math.round(item.value * 100)}%</progress>
                <b>{Math.round(item.value * 100)}%</b>
              </div>
            ))}
          </div>
        )}
        {relevantAnchor && (
          <div className="atlas-anchor-summary">
            <strong>结构锚点 · {Math.round(relevantAnchor.structural_score * 100)}%</strong>
            <span>{relevantAnchor.source_name} → {relevantAnchor.target_name}</span>
            <small>{relevantAnchor.difference}</small>
          </div>
        )}
        {!relevantAnchor && node.layers.length > 1 && <small className="atlas-anchor-empty">当前没有通过结构门的跨域锚点；预测仅作候选，不算掌握。</small>}
        <button className="atlas-evidence-toggle" type="button" aria-expanded={evidenceOpen} onClick={() => { setEvidenceOpen((value) => !value); }}>
          证据与来源 · {node.evidence_count} 条 <CaretDown size={15} />
        </button>
        {evidenceOpen && (
          <div className="atlas-evidence-list">
            {node.layers.flatMap((item) => item.evidence_ids.map((id) => (
              <Link key={`${item.source}:${id}`} to={`/inbox?evidence=${encodeURIComponent(id)}`} title={`${layerLabel(item.source)}来源`}>证据 {id}</Link>
            )))}
            {relevantAnchor?.evidence_ids.map((id) => (
              <Link key={`anchor:${id}`} to={`/inbox?evidence=${encodeURIComponent(id)}`} title="结构锚点来源">锚点证据 {id}</Link>
            ))}
            {node.evidence_count === 0 && <span>暂无直接证据；当前判断来自 {layerLabel(layer?.source ?? "inherited_prior")}。</span>}
          </div>
        )}
      </section>
      <section className="atlas-decision__next">
        <span className="atlas-section-label"><Target size={20} /> 下一步</span>
        <small className="atlas-metric-label">推荐路径（预测）</small>
        <p>{nextPath ? `${nextPath.move_name} · 预计成功 ${String(Math.round(nextPath.expected_success * 100))}%` : "用一道变式题验证边界条件与迁移稳定性。"}</p>
        <div className="atlas-route-preview" aria-label="推荐学习路径">
          <span data-state="current"><Hexagon size={17} weight="fill" />{node.name}</span><ArrowRight size={14} /><span data-state="prediction"><Circle size={16} weight="fill" />{nextPath?.concept_name ?? "边界变式"}</span><ArrowRight size={14} /><span data-state="due"><Hexagon size={17} weight="fill" />迁移验证</span>
        </div>
        <div className="atlas-actions">
          <Link className="atlas-primary-action" to={`/practice?concept=${encodeURIComponent(node.id)}`}>开始练习 <ArrowRight size={18} /></Link>
          <Link className="atlas-secondary-action" to={`/goals?concept=${encodeURIComponent(node.id)}`}>加入目标</Link>
        </div>
      </section>
    </aside>
  );
}

function AtlasLegend() {
  return (
    <aside className="atlas-legend" aria-label="地图图例">
      <div className="legend-known"><Hexagon size={15} weight="fill" /><span>会</span></div>
      <div className="legend-fuzzy"><Hexagon size={15} weight="fill" /><span>模糊 / 到期</span></div>
      <div className="legend-new"><Circle size={15} /><span>未学</span></div>
      <div className="legend-schema"><Rectangle size={17} /><span>知识架构</span></div>
      <hr />
      <div><ArrowRight size={19} /><span>观测</span></div>
      <div className="legend-prediction"><ArrowRight size={19} /><span>预测</span></div>
      <div className="legend-prior"><ArrowRight size={19} /><span>先验</span></div>
      <small>淡化与虚线表示低置信</small>
    </aside>
  );
}

function GlobalOverview({ snapshot }: { snapshot: MapWorkspaceSnapshot }) {
  const maximum = Math.max(1, ...snapshot.aggregates.map((item) => item.concept_count));
  return (
    <div className="atlas-global" aria-label="全局 Pack 与维度聚合">
      <header><span>全局诊断</span><h2>哪些领域正在形成，哪些已经到期。</h2></header>
      <div className="atlas-global__chart">
        {snapshot.aggregates.map((item) => (
          <article key={`${item.kind}:${item.id}`} className="atlas-global__row" data-kind={item.kind} data-concept-count={item.concept_count}>
            <div><small>{item.kind === "pack" ? "PACK" : "能力维度"}</small><h3>{item.label}</h3></div>
            <div className="atlas-global__bar"><span style={{ width: `${String(Math.max(4, item.mean_value * 100))}%` }} /></div>
            <span>{String(Math.round(item.mean_value * 100))}%</span>
            <span>{String(item.concept_count)} 个概念</span>
            <span>{item.due_count === null ? `置信 ${String(Math.round(item.mean_confidence * 100))}%` : `${String(item.due_count)} 到期`}</span>
            <i style={{ width: `${String(Math.max(4, (item.concept_count / maximum) * 100))}%` }} />
          </article>
        ))}
      </div>
    </div>
  );
}

export function MapWorkspaceView({
  snapshot,
  onFocusNode = () => undefined,
  initialNodeId = null,
}: {
  snapshot: MapWorkspaceSnapshot;
  onFocusNode?: (id: string) => void;
  initialNodeId?: string | null;
}) {
  const [search, setSearch] = useState("");
  const [keyboardOpen, setKeyboardOpen] = useState(false);
  const initialSelection = snapshot.nodes.find((node) => node.id === initialNodeId) ?? snapshot.nodes.find((node) => node.due_status === "due" && nodeUnderstanding(node) === "fuzzy") ?? snapshot.nodes.at(0);
  const [selectedId, setSelectedId] = useState<string | null>(initialSelection?.id ?? null);
  const normalizedSearch = search.trim().toLocaleLowerCase();
  const visibleNodes = snapshot.nodes.filter((node) =>
    normalizedSearch.length === 0 || node.name.toLocaleLowerCase().includes(normalizedSearch) || node.id.toLocaleLowerCase().includes(normalizedSearch)
  );
  const visibleIds = useMemo(() => new Set(visibleNodes.map((node) => node.id)), [visibleNodes]);
  const selected = visibleNodes.find((node) => node.id === selectedId) ?? visibleNodes.at(0) ?? null;

  if (snapshot.view === "global") return <GlobalOverview snapshot={snapshot} />;

  return (
    <div className="atlas-workspace">
      <label className="atlas-search">
        <MagnifyingGlass size={18} />
        <span className="sr-only">搜索当前页</span>
        <input value={search} onChange={(event) => { setSearch(event.target.value); }} placeholder="搜索当前页" aria-label="搜索当前页" />
      </label>
      <MapCanvas snapshot={snapshot} visibleIds={visibleIds} selectedId={selected?.id ?? null} onSelect={(id) => { setSelectedId(id); }} />
      <CompassRose className="atlas-compass" size={48} weight="thin" aria-hidden="true" />
      <AtlasLegend />
      <button className="atlas-keyboard-toggle" type="button" aria-expanded={keyboardOpen} onClick={() => { setKeyboardOpen((value) => !value); }}><Keyboard size={18} /> 键盘浏览</button>
      <div className={`map-node-list ${keyboardOpen ? "map-node-list--open" : ""}`} aria-label="可键盘访问的地图节点">
        {visibleNodes.map((node) => (
          <button key={node.id} type="button" aria-pressed={node.id === selected?.id} onClick={() => { setSelectedId(node.id); }}>
            <strong>{node.name}</strong>
            <span>{understandingLabel(node)} · {node.phase_label || node.phase} · {node.due_status}{primaryLayer(node)?.gate_status !== "active" ? " · 低置信" : ""}</span>
          </button>
        ))}
      </div>
      {selected && <DecisionDeck node={selected} paths={snapshot.paths} anchors={snapshot.anchors} onFocus={() => { onFocusNode(selected.id); }} />}
    </div>
  );
}

function AtlasHeader({ pack, generatedAt }: { pack: string | null; generatedAt: string }) {
  const [commandOpen, setCommandOpen] = useState(false);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen((value) => !value);
      }
      if (event.key === "Escape") setCommandOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);
  return (
    <header className="atlas-header">
      <Link className="atlas-brand" to="/" aria-label="Polaris Today"><CompassRose size={31} weight="duotone" /><strong>POLARIS</strong><span>哪里清楚，哪里还模糊。</span></Link>
      <nav className="atlas-primary-nav" aria-label="工作区导航">
        <NavLink to="/"><span>Today</span></NavLink>
        <NavLink to="/map" className="active"><MapTrifold size={19} /><span>Map</span></NavLink>
        <NavLink to="/practice"><Target size={19} /><span>Practice</span></NavLink>
        <NavLink to="/goals"><Path size={19} /><span>Goals</span></NavLink>
      </nav>
      <div className="atlas-context">
        <button className="atlas-pack" type="button"><MapTrifold size={17} /> {pack === "algorithms" ? "算法基础 · 核心包" : `Pack · ${pack ?? "全部"}`} <CaretDown size={14} /></button>
        <button className="atlas-command-button" type="button" aria-expanded={commandOpen} onClick={() => { setCommandOpen((value) => !value); }}><MagnifyingGlass size={18} /> <span>搜索或输入命令</span><kbd>⌘ K</kbd></button>
        {commandOpen && (
          <div className="atlas-command-menu" aria-label="Polaris 全局工作区">
            <strong>继续学习闭环</strong>
            <small>地图定位模糊后，进入证据、练习、画像与治理。</small>
            {PRODUCT_DESTINATIONS.map((item) => <Link key={item.to} to={item.to} onClick={() => { setCommandOpen(false); }}><span>{item.label}</span><small>{item.description}</small></Link>)}
            <span className="atlas-command-freshness">本地 Core · 更新于 {new Date(generatedAt).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}</span>
          </div>
        )}
      </div>
    </header>
  );
}

export function MapPage() {
  const [searchParams] = useSearchParams();
  const requestedConcept = searchParams.get("concept");
  const [view, setView] = useState<MapView>("current");
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [phase, setPhase] = useState("");
  const [due, setDue] = useState("");
  const [confidence, setConfidence] = useState("");
  const [root, setRoot] = useState<string | null>(null);
  const [cursor, setCursor] = useState<string | null>(null);
  const [history, setHistory] = useState<Array<string | null>>([]);
  const activeFilterCount = [phase, due, confidence].filter(Boolean).length;
  const query: MapWorkspaceQuery = {
    view,
    pack: null,
    root,
    depth: root ? 2 : null,
    phase: phase || null,
    due: due || null,
    min_confidence: confidence ? Number(confidence) : null,
    limit: 100,
    cursor,
  };
  const map = useQuery({ queryKey: commandKeys.map(query), queryFn: () => getMapWorkspace(query), staleTime: 15_000, retry: 1 });

  const resetPage = () => {
    setCursor(null);
    setHistory([]);
  };

  if (map.isPending) return <div className="route-loading atlas-route-state" role="status">正在展开本地知识地形…</div>;
  if (map.error) {
    const error = normalizeCommandError(map.error);
    return <div className="route-error atlas-route-state" role="alert"><strong>地图暂时不可用</strong><span>{error.message}</span><button type="button" onClick={() => void map.refetch()}>重试</button></div>;
  }

  return (
    <section className="map-atlas-page" aria-labelledby="map-title">
      <AtlasHeader pack={map.data.resolved_pack} generatedAt={map.data.generated_at} />
      <div className="atlas-body">
        <aside className="atlas-view-rail" aria-label="地图视图">
          {(["current", "prediction", "global"] as const).map((item) => {
            const Icon = item === "current" ? MapTrifold : item === "prediction" ? Path : GlobeHemisphereWest;
            return (
            <button key={item} type="button" aria-pressed={view === item} onClick={() => { setView(item); setRoot(null); resetPage(); }}>
                <Icon size={23} weight={view === item ? "fill" : "regular"} /><span>{VIEW_COPY[item].label}</span>
              </button>
            );
          })}
        </aside>
        <div className="atlas-main">
          <h1 id="map-title" className="sr-only">{VIEW_COPY[view].label}</h1>
          <div className="atlas-toolbar">
            {view !== "global" && <button className="atlas-filter-button" type="button" aria-expanded={filtersOpen} onClick={() => { setFiltersOpen((value) => !value); }}><Funnel size={18} /> 筛选{activeFilterCount > 0 && <b>{activeFilterCount}</b>} <CaretDown size={14} /></button>}
            <span>{view === "global" ? `${String(map.data.aggregates.length)} 组聚合` : VIEW_COPY[view].description}</span>
            {map.data.theta_mode && <span className="atlas-theta">{map.data.theta_mode} θ · {map.data.theta_mode === "isolated" ? "仅当前 Pack" : "允许跨域"}</span>}
            {root && <button type="button" className="atlas-neighborhood-exit" onClick={() => { setRoot(null); resetPage(); }}>退出两跳邻域 · {root}</button>}
            {filtersOpen && view !== "global" && (
              <div className="atlas-filters" aria-label="地图筛选">
                <label>相<select value={phase} onChange={(event) => { setPhase(event.target.value); resetPage(); }}><option value="">全部</option>{PHASES.map((item) => <option key={item}>{item}</option>)}</select></label>
                <label>到期<select value={due} onChange={(event) => { setDue(event.target.value); resetPage(); }}><option value="">全部</option><option value="due">已到期</option><option value="scheduled">已安排</option><option value="new">未学</option><option value="unscheduled">未安排</option></select></label>
                <label>最低置信<select value={confidence} onChange={(event) => { setConfidence(event.target.value); resetPage(); }}><option value="">不限</option><option value="0.25">25%</option><option value="0.5">50%</option><option value="0.75">75%</option></select></label>
              </div>
            )}
          </div>
          <MapWorkspaceView key={`${view}:${root ?? "all"}:${cursor ?? "first"}:${requestedConcept ?? "default"}`} snapshot={map.data} initialNodeId={requestedConcept} onFocusNode={(id) => { setRoot(id); resetPage(); }} />
          {view !== "global" && <nav className="atlas-pagination" aria-label="地图分页">
            <span>本页 {String(map.data.returned_nodes)} / {map.data.total_nodes.toLocaleString("zh-CN")}</span>
            <button type="button" disabled={history.length === 0} onClick={() => { const previous = history.at(-1) ?? null; setHistory((items) => items.slice(0, -1)); setCursor(previous); }}>上一页</button>
            <button type="button" disabled={!map.data.next_cursor} onClick={() => { setHistory((items) => [...items, cursor]); setCursor(map.data.next_cursor); }}>下一页 <ArrowRight size={16} /></button>
          </nav>}
        </div>
      </div>
    </section>
  );
}
