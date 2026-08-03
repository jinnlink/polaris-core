from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / "docs" / "visuals" / "atlas" / "data" / "atlas-data.json"


@dataclass
class ModuleNode:
    id: str
    title: str
    group: str
    summary: str
    files: list[str]
    depends_on: list[str] = field(default_factory=list)
    related_tables: list[str] = field(default_factory=list)
    related_tickets: list[str] = field(default_factory=list)
    source_refs: list[str] = field(default_factory=list)
    tier: str = "support"
    status: str = "implemented"


def read_text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def parse_public_modules() -> list[str]:
    lib_rs = read_text("crates/polaris-core/src/lib.rs")
    return re.findall(r"^pub mod ([a-zA-Z0-9_]+);", lib_rs, flags=re.MULTILINE)


def parse_tables() -> list[str]:
    db_rs = read_text("crates/polaris-core/src/db.rs")
    return re.findall(r"CREATE TABLE IF NOT EXISTS ([a-zA-Z0-9_]+)\(", db_rs)


def parse_indexes() -> list[str]:
    db_rs = read_text("crates/polaris-core/src/db.rs")
    return re.findall(r"CREATE (?:UNIQUE )?INDEX IF NOT EXISTS ([a-zA-Z0-9_]+)", db_rs)


MODULE_OVERRIDES: dict[str, dict[str, Any]] = {
    "citation": {
        "group": "Evidence",
        "summary": "Validates strict citation boundaries so report claims stay tied to evidence.",
        "related_tables": ["mirror_reports", "evidence_items"],
        "related_tickets": ["P03I"],
    },
    "config": {
        "group": "Foundation",
        "summary": "Registers typed parameters and keeps behavioral knobs explicit.",
        "related_tables": ["meta"],
        "related_tickets": ["P01", "P03J"],
    },
    "consolidation": {
        "group": "Cognitive Dynamics",
        "summary": "Runs night consolidation and counterfactual replay for slower learning signals.",
        "depends_on": ["db", "mastery", "mirt"],
        "related_tables": ["consolidation_runs", "theta_history", "residual_stats"],
        "related_tickets": ["P03B", "P03J"],
    },
    "db": {
        "group": "Data Ledger",
        "summary": "Owns SQLite schema, migrations, hot-path indexes, and local persistence.",
        "related_tables": ["meta", "op_log", "attempts", "mastery_states"],
        "related_tickets": ["P01", "P03L"],
        "tier": "tier-0",
    },
    "diagnosis": {
        "group": "Evidence",
        "summary": "Turns graph and attempt evidence into concept-level diagnosis.",
        "depends_on": ["graph", "mastery"],
        "related_tables": ["attempts", "mastery_states", "edges"],
        "related_tickets": ["P02B"],
    },
    "engine": {
        "group": "Core Loop",
        "summary": "Coordinates pack init, next-task selection, evidence submission, grading, reports, and maintenance jobs.",
        "depends_on": ["db", "scheduler", "grader", "mastery", "report", "diagnosis"],
        "related_tables": ["attempts", "behavior_events", "grade_queue", "mastery_states"],
        "related_tickets": ["P01", "P02B", "P03G", "P03I"],
        "tier": "tier-0",
    },
    "error": {
        "group": "Foundation",
        "summary": "Centralizes recoverable Polaris Core error types.",
        "related_tickets": ["P01"],
    },
    "fsrs": {
        "group": "Cognitive Dynamics",
        "summary": "Implements FSRS-style memory scheduling state and retrievability.",
        "depends_on": ["scheduler", "mastery"],
        "related_tables": ["mastery_states"],
        "related_tickets": ["P01"],
    },
    "geometry": {
        "group": "Knowledge Space",
        "summary": "Builds geometry-aware candidate sets for targeted learning paths.",
        "depends_on": ["graph", "mastery"],
        "related_tables": ["edges", "mastery_states"],
        "related_tickets": ["P03C"],
    },
    "grader": {
        "group": "Evidence",
        "summary": "Produces engine-owned provisional scores and asynchronous grade-queue outcomes.",
        "depends_on": ["db", "mastery"],
        "related_tables": ["attempts", "grade_queue", "mastery_states"],
        "related_tickets": ["P01", "P02C"],
        "tier": "tier-1",
    },
    "graph": {
        "group": "Knowledge Space",
        "summary": "Represents typed concept edges and prerequisite/confusion topology.",
        "related_tables": ["concepts", "edges"],
        "related_tickets": ["P02A", "P02B"],
    },
    "gu": {
        "group": "Evidence",
        "summary": "Induces and audits G_u rules from repeated evidence patterns.",
        "depends_on": ["db", "diagnosis"],
        "related_tables": ["gu_rules"],
        "related_tickets": ["P03H"],
    },
    "mastery": {
        "group": "Core Loop",
        "summary": "Updates concept mastery, calibration gaps, phase, and review state.",
        "depends_on": ["db", "phase", "fsrs"],
        "related_tables": ["mastery_states", "attempts", "theta"],
        "related_tickets": ["P01", "P03E"],
        "tier": "tier-0",
    },
    "mental_fit": {
        "group": "Cognitive Dynamics",
        "summary": "Fits hazard, HMM gate, and EM mental-dynamics models for audit-only updates.",
        "depends_on": ["mental_state", "db"],
        "related_tables": ["hazard_models", "state_gate_evals"],
        "related_tickets": ["P03K"],
    },
    "mental_state": {
        "group": "Cognitive Dynamics",
        "summary": "Classifies learner state from behavior events through HMM-style signals.",
        "depends_on": ["db"],
        "related_tables": ["behavior_events", "hazard_models"],
        "related_tickets": ["P03D"],
    },
    "mirt": {
        "group": "Cognitive Dynamics",
        "summary": "Maintains latent ability estimates used by diagnosis and consolidation.",
        "depends_on": ["db", "mastery"],
        "related_tables": ["theta", "theta_history"],
        "related_tickets": ["P03A"],
    },
    "moves": {
        "group": "Teaching",
        "summary": "Defines move taxonomy for recall, explain, apply, analyze, evaluate, create, and transfer.",
        "related_tables": ["moves_effects"],
        "related_tickets": ["P03F"],
    },
    "pack": {
        "group": "Pack",
        "summary": "Loads and validates domain packs, concepts, prerequisites, moves, and misconceptions.",
        "depends_on": ["graph", "moves"],
        "related_tables": ["concepts", "edges"],
        "related_tickets": ["P01", "P05A1"],
    },
    "phase": {
        "group": "Knowledge Space",
        "summary": "Classifies knowledge phase so tasks target recall, transfer, and boundary repair.",
        "depends_on": ["mastery"],
        "related_tables": ["mastery_states"],
        "related_tickets": ["P03E"],
    },
    "report": {
        "group": "Evidence",
        "summary": "Builds mirror reports with evidence-bound assertions, hypotheses, and suggestions.",
        "depends_on": ["citation", "db"],
        "related_tables": ["mirror_reports", "attempts", "evidence_items"],
        "related_tickets": ["P03I"],
    },
    "scheduler": {
        "group": "Core Loop",
        "summary": "Chooses next tasks and interleaved batches under memory, phase, and mastery constraints.",
        "depends_on": ["fsrs", "mastery", "phase", "geometry"],
        "related_tables": ["mastery_states", "behavior_events"],
        "related_tickets": ["P01", "P03G"],
        "tier": "tier-0",
    },
    "status": {
        "group": "Entrypoints",
        "summary": "Builds read-only status snapshots for CLI and MCP consumers.",
        "depends_on": ["db", "phase"],
        "related_tables": ["mastery_states", "concepts"],
        "related_tickets": ["P02C"],
    },
    "teaching": {
        "group": "Teaching",
        "summary": "Produces Tier 2 teaching instructions without letting external AI write mastery state.",
        "depends_on": ["diagnosis", "moves"],
        "related_tables": ["attempts", "edges"],
        "related_tickets": ["P02C", "P03F"],
        "tier": "tier-2",
    },
    "tuning": {
        "group": "Cognitive Dynamics",
        "summary": "Runs audit-backed parameter tuning and records accepted or rejected candidates.",
        "depends_on": ["db", "consolidation"],
        "related_tables": ["param_tuning_runs"],
        "related_tickets": ["P03J"],
    },
}

