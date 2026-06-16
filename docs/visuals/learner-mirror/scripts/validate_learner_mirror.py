from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[4]
MIRROR_DIR = ROOT / "docs" / "visuals" / "learner-mirror"
DATA = MIRROR_DIR / "data" / "sample.json"

REQUIRED_FILES = [
    MIRROR_DIR / "index.html",
    MIRROR_DIR / "styles.css",
    MIRROR_DIR / "app.js",
    DATA,
    MIRROR_DIR / "README.md",
]

REQUIRED_TOP_LEVEL_KEYS = [
    "meta",
    "generated_at",
    "confidence_curve",
    "phase_distribution",
    "recent_assertions",
]

REQUIRED_PHASES = [
    "undetermined",
    "phantom",
    "fluctuation",
    "settling",
    "solidification",
    "transfer",
    "generation",
    "regression",
]

FORBIDDEN_META_KEYS = {
    "user_id",
    "email",
    "database_path",
    "source_db",
    "session_id",
}


def fail(message: str) -> None:
    raise SystemExit(message)


def read_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        fail(f"missing data file: {path}")
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"invalid json: {error}")
    if not isinstance(data, dict):
        fail("sample data must be a JSON object")
    return data


def require_string(value: Any, field: str) -> None:
    if not isinstance(value, str) or not value.strip():
        fail(f"{field} must be a non-empty string")


def require_number_0_1(value: Any, field: str) -> None:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        fail(f"{field} must be a number")
    if value < 0 or value > 1:
        fail(f"{field} must be between 0 and 1")


def validate_files() -> None:
    for path in REQUIRED_FILES:
        if not path.exists():
            fail(f"missing static file: {path}")
        if path.is_file() and path.stat().st_size == 0:
            fail(f"empty static file: {path}")


def validate_meta(data: dict[str, Any]) -> None:
    for key in REQUIRED_TOP_LEVEL_KEYS:
        if key not in data:
            fail(f"missing top-level key: {key}")

    meta = data["meta"]
    if not isinstance(meta, dict):
        fail("meta must be an object")
    if meta.get("project") != "polaris-core":
        fail("meta.project must be polaris-core")
    if meta.get("fixture") is not True:
        fail("meta.fixture must be true")
    if meta.get("synthetic") is not True:
        fail("meta.synthetic must be true")
    require_string(meta.get("privacy_note"), "meta.privacy_note")

    forbidden = sorted(FORBIDDEN_META_KEYS.intersection(meta))
    if forbidden:
        fail(f"sample meta contains forbidden user data keys: {', '.join(forbidden)}")


def validate_curve(data: dict[str, Any]) -> None:
    points = data["confidence_curve"]
    if not isinstance(points, list) or len(points) < 5:
        fail("confidence_curve must contain at least 5 points")
    previous_time = ""
    seen_final = False
    seen_provisional = False
    for index, point in enumerate(points):
        if not isinstance(point, dict):
            fail(f"confidence_curve[{index}] must be an object")
        require_string(point.get("attempt_id"), f"confidence_curve[{index}].attempt_id")
        require_string(point.get("created_at"), f"confidence_curve[{index}].created_at")
        require_string(point.get("concept_id"), f"confidence_curve[{index}].concept_id")
        require_number_0_1(point.get("confidence"), f"confidence_curve[{index}].confidence")
        require_number_0_1(point.get("actual_score"), f"confidence_curve[{index}].actual_score")
        if not isinstance(point.get("is_final"), bool):
            fail(f"confidence_curve[{index}].is_final must be a boolean")
        seen_final = seen_final or point["is_final"]
        seen_provisional = seen_provisional or not point["is_final"]
        if previous_time and point["created_at"] < previous_time:
            fail("confidence_curve must be sorted by created_at ascending")
        previous_time = point["created_at"]
    if not seen_final or not seen_provisional:
        fail("confidence_curve must include both final and provisional points")


def validate_phase_distribution(data: dict[str, Any]) -> None:
    phases = data["phase_distribution"]
    if not isinstance(phases, list):
        fail("phase_distribution must be a list")
    phase_names = []
    total = 0
    for index, phase in enumerate(phases):
        if not isinstance(phase, dict):
            fail(f"phase_distribution[{index}] must be an object")
        require_string(phase.get("phase"), f"phase_distribution[{index}].phase")
        require_string(phase.get("label"), f"phase_distribution[{index}].label")
        require_string(phase.get("summary"), f"phase_distribution[{index}].summary")
        count = phase.get("count")
        if not isinstance(count, int) or count < 0:
            fail(f"phase_distribution[{index}].count must be a non-negative integer")
        phase_names.append(phase["phase"])
        total += count
    missing = [phase for phase in REQUIRED_PHASES if phase not in phase_names]
    if missing:
        fail(f"phase_distribution missing phases: {', '.join(missing)}")
    if total <= 0:
        fail("phase_distribution must contain at least one concept")


def validate_assertions(data: dict[str, Any]) -> None:
    assertions = data["recent_assertions"]
    if not isinstance(assertions, list) or not assertions:
        fail("recent_assertions must be a non-empty list")
    for index, assertion in enumerate(assertions):
        if not isinstance(assertion, dict):
            fail(f"recent_assertions[{index}] must be an object")
        for key in ["id", "kind", "claim", "suggested_action"]:
            require_string(assertion.get(key), f"recent_assertions[{index}].{key}")
        require_number_0_1(assertion.get("confidence"), f"recent_assertions[{index}].confidence")


def main() -> int:
    validate_files()
    data = read_json(DATA)
    validate_meta(data)
    validate_curve(data)
    validate_phase_distribution(data)
    validate_assertions(data)
    print("learner mirror validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
