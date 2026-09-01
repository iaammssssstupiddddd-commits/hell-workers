"""Verify the sealed historical-P02 and current-wall reference locators."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path, PurePosixPath
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_CONTRACT = (
    REPO_ROOT
    / "tools/blender_ai_workflow/fixtures/wall-reference-locators-v1.json"
)


class ContractError(ValueError):
    """Raised when reference evidence differs from the sealed locator contract."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"{label} must be a regular file: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot read {label} JSON {path}: {error}") from error
    require(isinstance(payload, dict), f"{label} must be a JSON object: {path}")
    return payload


def sha256_file(path: Path) -> str:
    require(path.is_file() and not path.is_symlink(), f"artifact must be a regular file: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_sha256(value: Any, label: str) -> str:
    require(
        isinstance(value, str)
        and len(value) == 64
        and value == value.lower()
        and all(character in "0123456789abcdef" for character in value),
        f"{label} must be a lowercase SHA-256 digest",
    )
    return value


def relative_path(value: Any, label: str) -> Path:
    require(isinstance(value, str) and value, f"{label} must be a non-empty path")
    pure = PurePosixPath(value)
    require(not pure.is_absolute() and ".." not in pure.parts, f"{label} must stay below its root")
    return Path(*pure.parts)


def require_exact_keys(payload: dict[str, Any], expected: set[str], label: str) -> None:
    require(set(payload) == expected, f"{label} keys differ: {sorted(payload)}")


def verify_file(root: Path, entry: dict[str, Any], label: str) -> Path:
    require_exact_keys(entry, {"path", "sha256"}, label)
    path = root / relative_path(entry["path"], f"{label}.path")
    expected = require_sha256(entry["sha256"], f"{label}.sha256")
    require(sha256_file(path) == expected, f"{label} hash differs: {path}")
    return path


def parse_checksum_ledger(path: Path) -> dict[str, str]:
    require(path.is_file() and not path.is_symlink(), f"checksum ledger must be a regular file: {path}")
    rows: dict[str, str] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ContractError(f"cannot read checksum ledger {path}: {error}") from error
    require(lines, "checksum ledger must not be empty")
    for line_number, line in enumerate(lines, start=1):
        require(len(line) > 66 and line[64:66] == "  ", f"invalid checksum row {line_number}")
        digest = require_sha256(line[:64], f"checksum row {line_number}")
        locator = line[66:]
        relative_path(locator, f"checksum row {line_number} path")
        require(locator not in rows, f"duplicate checksum locator: {locator}")
        rows[locator] = digest
    return rows


def require_ledger_rows(
    ledger: dict[str, str], entries: list[dict[str, Any]], label: str
) -> None:
    for entry in entries:
        locator = entry["path"]
        require(
            ledger.get(locator) == entry["sha256"],
            f"{label} ledger row differs: {locator}",
        )


def verify_historical_p02(contract: dict[str, Any], root: Path) -> dict[str, Any]:
    require(root.is_dir() and not root.is_symlink(), f"historical P02 root must be a directory: {root}")
    require_exact_keys(
        contract,
        {
            "role",
            "authority",
            "logical_locator",
            "artifact_root",
            "not_valid_for",
            "baseline_index",
            "checksum_ledger",
            "stage",
            "renderdoc",
        },
        "historical_p02",
    )
    require(contract["role"] == "historical-p02-presentation-contract", "historical P02 role differs")
    require(contract["authority"] == "registered-rtt-light-baseline-index", "historical P02 authority differs")
    require(
        contract["logical_locator"] == "rtt-light-v1/baseline-index.json#/stages/p02",
        "historical P02 logical locator differs",
    )
    require(
        contract["artifact_root"] == "target/perf-runs/rtt-light/rtt-light-v1",
        "historical P02 artifact root differs",
    )
    require(
        contract["not_valid_for"] == ["current-wall-pixels", "current-source-runtime-state"],
        "historical P02 scope exclusions differ",
    )

    index_path = verify_file(root, contract["baseline_index"], "baseline_index")
    ledger_path = verify_file(root, contract["checksum_ledger"], "checksum_ledger")
    index = load_json_object(index_path, "baseline index")
    require(index.get("schema_version") == 1 and index.get("contract_id") == "rtt-light-v1", "baseline index contract differs")
    registered = index.get("stages", {}).get("p02")
    require(isinstance(registered, dict), "baseline index has no registered p02 stage")

    stage = contract["stage"]
    require_exact_keys(
        stage,
        {
            "attempt_id",
            "subject_commit",
            "source_fingerprint",
            "attempt_manifest",
            "attempt_directory_sha256",
            "renderdoc_case_directory_sha256",
        },
        "historical_p02.stage",
    )
    require(registered.get("status") == "valid", "registered p02 stage is not valid")
    for field in ("attempt_id", "subject_commit", "source_fingerprint"):
        require(registered.get(field) == stage[field], f"registered p02 {field} differs")
    require(registered.get("attempt_manifest") == stage["attempt_manifest"], "registered p02 attempt manifest differs")
    cases = registered.get("cases", {})
    require(cases.get("attempt", {}).get("directory_sha256") == stage["attempt_directory_sha256"], "registered p02 attempt directory digest differs")
    renderdoc_case = cases.get("renderdoc-medium-gpu", {})
    require(renderdoc_case.get("status") == "valid", "registered p02 RenderDoc case is not valid")
    require(renderdoc_case.get("directory_sha256") == stage["renderdoc_case_directory_sha256"], "registered p02 RenderDoc directory digest differs")

    renderdoc = contract["renderdoc"]
    require_exact_keys(
        renderdoc,
        {"relative_root", "manifest", "runtime_checkpoint", "extraction", "capture", "expected_presentation"},
        "historical_p02.renderdoc",
    )
    expected_renderdoc_root = str(relative_path(renderdoc["relative_root"], "renderdoc.relative_root"))
    require(renderdoc_case.get("path") == expected_renderdoc_root, "registered p02 RenderDoc path differs")

    ledger = parse_checksum_ledger(ledger_path)
    artifact_entries = [
        contract["baseline_index"],
        stage["attempt_manifest"],
        renderdoc["manifest"],
        renderdoc["runtime_checkpoint"],
        renderdoc["extraction"],
        renderdoc["capture"],
    ]
    require_ledger_rows(ledger, artifact_entries, "historical P02")
    attempt_path = verify_file(root, stage["attempt_manifest"], "p02 attempt manifest")
    manifest_path = verify_file(root, renderdoc["manifest"], "p02 RenderDoc manifest")
    checkpoint_path = verify_file(root, renderdoc["runtime_checkpoint"], "p02 runtime checkpoint")
    verify_file(root, renderdoc["extraction"], "p02 RenderDoc extraction")
    capture_entry = renderdoc["capture"]
    require_exact_keys(capture_entry, {"path", "sha256", "bytes"}, "p02 RenderDoc capture")
    capture_path = root / relative_path(capture_entry["path"], "p02 RenderDoc capture.path")
    require(
        capture_path.is_file() and not capture_path.is_symlink(),
        f"p02 RenderDoc capture must be a regular file: {capture_path}",
    )
    require(capture_path.stat().st_size == capture_entry["bytes"], "p02 RenderDoc capture size differs")
    require(sha256_file(capture_path) == capture_entry["sha256"], "p02 RenderDoc capture hash differs")

    attempt = load_json_object(attempt_path, "p02 attempt manifest")
    require(
        attempt.get("schema_version") == 1
        and attempt.get("stage_id") == "p02"
        and attempt.get("status") == "valid"
        and attempt.get("attempt_id") == stage["attempt_id"]
        and attempt.get("subject_commit") == stage["subject_commit"]
        and attempt.get("source_fingerprint") == stage["source_fingerprint"]
        and attempt.get("raw_directory_sha256") == stage["attempt_directory_sha256"],
        "p02 attempt manifest identity differs",
    )
    manifest = load_json_object(manifest_path, "p02 RenderDoc manifest")
    require(
        manifest.get("schema_version") == 1
        and manifest.get("stage_id") == "p02"
        and manifest.get("case_id") == "renderdoc-medium-gpu"
        and manifest.get("status") == "valid"
        and manifest.get("unexpected_log_lines") == 0,
        "p02 RenderDoc manifest identity differs",
    )
    require(
        manifest.get("source")
        == {"clean": True, "commit": stage["subject_commit"], "fingerprint": stage["source_fingerprint"]},
        "p02 RenderDoc source identity differs",
    )
    require(manifest.get("capture", {}).get("sha256") == capture_entry["sha256"], "p02 RenderDoc manifest capture hash differs")
    require(manifest.get("capture", {}).get("bytes") == capture_entry["bytes"], "p02 RenderDoc manifest capture size differs")

    checkpoint = load_json_object(checkpoint_path, "p02 runtime checkpoint")
    require(
        checkpoint.get("schema_version") == 3
        and checkpoint.get("stage_id") == "p02"
        and checkpoint.get("status") == "valid",
        "p02 runtime checkpoint identity differs",
    )
    require(checkpoint.get("p02_presentation") == renderdoc["expected_presentation"], "historical p02 presentation contract differs")
    require(checkpoint.get("capture_artifact") == {"sha256": capture_entry["sha256"], "bytes": capture_entry["bytes"]}, "p02 checkpoint capture identity differs")
    return {
        "attempt_id": stage["attempt_id"],
        "subject_commit": stage["subject_commit"],
        "capture_sha256": capture_entry["sha256"],
    }


def verify_current_wall(contract: dict[str, Any], root: Path) -> dict[str, Any]:
    require(root.is_dir() and not root.is_symlink(), f"current wall root must be a directory: {root}")
    require_exact_keys(
        contract,
        {
            "role",
            "authority",
            "logical_locator",
            "artifact_root",
            "not_valid_for",
            "profile",
            "subject_commit",
            "source_fingerprint",
            "harness_fingerprint",
            "asset_view_fingerprint",
            "binary_sha256",
            "manifest",
            "observation",
            "screenshot",
            "expected_subject",
        },
        "current_wall",
    )
    require(contract["role"] == "current-fallback-wall-visual-reference", "current wall role differs")
    require(contract["authority"] == "wall-art-current-calibration-v1-manifest", "current wall authority differs")
    require(
        contract["logical_locator"]
        == "target/native-acceptance/wall-art-20260901T055138Z-f93b0b12/manifest.json",
        "current wall logical locator differs",
    )
    require(
        contract["artifact_root"]
        == "target/native-acceptance/wall-art-20260901T055138Z-f93b0b12",
        "current wall artifact root differs",
    )
    require(
        contract["not_valid_for"] == ["historical-p02-performance", "production-wall-art-approval"],
        "current wall scope exclusions differ",
    )
    manifest_path = verify_file(root, contract["manifest"], "current wall manifest")
    observation_path = verify_file(root, contract["observation"], "current wall observation")
    screenshot = contract["screenshot"]
    require_exact_keys(screenshot, {"path", "sha256", "bytes"}, "current wall screenshot")
    screenshot_path = root / relative_path(screenshot["path"], "current wall screenshot.path")
    require(
        screenshot_path.is_file() and not screenshot_path.is_symlink(),
        f"current wall screenshot must be a regular file: {screenshot_path}",
    )
    require(screenshot_path.stat().st_size == screenshot["bytes"], "current wall screenshot size differs")
    require(sha256_file(screenshot_path) == screenshot["sha256"], "current wall screenshot hash differs")

    manifest = load_json_object(manifest_path, "current wall manifest")
    expected_manifest = {
        "schema_version": 1,
        "status": "pass",
        "profile": contract["profile"],
        "subject_commit": contract["subject_commit"],
        "source_fingerprint": contract["source_fingerprint"],
        "harness_fingerprint": contract["harness_fingerprint"],
        "asset_view_fingerprint": contract["asset_view_fingerprint"],
        "binary_sha256": contract["binary_sha256"],
        "screenshot_sha256": screenshot["sha256"],
    }
    for field, expected in expected_manifest.items():
        require(manifest.get(field) == expected, f"current wall manifest {field} differs")

    observation = load_json_object(observation_path, "current wall observation")
    require(
        observation.get("schema_version") == 1
        and observation.get("status") == "pass"
        and observation.get("profile") == contract["profile"],
        "current wall observation identity differs",
    )
    fixture = observation.get("probe_status", {}).get("fixture", {})
    subject = contract["expected_subject"]
    require_exact_keys(subject, {"ordinal", "grid", "mask", "roi"}, "current wall expected_subject")
    require(
        fixture.get("subject_ordinal") == subject["ordinal"]
        and fixture.get("subject_grid") == subject["grid"]
        and fixture.get("subject_mask") == subject["mask"],
        "current wall subject differs",
    )
    require(observation.get("probe_status", {}).get("probe", {}).get("roi") == subject["roi"], "current wall ROI differs")
    require(observation.get("screenshot", {}).get("sha256") == screenshot["sha256"], "current wall observation screenshot hash differs")
    return {
        "profile": contract["profile"],
        "subject_commit": contract["subject_commit"],
        "screenshot_sha256": screenshot["sha256"],
    }


def verify_reference_locators(
    *, contract_path: Path, baseline_root: Path | None = None, current_root: Path | None = None
) -> dict[str, Any]:
    contract = load_json_object(contract_path, "wall reference locator contract")
    require_exact_keys(contract, {"schema_version", "contract_id", "historical_p02", "current_wall"}, "wall reference locator contract")
    require(contract["schema_version"] == 1, "wall reference locator schema_version must be 1")
    require(contract["contract_id"] == "wall-reference-locators-v1", "wall reference locator contract_id differs")
    historical = contract["historical_p02"]
    current = contract["current_wall"]
    baseline_root = baseline_root or REPO_ROOT / relative_path(historical["artifact_root"], "historical_p02.artifact_root")
    current_root = current_root or REPO_ROOT / relative_path(current["artifact_root"], "current_wall.artifact_root")
    return {
        "schema_version": 1,
        "status": "pass",
        "contract_id": contract["contract_id"],
        "historical_p02": verify_historical_p02(historical, baseline_root),
        "current_wall": verify_current_wall(current, current_root),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--baseline-root", type=Path)
    parser.add_argument("--current-root", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        report = verify_reference_locators(
            contract_path=args.contract,
            baseline_root=args.baseline_root,
            current_root=args.current_root,
        )
    except ContractError as error:
        print(f"wall reference locator verification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