EXTRA_MODULES = [
    ModuleNode(
        id="cli",
        title="CLI Entrypoint",
        group="Entrypoints",
        summary="Exposes init, ingest, next, submit, hint, abandon, status, grading, report, tuning, diagnosis, MCP, and pack validation commands.",
        files=["crates/polaris-cli/src/main.rs"],
        depends_on=["engine", "pack", "fsrs", "phase", "mcp"],
        related_tables=["sessions", "behavior_events", "attempts"],
        related_tickets=["P01", "P02C", "P03I"],
        source_refs=["crates/polaris-cli/src/main.rs"],
        tier="tier-0",
    ),
    ModuleNode(
        id="mcp",
        title="MCP Interface",
        group="Entrypoints",
        summary="Provides get_next_task, get_interleaved_batch, submit_evidence, get_teaching_instruction, and read-only status/diagnosis resources.",
        files=["crates/polaris-cli/src/mcp.rs"],
        depends_on=["engine", "teaching", "diagnosis", "status"],
        related_tables=["behavior_events", "attempts", "mastery_states"],
        related_tickets=["P02C"],
        source_refs=["crates/polaris-cli/src/mcp.rs"],
        tier="tier-2",
    ),
]

TABLE_SUMMARIES = {
    "meta": "Configuration and migration metadata, including typed parameter registration.",
    "op_log": "Append-only operation log for replay and audit boundaries.",
    "evidence_items": "Raw local evidence items ingested from learner or system sources.",
    "attempts": "Learner responses, provisional scores, confidence, latency, and evidence links.",
    "concepts": "Domain pack concepts and initial mastery priors.",
    "edges": "Typed prerequisite, confusion, and transfer relationships.",
    "mastery_states": "Derived mastery state, phase, FSRS JSON, due time, and calibration signals.",
    "sessions": "Learning sessions used to bind behavior events.",
    "behavior_events": "Fine-grained learner behavior such as next, hint, and abandon.",
    "grade_queue": "Asynchronous Tier 1 grading queue and outcomes.",
    "theta": "Current latent ability estimates.",
    "theta_history": "Historical latent ability snapshots for replay and trend analysis.",
    "residual_stats": "Residual audit statistics for model fit and drift.",
    "consolidation_runs": "Nightly consolidation run records.",
    "moves_effects": "Move-level teaching effect estimates.",
    "mrt_log": "Micro-randomized trial audit records.",
    "param_tuning_runs": "Parameter tuning proposals and accepted/rejected outcomes.",
    "gu_rules": "Induced G_u rules with audit lifecycle state.",
    "mirror_reports": "Evidence-bound mirror reports and filtered assertions.",
    "hazard_models": "Mental-dynamics hazard model fit records.",
    "state_gate_evals": "HMM/state gate evaluations for participation decisions.",
}

