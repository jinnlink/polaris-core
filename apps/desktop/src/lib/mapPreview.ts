import type {
  MapWorkspaceLayer,
  MapWorkspaceNode,
  MapWorkspaceQuery,
  MapWorkspaceSnapshot,
} from "../contracts/core";

function layer(
  source: "observed" | "latent_prediction" | "inherited_prior",
  value: number,
  gateStatus: "active" | "shadow" | "prior_only" = "active",
  evidenceIds: string[] = [],
): MapWorkspaceLayer {
  const predicted = source !== "observed";
  return {
    source,
    cross_domain: source === "latent_prediction",
    value,
    confidence: predicted ? null : Math.min(0.94, 0.48 + evidenceIds.length * 0.13),
    lower: predicted ? Math.max(0.04, value - 0.18) : null,
    upper: predicted ? Math.min(0.96, value + 0.18) : null,
    gate_status: gateStatus,
    model_version: predicted ? "prediction-map-v1" : "knowledge-map-v1",
    origin: source === "observed" ? "learning-evidence" : "pack:algorithms",
    evidence_ids: evidenceIds,
    provenance_complete: true,
  };
}

const previewNodeRows: Array<[string, string, string, string, string, string, number, number, MapWorkspaceLayer]> = [
  ["graph-representation", "图的表示", "concept", "transfer", "能够迁移", "scheduled", 8, 4, layer("observed", 0.86, "active", ["ev-graph-1", "ev-graph-2"])],
  ["adjacency", "邻接表与邻接矩阵", "concept", "transfer", "能够迁移", "scheduled", 7, 3, layer("observed", 0.82, "active", ["ev-adj-1"])],
  ["priority-queue", "优先队列（最小堆）", "concept", "consolidation", "正在稳固", "scheduled", 6, 2, layer("observed", 0.76, "active", ["ev-heap-1"])],
  ["greedy", "贪心策略", "concept", "consolidation", "正在稳固", "scheduled", 5, 2, layer("observed", 0.72, "active", ["ev-greedy-1"])],
  ["dijkstra", "Dijkstra 最短路径", "concept", "acquisition", "正在形成", "due", 3, 2, layer("observed", 0.46, "shadow", ["ev-dijkstra-1", "ev-dijkstra-2"])],
  ["shortest-path-schema", "最短路径问题范式", "schema", "acquisition", "正在形成", "due", 2, 1, layer("observed", 0.52, "shadow", ["ev-schema-1"])],
  ["astar", "A* 启发式搜索", "concept", "undetermined", "证据不足", "new", 0, 0, layer("latent_prediction", 0.54, "shadow")],
  ["bellman-ford", "Bellman–Ford", "concept", "undetermined", "证据不足", "new", 0, 0, layer("latent_prediction", 0.43, "shadow")],
  ["johnson", "Johnson 全源最短路", "concept", "undetermined", "证据不足", "new", 0, 0, layer("inherited_prior", 0.34, "prior_only")],
  ["mst", "最小生成树", "concept", "undetermined", "证据不足", "new", 0, 0, layer("inherited_prior", 0.41, "prior_only")],
  ["dynamic-programming", "动态规划", "concept", "acquisition", "正在形成", "unscheduled", 2, 1, layer("latent_prediction", 0.48, "shadow")],
  ["union-find", "并查集", "concept", "undetermined", "证据不足", "new", 0, 0, layer("inherited_prior", 0.38, "prior_only")],
];

const previewNodes: MapWorkspaceNode[] = previewNodeRows.map(([id, name, kind, phase, phaseLabel, dueStatus, attempts, evidence, estimate]) => ({
  id,
  name,
  kind,
  pack: "algorithms",
  phase,
  phase_label: phaseLabel,
  phase_summary: id === "dijkstra" ? "能写出基本实现，但负权边与复杂图上的推理仍不稳定。" : "当前判断由本地学习证据与模型状态共同支持。",
  due_status: dueStatus,
  attempt_count: attempts,
  evidence_count: evidence,
  layers: [estimate],
}));

const previewEdgeRows: Array<[string, string, string, string]> = [
  ["e1", "graph-representation", "adjacency", "prerequisite"],
  ["e2", "adjacency", "dijkstra", "prerequisite"],
  ["e3", "priority-queue", "dijkstra", "prerequisite"],
  ["e4", "greedy", "dijkstra", "prerequisite"],
  ["e5", "shortest-path-schema", "dijkstra", "maps_to"],
  ["e6", "dijkstra", "astar", "maps_to"],
  ["e7", "dijkstra", "bellman-ford", "contrast"],
  ["e8", "dijkstra", "johnson", "prerequisite"],
  ["e9", "bellman-ford", "johnson", "prerequisite"],
  ["e10", "greedy", "mst", "maps_to"],
  ["e11", "union-find", "mst", "prerequisite"],
  ["e12", "dynamic-programming", "bellman-ford", "contrast"],
];

