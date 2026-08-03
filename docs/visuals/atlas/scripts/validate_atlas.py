from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
ATLAS_DIR = ROOT / "docs" / "visuals" / "atlas"
DATA = ATLAS_DIR / "data" / "atlas-data.json"
REQUIRED_FRONTEND_FILES = [
    ATLAS_DIR / "index.html",
    ATLAS_DIR / "styles.css",
    ATLAS_DIR / "app.js",
]
REQUIRED_TOP_LEVEL_KEYS = [
    "meta",
    "modules",
    "flows",
    "database",
    "tickets",
    "guardrails",
    "views",
]


def fail(message: str) -> None:
    raise SystemExit(message)


def load_data() -> dict[str, Any]:
    if not DATA.exists():
        fail(f"missing data file: {DATA}")
    try:
        payload = json.loads(DATA.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"invalid json: {error}")
    if not isinstance(payload, dict):
        fail("atlas data must be a JSON object")
    return payload


def validate_top_level(data: dict[str, Any]) -> None:
    for key in REQUIRED_TOP_LEVEL_KEYS:
        if key not in data:
            fail(f"missing top-level key: {key}")
    if data["meta"].get("project") != "polaris-core":
        fail("meta.project must be polaris-core")
    if not data["modules"]:
        fail("modules must not be empty")


def validate_files(data: dict[str, Any]) -> None:
    for file_path in REQUIRED_FRONTEND_FILES:
        if not file_path.exists():
            fail(f"missing frontend file: {file_path}")

    for module in data["modules"]:
        module_id = module.get("id", "<missing id>")
        for file_name in module.get("files", []):
            if file_name.startswith("virtual:"):
                continue
            if not (ROOT / file_name).exists():
                fail(f"missing module file for {module_id}: {file_name}")


def validate_statuses(data: dict[str, Any]) -> None:
    allowed = {"completed", "in_progress", "queued"}
    active = []
    for ticket in data.get("tickets", []):
        ticket_id = ticket.get("id", "")
        status = ticket.get("status", "")
        if status not in allowed:
            fail(f"unknown ticket status for {ticket_id}: {status}")
        if status == "in_progress":
            active.append(ticket_id)
    if len(active) > 1:
        fail(f"multiple in-progress tickets: {', '.join(active)}")


def validate_relationships(data: dict[str, Any]) -> None:
    module_ids = {module["id"] for module in data["modules"]}
    for module in data["modules"]:
        for dependency in module.get("depends_on", []):
            if dependency not in module_ids:
                fail(f"unknown dependency for {module['id']}: {dependency}")
    for flow in data.get("flows", []):
        if not flow.get("steps"):
            fail(f"flow has no steps: {flow.get('id')}")


def main() -> int:
    data = load_data()
    validate_top_level(data)
    validate_files(data)
    validate_statuses(data)
    validate_relationships(data)
    print("atlas validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