GUARDRAILS = [
    {
        "id": "sync-no-llm",
        "title": "Synchronous path has no LLM dependency",
        "summary": "Tier 0 learning actions must stay local, deterministic, and fast.",
        "source_refs": ["SPEC.md", "docs/MASTER_PLAN.md"],
    },
    {
        "id": "engine-owned-mastery",
        "title": "External AI cannot write mastery",
        "summary": "LLM output may teach or explain, but mastery state is owned by engine evidence and scoring.",
        "source_refs": ["SPEC.md", "crates/polaris-cli/src/mcp.rs"],
    },
    {
        "id": "strict-citation",
        "title": "Mirror reports need strict citations",
        "summary": "Assertions must carry evidence ids and confidence, or stay out of the report.",
        "source_refs": ["docs/DATA_MODEL.md", "crates/polaris-core/src/citation.rs"],
    },
    {
        "id": "local-persistent",
        "title": "Local-persistent ledger",
        "summary": "Facts are stored locally in SQLite and replayable through evidence and operation records.",
        "source_refs": ["SPEC.md", "crates/polaris-core/src/db.rs"],
    },
    {
        "id": "single-ticket",
        "title": "Single-ticket discipline",
        "summary": "Only one implementation ticket is in progress; queue state remains the source of truth.",
        "source_refs": ["AGENTS.md", "docs/tickets/QUEUE.md"],
    },
]

