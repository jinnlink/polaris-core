import { fireEvent, render, screen } from "@testing-library/react";
import axe from "axe-core";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import type { MapWorkspaceSnapshot } from "../contracts/core";
import { graphElements, MapWorkspaceView } from "./MapPage";

const cytoscapeMock = vi.hoisted(() => vi.fn((options: unknown) => {
  void options;
  return {
    on: vi.fn(),
    off: vi.fn(),
    destroy: vi.fn(),
    $id: vi.fn(() => ({ select: vi.fn(), empty: vi.fn(() => false), renderedPosition: vi.fn(() => ({ x: 100, y: 100 })) })),
    zoom: vi.fn(() => 1),
    fit: vi.fn(),
    panBy: vi.fn(),
    ready: vi.fn((callback: () => void) => { callback(); }),
  };
}));

vi.mock("cytoscape", () => ({ default: cytoscapeMock }));

const snapshot: MapWorkspaceSnapshot = {
  generated_at: "2026-08-09T09:00:00Z",
  view: "prediction",
  resolved_pack: "algorithms",
  theta_mode: "isolated",
  total_nodes: 10_000,
  returned_nodes: 2,
  next_cursor: "cursor:100",
  nodes: [
    {
      id: "graphs",
      name: "图",
      kind: "concept",
      pack: "algorithms",
      phase: "acquisition",
      phase_label: "正在形成",
      phase_summary: "能识别，但还需要主动回忆。",
      due_status: "due",
      attempt_count: 2,
      evidence_count: 1,
      layers: [{
        source: "observed",
        cross_domain: false,
        value: 0.62,
        confidence: 0.72,
        lower: null,
        upper: null,
        gate_status: "active",
        model_version: "knowledge-map-v1",
        origin: "mastery_states",
        evidence_ids: ["evidence-1"],
        provenance_complete: true,
      }],
    },
    {
      id: "shortest_path",
      name: "最短路径",
      kind: "schema",
      pack: "algorithms",
      phase: "undetermined",
      phase_label: "证据不足",
      phase_summary: "当前只有跨域预测。",
      due_status: "new",
      attempt_count: 0,
      evidence_count: 0,
      layers: [{
        source: "latent_prediction",
        cross_domain: false,
        value: 0.48,
        confidence: null,
        lower: 0.18,
        upper: 0.78,
        gate_status: "shadow",
        model_version: "prediction-map-v1",
        origin: "pack:algorithms",
        evidence_ids: [],
        provenance_complete: true,
      }],
    },
  ],
  edges: [{
    id: "edge-1",
    source_id: "graphs",
    target_id: "shortest_path",
    kind: "prerequisite",
    weight: 1,
    origin: "pack:algorithms",
    evidence_ids: [],
  }],
  aggregates: [],
  anchors: [],
  paths: [],
};

describe("MapWorkspaceView", () => {
  it("keeps a 10k graph bounded to the current page and labels low confidence", () => {
    const elements = graphElements(snapshot, new Set(["shortest_path"]));

    expect(elements).toHaveLength(1);
    expect(elements[0]?.classes).toContain("low-confidence");
    expect(elements[0]?.classes).toContain("latent_prediction");

    const complete = graphElements(snapshot, new Set(["graphs", "shortest_path"]));
    expect(complete).toHaveLength(3);
    expect(complete[1]?.classes).toContain("schema");
    expect(complete[2]?.data).toMatchObject({
      source: "graphs",
      target: "shortest_path",
      label: "先修",
    });
  });

  it("offers keyboard nodes, evidence jump and an accessible inspector", async () => {
    const onFocusNode = vi.fn();
    const { container } = render(
      <MemoryRouter>
        <MapWorkspaceView snapshot={snapshot} onFocusNode={onFocusNode} />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText("可键盘访问的地图节点").querySelectorAll("button")).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "键盘浏览" }));
    fireEvent.click(screen.getByRole("button", { name: /证据与来源/ }));
    expect(screen.getByRole("link", { name: "证据 evidence-1" })).toHaveAttribute("href", "/inbox?evidence=evidence-1");
    expect(screen.getByRole("button", { name: /最短路径.*低置信/ })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "只看两跳邻域" }));
    expect(onFocusNode).toHaveBeenCalledWith("graphs");

    const result = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(result.violations).toEqual([]);
    const graphOptions = cytoscapeMock.mock.calls[0]?.[0] as {
      maxZoom: number;
      autoungrabify: boolean;
      layout: { animate: boolean };
    };
    expect(graphOptions.maxZoom).toBe(2.5);
    expect(graphOptions.autoungrabify).toBe(true);
    expect(graphOptions.layout.animate).toBe(false);
  });

  it("searches only the loaded page without requesting or rendering all nodes", () => {
    render(
      <MemoryRouter>
        <MapWorkspaceView snapshot={snapshot} />
      </MemoryRouter>,
    );

    fireEvent.change(screen.getByPlaceholderText("搜索当前页"), { target: { value: "shortest" } });
    fireEvent.click(screen.getByRole("button", { name: "键盘浏览" }));
    expect(screen.getByLabelText("可键盘访问的地图节点").querySelectorAll("button")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /最短路径/ })).toBeVisible();
  });

  it("opens the concept requested by the product handoff route", () => {
    render(
      <MemoryRouter>
        <MapWorkspaceView snapshot={snapshot} initialNodeId="graphs" />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "键盘浏览" }));
    expect(screen.getByRole("button", { name: /^图模糊/ })).toHaveAttribute("aria-pressed", "true");
  });

  it("renders global Pack and dimension aggregates without a giant graph", () => {
    const global = {
      ...snapshot,
      view: "global",
      nodes: [],
      edges: [],
      aggregates: [{
        id: "algorithms",
        label: "Algorithms",
        kind: "pack",
        concept_count: 10_000,
        due_count: 128,
        observed_count: 2_400,
        mean_value: 0.61,
        mean_confidence: 0.74,
      }],
    };
    render(
      <MemoryRouter>
        <MapWorkspaceView snapshot={global} />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Algorithms" })).toBeVisible();
    expect(screen.getByText("10000 个概念")).toBeVisible();
    expect(screen.queryByRole("img", { name: /知识地图/ })).not.toBeInTheDocument();
  });
});
