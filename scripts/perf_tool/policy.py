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


CONTROLLED_POLICY_DOWNSTREAM_COUNTERS = (
    "candidate_snapshot_attempts",
    "candidate_score_attempts",
    "worker_score_attempts",
    "source_selector_calls",
    "source_selector_scanned_items",
    "reachable_with_cache_calls",
)


def _checkpoint(
    validation: Validation, checkpoint: str
) -> dict[str, str] | None:
    if validation.determinism is None:
        return None
    return next(
        (
            row
            for row in validation.determinism
            if row.get("checkpoint") == checkpoint
        ),
        None,
    )


def _invalidate_controlled_cases(
    runs: list[tuple[Path, Validation]],
    case_ids: set[str],
    reason: str,
) -> bool:
    changed = False
    for run_dir, validation in runs:
        if run_dir.parent.name not in case_ids or reason in validation.reasons:
            continue
        validation.valid = False
        validation.reasons.append(reason)
        write_json(run_dir / "validation.json", validation.to_json())
        changed = True
    return changed


def apply_familiar_policy_controlled_audit(
    session_dir: Path,
    manifest: dict[str, Any],
    runs: list[tuple[Path, Validation]],
) -> bool:
    matrix = manifest["matrix"]
    policies = set(matrix.get("familiar_policies", ["baseline"]))
    dialogs = set(matrix.get("operation_dialog_modes", ["hidden"]))
    comparison_path = session_dir / "familiar_policy_comparison.json"
    if policies != {"default", "disabled"} or dialogs != {"hidden", "open"}:
        if comparison_path.exists():
            comparison_path.unlink()
        return False

    case_metadata = {
        case["id"]: case
        for case in manifest.get("cases", [])
    }
    groups: dict[tuple[Any, ...], dict[tuple[str, str], str]] = {}
    for case_id, case in case_metadata.items():
        contract = (
            case["workload"],
            case["size"],
            case["render"],
            case["seed"],
            case.get("souls"),
            case.get("familiars"),
        )
        groups.setdefault(contract, {})[
            (case["familiar_policy"], case["operation_dialog"])
        ] = case_id

    valid_by_case: dict[str, Validation] = {}
    for run_dir, validation in runs:
        if validation.valid and validation.determinism is not None:
            valid_by_case.setdefault(run_dir.parent.name, validation)

    expected_modes = {
        ("default", "hidden"),
        ("default", "open"),
        ("disabled", "hidden"),
        ("disabled", "open"),
    }
    changed = False
    group_results: list[dict[str, Any]] = []
    all_reasons: list[str] = []
    for contract, mode_cases in sorted(groups.items()):
        case_ids = set(mode_cases.values())
        failures: list[str] = []
        if set(mode_cases) != expected_modes:
            failures.append("matrix does not contain the four required policy/dialog cases")
        validations = {
            mode: valid_by_case.get(case_id)
            for mode, case_id in mode_cases.items()
        }
        if any(validation is None for validation in validations.values()):
            failures.append("one or more controlled cases has no valid deterministic run")

        counters: dict[str, dict[str, int]] = {}
        if not failures:
            for policy in ("default", "disabled"):
                hidden = validations[(policy, "hidden")]
                opened = validations[(policy, "open")]
                assert hidden is not None and opened is not None
                if determinism_signature(hidden.determinism) != determinism_signature(
                    opened.determinism
                ):
                    failures.append(
                        f"{policy} dialog hidden/open determinism or AI work differs"
                    )

            default_hidden = validations[("default", "hidden")]
            disabled_hidden = validations[("disabled", "hidden")]
            assert default_hidden is not None and disabled_hidden is not None
            default_initial = _checkpoint(default_hidden, "fixture-pre-update")
            disabled_initial = _checkpoint(disabled_hidden, "fixture-pre-update")
            default_work = _checkpoint(default_hidden, "post-warmup")
            disabled_work = _checkpoint(disabled_hidden, "post-warmup")
            if None in (default_initial, disabled_initial, default_work, disabled_work):
                failures.append("required fixture-pre-update or post-warmup checkpoint is absent")
            else:
                assert default_initial is not None
                assert disabled_initial is not None
                assert default_work is not None
                assert disabled_work is not None
                if default_initial["structural_checksum"] != disabled_initial[
                    "structural_checksum"
                ]:
                    failures.append(
                        "default/disabled controlled fixtures have different structural checksums"
                    )
                if default_initial["state_checksum"] == disabled_initial["state_checksum"]:
                    failures.append(
                        "default/disabled policy state is not represented in the audit checksum"
                    )

                counters = {
                    "default": {
                        field: int(default_work[field])
                        for field in (
                            "candidate_membership_checks",
                            "policy_disabled_rejections",
                            *CONTROLLED_POLICY_DOWNSTREAM_COUNTERS,
                        )
                    },
                    "disabled": {
                        field: int(disabled_work[field])
                        for field in (
                            "candidate_membership_checks",
                            "policy_disabled_rejections",
                            *CONTROLLED_POLICY_DOWNSTREAM_COUNTERS,
                        )
                    },
                }
                if counters["default"]["candidate_membership_checks"] <= 0:
                    failures.append("default policy never reached the candidate membership gate")
                if counters["default"]["policy_disabled_rejections"] != 0:
                    failures.append("default policy unexpectedly rejected a candidate")
                disabled_membership = counters["disabled"]["candidate_membership_checks"]
                if disabled_membership <= 0:
                    failures.append("disabled policy never reached the candidate membership gate")
                if (
                    counters["disabled"]["policy_disabled_rejections"]
                    != disabled_membership
                ):
                    failures.append(
                        "disabled policy did not stop every observed candidate at the policy gate"
                    )
                for counter in CONTROLLED_POLICY_DOWNSTREAM_COUNTERS:
                    default_value = counters["default"][counter]
                    disabled_value = counters["disabled"][counter]
                    if default_value <= 0:
                        failures.append(
                            f"default controlled fixture did not exercise {counter}"
                        )
                    if disabled_value != 0:
                        failures.append(
                            f"disabled policy performed downstream work in {counter}"
                        )
                    if disabled_value >= default_value:
                        failures.append(
                            f"disabled policy did not reduce {counter}"
                        )

        if failures:
            reason = (
                "familiar policy controlled comparison failed: "
                + "; ".join(failures)
            )
            changed |= _invalidate_controlled_cases(runs, case_ids, reason)
            all_reasons.extend(failures)
        group_results.append(
            {
                "contract": {
                    "workload": contract[0],
                    "size": contract[1],
                    "render": contract[2],
                    "seed": contract[3],
                    "souls": contract[4],
                    "familiars": contract[5],
                },
                "status": "fail" if failures else "pass",
                "failures": failures,
                "post_warmup_counters": counters,
            }
        )

    result = {
        "schema_version": 1,
        "status": "fail" if all_reasons else "pass",
        "checkpoint": "post-warmup",
        "required_policy_modes": ["default", "disabled"],
        "required_operation_dialog_modes": ["hidden", "open"],
        "groups": group_results,
        "failures": all_reasons,
    }
    write_json(comparison_path, result)
    return changed