VIEWS = [
    {
        "id": "overview",
        "title": "Overview",
        "summary": "High-level loop, current status, and fastest paths into the system.",
    },
    {
        "id": "graph",
        "title": "Architecture Graph",
        "summary": "Clickable module and dependency graph with source-linked Inspector.",
    },
    {
        "id": "timeline",
        "title": "Ticket Timeline",
        "summary": "Ticket state is generated from QUEUE so completed, active, and remaining work stay distinct.",
    },
    {
        "id": "data",
        "title": "Data Model",
        "summary": "SQLite tables, indexes, facts, derived state, and audit surfaces.",
    },
    {
        "id": "guardrails",
        "title": "Guardrails",
        "summary": "Rules that keep Polaris evidence-bound, local, and ticket-disciplined.",
    },
]


def module_file(name: str) -> str:
    return f"crates/polaris-core/src/{name}.rs"


def build_modules() -> list[dict[str, Any]]:
    modules: list[ModuleNode] = []
    for name in parse_public_modules():
        override = MODULE_OVERRIDES.get(name, {})
        modules.append(
            ModuleNode(
                id=name,
                title=override.get("title", name.replace("_", " ").title()),
                group=override.get("group", "Core Module"),
                summary=override.get(
                    "summary",
                    f"Polaris Core module exported from crates/polaris-core/src/lib.rs: {name}.",
                ),
                files=[module_file(name)],
                depends_on=override.get("depends_on", []),
                related_tables=override.get("related_tables", []),
                related_tickets=override.get("related_tickets", []),
                source_refs=override.get(
                    "source_refs",
                    ["crates/polaris-core/src/lib.rs", module_file(name)],
                ),
                tier=override.get("tier", "support"),
                status=override.get("status", "implemented"),
            )
        )
    modules.extend(EXTRA_MODULES)
    return [asdict(module) for module in sorted(modules, key=lambda item: (item.group, item.id))]


def build_database() -> list[dict[str, Any]]:
    indexes = parse_indexes()
    hot_path_tables = {
        match.group(1)
        for index in indexes
        for match in [re.match(r"idx_([a-zA-Z0-9]+)_", index)]
        if match
    }
    return [
        {
            "id": table,
            "title": table,
            "summary": TABLE_SUMMARIES.get(table, "SQLite table managed by crates/polaris-core/src/db.rs."),
            "source_refs": ["crates/polaris-core/src/db.rs", "docs/DATA_MODEL.md"],
            "hot_path": table in hot_path_tables,
        }
        for table in parse_tables()
    ]


def build_flows() -> list[dict[str, Any]]:
    return [
        {
            "id": "learning-loop",
            "title": "Learning Loop",
            "summary": "From user-facing task selection to evidence submission, scoring, mastery update, and the next recommendation.",
            "steps": ["cli", "mcp", "engine", "scheduler", "grader", "mastery", "report"],
            "source_refs": ["crates/polaris-cli/src/main.rs", "crates/polaris-core/src/engine.rs"],
        },
        {
            "id": "evidence-replay",
            "title": "Evidence Replay",
            "summary": "Local facts remain replayable through append-only evidence, behavior, attempts, and derived mastery state.",
            "steps": ["evidence_items", "behavior_events", "attempts", "grade_queue", "mastery_states"],
            "source_refs": ["docs/DATA_MODEL.md", "crates/polaris-core/src/db.rs"],
        },
        {
            "id": "agent-interface",
            "title": "Agent Interface",
            "summary": "MCP tools expose the same local loop while preserving engine-owned scoring and Tier 2 teaching boundaries.",
            "steps": ["mcp", "engine", "teaching", "diagnosis", "status"],
            "source_refs": ["crates/polaris-cli/src/mcp.rs"],
        },
        {
            "id": "nightly-audit",
            "title": "Nightly Audit Loop",
            "summary": "Slow-path consolidation, fitting, tuning, and mirror reports audit model assumptions without blocking Tier 0.",
            "steps": ["consolidation", "mental_fit", "tuning", "gu", "report"],
            "source_refs": ["docs/DATA_MODEL.md", "docs/tickets/QUEUE.md"],
        },
    ]


