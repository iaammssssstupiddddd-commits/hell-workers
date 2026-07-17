from __future__ import annotations

from .execution import *

def load_valid_runs(session_dir: Path) -> list[tuple[Path, Validation]]:
    runs: list[tuple[Path, Validation]] = []
    for validation_path in sorted((session_dir / "cases").glob("*/run-*/validation.json")):
        payload = json.loads(validation_path.read_text(encoding="utf-8"))
        validation = Validation(
            valid=bool(payload["valid"]),
            reasons=list(payload["reasons"]),
            summary=payload.get("summary"),
            adapter=payload.get("adapter"),
            warning_lines=list(payload.get("warning_lines", [])),
            teardown_warning_lines=list(payload.get("teardown_warning_lines", [])),
            determinism=payload.get("determinism"),
            scene_roots=payload.get("scene_roots"),
        )
        runs.append((validation_path.parent, validation))
    return runs


def median_and_mad(values: list[float]) -> tuple[float, float]:
    center = statistics.median(values)
    return center, statistics.median([abs(value - center) for value in values])


def reset_checksum_policy(runs: list[tuple[Path, Validation]]) -> bool:
    """Restore per-run validation before applying the session checksum policy.

    This lets `summarize --warmup-checksum-policy ...` safely switch between
    recording and requiring a dynamic warm-up checkpoint without masking an
    unrelated capture failure.
    """
    changed = False
    for run_dir, validation in runs:
        reasons = [
            reason
            for reason in validation.reasons
            if not reason.startswith(CHECKSUM_POLICY_REASON_PREFIXES)
        ]
        valid = not reasons
        if reasons != validation.reasons or valid != validation.valid:
            validation.reasons = reasons
            validation.valid = valid
            write_json(run_dir / "validation.json", validation.to_json())
            changed = True
    return changed


def apply_checksum_policy(
    runs: list[tuple[Path, Validation]],
    warmup_policy: str,
    measure_end_policy: str,
) -> bool:
    by_case: dict[str, list[tuple[Path, Validation]]] = {}
    for run_dir, validation in runs:
        by_case.setdefault(run_dir.parent.name, []).append((run_dir, validation))
    changed = False
    for case_runs in by_case.values():
        initial_checksums = {
            validation.summary["initial_state_checksum"]
            for _, validation in case_runs
            if validation.valid and validation.summary is not None
        }
        if len(initial_checksums) > 1:
            reason = "initial_state_checksum differs across repeated runs: " + ", ".join(
                sorted(initial_checksums)
            )
            for run_dir, validation in case_runs:
                if not validation.valid:
                    continue
                validation.valid = False
                validation.reasons.append(reason)
                write_json(run_dir / "validation.json", validation.to_json())
                changed = True

        for field, policy in (
            ("warmup_state_checksum", warmup_policy),
            ("measure_end_state_checksum", measure_end_policy),
        ):
            if policy != "require":
                continue
            checksums = {
                validation.summary[field]
                for _, validation in case_runs
                if validation.valid and validation.summary is not None
            }
            if len(checksums) <= 1:
                continue
            reason = f"{field} differs across repeated runs: " + ", ".join(sorted(checksums))
            for run_dir, validation in case_runs:
                if not validation.valid:
                    continue
                validation.valid = False
                validation.reasons.append(reason)
                write_json(run_dir / "validation.json", validation.to_json())
                changed = True
    return changed


def determinism_signature(checkpoints: list[dict[str, str]]) -> str:
    fields = DETERMINISM_COLUMNS
    serialized = "\n".join(
        ",".join(checkpoint[field] for field in fields) for checkpoint in checkpoints
    )
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()[:16]


def apply_determinism_policy(runs: list[tuple[Path, Validation]]) -> bool:
    by_case: dict[str, list[tuple[Path, Validation]]] = {}
    for run_dir, validation in runs:
        by_case.setdefault(run_dir.parent.name, []).append((run_dir, validation))

    changed = False
    for case_runs in by_case.values():
        signatures = {
            determinism_signature(validation.determinism)
            for _, validation in case_runs
            if validation.valid and validation.determinism is not None
        }
        if len(signatures) <= 1:
            continue
        reason = "determinism checkpoints differ across repeated runs: " + ", ".join(
            sorted(signatures)
        )
        for run_dir, validation in case_runs:
            if not validation.valid:
                continue
            validation.valid = False
            validation.reasons.append(reason)
            write_json(run_dir / "validation.json", validation.to_json())
            changed = True
    return changed