const previewEdges = previewEdgeRows.map(([id, sourceId, targetId, kind]) => ({
  id,
  source_id: sourceId,
  target_id: targetId,
  kind,
  weight: 1,
  origin: "preview:algorithms",
  evidence_ids: [],
}));

export function previewMapWorkspace(query: MapWorkspaceQuery): MapWorkspaceSnapshot {
  if (query.view === "global") {
    return {
      generated_at: new Date().toISOString(),
      view: "global",
      resolved_pack: null,
      theta_mode: null,
      total_nodes: 10_000,
      returned_nodes: 0,
      next_cursor: null,
      nodes: [],
      edges: [],
      aggregates: [
        { id: "algorithms", label: "Algorithms", kind: "pack", concept_count: 3280, due_count: 42, observed_count: 1240, mean_value: 0.68, mean_confidence: 0.79 },
        { id: "rust", label: "Rust", kind: "pack", concept_count: 2840, due_count: 28, observed_count: 990, mean_value: 0.74, mean_confidence: 0.83 },
        { id: "english", label: "English", kind: "pack", concept_count: 3880, due_count: 67, observed_count: 1480, mean_value: 0.61, mean_confidence: 0.71 },
        { id: "reasoning", label: "结构推理", kind: "dimension", concept_count: 2160, due_count: null, observed_count: null, mean_value: 0.57, mean_confidence: 0.7 },
        { id: "transfer", label: "迁移能力", kind: "dimension", concept_count: 1790, due_count: null, observed_count: null, mean_value: 0.49, mean_confidence: 0.64 },
      ],
      anchors: [],
      paths: [],
    };
  }

  let nodes = previewNodes;
  if (query.root) {
    const neighborIds = new Set([query.root]);
    for (const edge of previewEdges) {
      if (edge.source_id === query.root) neighborIds.add(edge.target_id);
      if (edge.target_id === query.root) neighborIds.add(edge.source_id);
    }
    nodes = nodes.filter((node) => neighborIds.has(node.id));
  }
  if (query.phase) nodes = nodes.filter((node) => node.phase === query.phase);
  if (query.due) nodes = nodes.filter((node) => node.due_status === query.due);
  const minimumConfidence = query.min_confidence;
  if (minimumConfidence !== null) {
    nodes = nodes.filter((node) => {
      const estimate = node.layers.at(0);
      return (estimate?.confidence ?? estimate?.value ?? 0) >= minimumConfidence;
    });
  }
  if (query.view === "prediction") {
    nodes = nodes.map((node) => {
      const current = node.layers.at(0);
      const evidenceIds = current?.evidence_ids ?? [];
      const value = current?.value ?? 0.45;
      return {
        ...node,
        layers: [
          ...(node.attempt_count > 0 ? [layer("observed", value, "active", evidenceIds)] : []),
          layer("latent_prediction", Math.min(0.9, value + 0.08), "shadow"),
          layer("inherited_prior", 0.42, "prior_only"),
        ],
      };
    });
  }
  const nodeIds = new Set(nodes.map((node) => node.id));

  return {
    generated_at: new Date().toISOString(),
    view: query.view,
    resolved_pack: "algorithms",
    theta_mode: query.view === "prediction" ? "shared" : null,
    total_nodes: 10_000,
    returned_nodes: nodes.length,
    next_cursor: query.cursor ? null : "preview:next",
    nodes,
    edges: previewEdges.filter((edge) => nodeIds.has(edge.source_id) && nodeIds.has(edge.target_id)),
    aggregates: [],
    anchors: query.view === "prediction" ? [{
      id: "anchor-greedy-dijkstra",
      source_concept_id: "greedy",
      source_name: "贪心策略",
      source_pack: "algorithms",
      target_id: "dijkstra",
      target_name: "Dijkstra 最短路径",
      target_pack: "algorithms",
      structural_score: 0.82,
      difference: "可迁移局部最优结构，但必须验证负权边与松弛顺序的差异。",
      origin: "preview:maps_to",
      evidence_ids: ["ev-greedy-1"],
    }] : [],
    paths: query.view === "prediction" ? [
      { rank: 1, concept_id: "astar", concept_name: "A* 启发式搜索", move_name: "边界变式与启发函数", expected_success: 0.63 },
      { rank: 2, concept_id: "bellman-ford", concept_name: "Bellman–Ford", move_name: "负权边对比练习", expected_success: 0.58 },
      { rank: 3, concept_id: "johnson", concept_name: "Johnson 全源最短路", move_name: "组合迁移", expected_success: 0.51 },
    ] : [],
  };
}