def build_tickets() -> list[dict[str, str]]:
    queue = read_text("docs/tickets/QUEUE.md")
    tickets: list[dict[str, str]] = []
    phase = ""
    phase_pattern = re.compile(r"^## Phase\s+(\d+)", re.IGNORECASE)
    ticket_pattern = re.compile(
        r"^- \[(?P<done>[ xX])\] \*\*(?P<id>P[0-9]+[A-Z0-9]*)\s+(?P<title>.+?)\*\*"
    )
    for line in queue.splitlines():
        phase_match = phase_pattern.match(line)
        if phase_match:
            phase = f"P{int(phase_match.group(1)):02d}"
            continue
        if line.startswith("## Backlog"):
            break
        ticket_match = ticket_pattern.match(line)
        if not ticket_match or not phase:
            continue
        if ticket_match.group("done").lower() == "x":
            status = "completed"
        elif "In Progress" in line:
            status = "in_progress"
        else:
            status = "queued"
        service_match = re.search(r"服务环节：([^；\n]+)", line)
        tickets.append(
            {
                "id": ticket_match.group("id"),
                "title": ticket_match.group("title"),
                "phase": phase,
                "status": status,
                "service_loop": service_match.group(1).strip() if service_match else "",
                "source_refs": "docs/tickets/QUEUE.md",
            }
        )
    return tickets


def existing_generated_at() -> str | None:
    if not OUT.exists():
        return None
    try:
        return json.loads(OUT.read_text(encoding="utf-8"))["meta"]["generated_at"]
    except (KeyError, json.JSONDecodeError, TypeError):
        return None


def build_data(generated_at: str | None = None) -> dict[str, Any]:
    timestamp = generated_at or datetime.now(timezone.utc).isoformat()
    tickets = build_tickets()
    completed_count = sum(ticket["status"] == "completed" for ticket in tickets)
    active = [ticket["id"] for ticket in tickets if ticket["status"] == "in_progress"]
    remaining_count = len(tickets) - completed_count
    return {
        "meta": {
            "project": "polaris-core",
            "generated_at": timestamp,
            "design": "Polaris Porcelain Intelligence Atlas",
            "phase_note": (
                f"{completed_count} tickets completed; "
                f"active: {', '.join(active) if active else 'none'}; "
                f"{remaining_count} tickets remain. Source: docs/tickets/QUEUE.md."
            ),
            "visual_direction": "Radix Air Sage + Quiet Motion System",
        },
        "modules": build_modules(),
        "flows": build_flows(),
        "database": build_database(),
        "tickets": tickets,
        "guardrails": GUARDRAILS,
        "views": VIEWS,
    }


def encode(data: dict[str, Any]) -> str:
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Polaris Atlas data.")
    parser.add_argument("--write", action="store_true", help="Write atlas-data.json.")
    parser.add_argument("--check", action="store_true", help="Fail if atlas-data.json is stale.")
    args = parser.parse_args()

    if args.check:
        if not OUT.exists():
            print("atlas-data.json is missing")
            return 1
        expected = encode(build_data(generated_at=existing_generated_at()))
        current = OUT.read_text(encoding="utf-8")
        if current != expected:
            print("atlas-data.json is out of date")
            return 1
        print("atlas-data.json is current")
        return 0

    data = build_data()
    output = encode(data)
    if args.write:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(output, encoding="utf-8")
        print(f"wrote {OUT}")
        return 0

    print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
