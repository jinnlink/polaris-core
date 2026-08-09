import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import type { TodaySnapshot } from "../contracts/core";
import { TodayView } from "./TodayPage";

const snapshot: TodaySnapshot = {
  generated_at: "2026-08-09T09:00:00Z",
  current_pack: "rust",
  theta_mode: "isolated",
  packs: [
    { id: "rust", title: "Rust", concept_count: 14, active: true, theta_mode: "isolated" },
  ],
  top_signal: {
    claim: "所有权边界仍需要一次主动回忆。",
    confidence: 0.82,
    suggested_action: "先做一道借用检查题。",
  },
  actions: [
    {
      id: "practice:ownership",
      kind: "practice",
      title: "所有权",
      detail: "主动回忆 · retrieval",
      route: "/practice",
      concept_id: "ownership",
      expected_success: 0.74,
    },
    {
      id: "fallback:inbox",
      kind: "inbox",
      title: "处理学习收件箱",
      detail: "选一条线索。",
      route: "/inbox",
      concept_id: null,
      expected_success: null,
    },
    {
      id: "fallback:rest",
      kind: "rest",
      title: "休息一下",
      detail: "状态会保留。",
      route: null,
      concept_id: null,
      expected_success: null,
    },
  ],
  notification_policy: {
    state_gate_passed: true,
    dominant_state: "flow",
    suppress_non_error: true,
  },
};

describe("TodayView", () => {
  it("renders exactly three choices with signal and flow protection", () => {
    render(
      <MemoryRouter>
        <TodayView snapshot={snapshot} switching={false} onSwitchPack={vi.fn()} onRest={vi.fn()} />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText("今天的可选行动").children).toHaveLength(3);
    expect(screen.getByText("所有权边界仍需要一次主动回忆。")).toBeVisible();
    expect(screen.getByText("心流保护中 · 已静音")).toBeVisible();
    expect(screen.getByText("isolated 模式")).toBeVisible();
  });

  it("switches packs and lets rest hide the compact window", () => {
    const onSwitchPack = vi.fn();
    const onRest = vi.fn();
    const withSecondPack = {
      ...snapshot,
      packs: [
        ...snapshot.packs,
        {
          id: "algorithms",
          title: "Algorithms",
          concept_count: 17,
          active: false,
          theta_mode: "shared",
        },
      ],
    };
    render(
      <MemoryRouter>
        <TodayView snapshot={withSecondPack} switching={false} onSwitchPack={onSwitchPack} onRest={onRest} />
      </MemoryRouter>,
    );

    fireEvent.change(screen.getByLabelText("学习空间"), { target: { value: "algorithms" } });
    fireEvent.click(screen.getByRole("button", { name: /休息一下/ }));

    expect(onSwitchPack).toHaveBeenCalledWith("algorithms");
    expect(onRest).toHaveBeenCalledOnce();
  });
});
