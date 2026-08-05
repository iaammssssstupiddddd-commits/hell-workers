from __future__ import annotations

from .execution import *

def validate_session_artifact_set(
    session_dir: Path, manifest: dict[str, Any]
) -> list[str]:
    """Validate the exact case/run set before any aggregation.

    Manifest schema v2 makes missing runs, unknown cases, invalid preflights,
    and duplicate case identifiers session-fatal. Older artifacts retain their
    legacy summarization behavior and cannot be used as a v2 formal bundle.
    """
    if manifest.get("schema_version", 1) < SESSION_MANIFEST_SCHEMA_VERSION:
        return []

    errors: list[str] = []
    matrix = manifest.get("matrix")
    cases = manifest.get("cases")
    if not isinstance(matrix, dict):
        return ["manifest schema v2 requires a matrix object"]
    if not isinstance(cases, list) or not cases:
        return ["manifest schema v2 requires a nonempty cases list"]

    rtt_light_matrix = matrix.get("rtt_light_contract")
    behavior_manifest = (
        matrix.get("capture_kind") == "fixed-step-behavior"
        or (
            isinstance(rtt_light_matrix, dict)
            and rtt_light_matrix.get("lane") == "behavior"
        )
        or any(
            isinstance(case, dict) and case.get("behavior_case") is not None
            for case in cases
        )
    )
    if behavior_manifest:
        contract = load_rtt_light_contract("rtt-light-v1")
        expected_behavior_cases = contract["stages"]["current"][
            "required_behavior_cases"
        ]
        rtt_selection = matrix.get("rtt_light_contract")
        exact_matrix = {
            "capture_kind": matrix.get("capture_kind"),
            "workload": matrix.get("workload"),
            "sizes": matrix.get("sizes"),
            "renders": matrix.get("renders"),
            "seed": matrix.get("seed"),
            "repeat": matrix.get("repeat"),
            "preflight_runs": matrix.get("preflight_runs"),
            "fixed_hz": matrix.get("fixed_hz"),
            "clock_mode": matrix.get("clock_mode"),
            "behavior_cases": matrix.get("behavior_cases"),
        }
        if exact_matrix != {
            "capture_kind": "fixed-step-behavior",
            "workload": "indoor-light",
            "sizes": ["small"],
            "renders": ["cpu"],
            "seed": contract["formal_matrix"]["seed"],
            "repeat": contract["behavior_fixture"]["repeat"],
            "preflight_runs": contract["formal_matrix"]["behavior"][
                "preflight_runs"
            ],
            "fixed_hz": contract["formal_matrix"]["fixed_hz"],
            "clock_mode": "fixed-behavior",
            "behavior_cases": expected_behavior_cases,
        }:
            errors.append("behavior manifest matrix differs from the canonical contract")
        if not isinstance(rtt_selection, dict) or (
            rtt_selection.get("contract_id"),
            rtt_selection.get("stage_id"),
            rtt_selection.get("lane"),
        ) != ("rtt-light-v1", "current", "behavior"):
            errors.append("behavior manifest has the wrong RtT-light selection")
        requested_environment = manifest.get("requested_environment")
        if not isinstance(requested_environment, dict) or {
            key: requested_environment.get(key)
            for key in ("HW_PRESENT_MODE", "HW_WINDOW_BACKEND", "WGPU_BACKEND")
        } != {
            "HW_PRESENT_MODE": contract["formal_matrix"]["present_mode"],
            "HW_WINDOW_BACKEND": "headless",
            "WGPU_BACKEND": contract["formal_matrix"]["backend"],
        }:
            errors.append("behavior manifest environment differs from the canonical contract")
        binary = manifest.get("binary")
        if not isinstance(binary, dict) or binary.get("instrumentation") != "capture":
            errors.append("behavior manifest instrumentation must be capture")
        observed_behavior_cases = [
            case.get("behavior_case") for case in cases if isinstance(case, dict)
        ]
        if observed_behavior_cases != expected_behavior_cases:
            errors.append("behavior manifest cases differ from the canonical contract")
        for case in cases:
            if not isinstance(case, dict):
                continue
            if any(
                (
                    case.get("workload") != "indoor-light",
                    case.get("size") != "small",
                    case.get("render") != "cpu",
                    case.get("seed") != contract["formal_matrix"]["seed"],
                    case.get("souls") is not None,
                    case.get("familiars") is not None,
                    case.get("familiar_policy") != "baseline",
                    case.get("operation_dialog") != "hidden",
                    case.get("dashboard_mode") != "hidden",
                )
            ):
                errors.append("behavior manifest case metadata differs from the contract")
                break

    case_ids = [case.get("id") for case in cases if isinstance(case, dict)]
    if len(case_ids) != len(cases) or any(not isinstance(case_id, str) for case_id in case_ids):
        errors.append("manifest cases must each contain a string id")
        return errors
    duplicate_case_ids = sorted(
        case_id for case_id in set(case_ids) if case_ids.count(case_id) > 1
    )
    if duplicate_case_ids:
        errors.append("manifest contains duplicate case ids: " + ", ".join(duplicate_case_ids))

    cases_dir = session_dir / "cases"
    actual_case_ids = (
        {path.name for path in cases_dir.iterdir() if path.is_dir()}
        if cases_dir.is_dir()
        else set()
    )
    expected_case_ids = set(case_ids)
    missing_cases = sorted(expected_case_ids - actual_case_ids)
    unknown_cases = sorted(actual_case_ids - expected_case_ids)
    if missing_cases:
        errors.append("missing case directories: " + ", ".join(missing_cases))
    if unknown_cases:
        errors.append("unknown case directories: " + ", ".join(unknown_cases))

    repeat = matrix.get("repeat")
    preflight_runs = matrix.get("preflight_runs")
    if not isinstance(repeat, int) or repeat < 1:
        errors.append("matrix repeat must be a positive integer")
        return errors
    if not isinstance(preflight_runs, int) or preflight_runs < 0:
        errors.append("matrix preflight_runs must be a nonnegative integer")
        return errors

    expected_labels = {
        *(f"run-{index:03d}" for index in range(1, repeat + 1)),
        *(f"preflight-{index:03d}" for index in range(1, preflight_runs + 1)),
    }
    for case_id in sorted(expected_case_ids & actual_case_ids):
        case_dir = cases_dir / case_id
        actual_labels = {
            path.name
            for path in case_dir.iterdir()
            if path.is_dir() and not path.name.startswith(".")
        }
        missing_labels = sorted(expected_labels - actual_labels)
        unknown_labels = sorted(actual_labels - expected_labels)
        if missing_labels:
            errors.append(f"{case_id} missing runs: " + ", ".join(missing_labels))
        if unknown_labels:
            errors.append(f"{case_id} has unknown runs: " + ", ".join(unknown_labels))

        for label in sorted(expected_labels & actual_labels):
            validation_path = case_dir / label / "validation.json"
            if not validation_path.is_file():
                errors.append(f"{case_id}/{label} is missing validation.json")
                continue
            try:
                payload = json.loads(validation_path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as error:
                errors.append(f"{case_id}/{label} has invalid validation.json: {error}")
                continue
            if label.startswith("preflight-") and payload.get("valid") is not True:
                reasons = payload.get("reasons")
                reason_text = "; ".join(reasons) if isinstance(reasons, list) else "invalid"
                errors.append(f"{case_id}/{label} failed: {reason_text}")

    return errors

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
            determinism_records=payload.get("determinism_records"),
            scene_roots=payload.get("scene_roots"),
            render_inventory=payload.get("render_inventory"),
            window=payload.get("window"),
            indoor_light_fixture=payload.get("indoor_light_fixture"),
            indoor_light_layout=payload.get("indoor_light_layout"),
            indoor_light_presentation=payload.get("indoor_light_presentation"),
            timeline=payload.get("timeline"),
            behavior_save_artifact=payload.get("behavior_save_artifact"),
            profile_artifact=payload.get("profile_artifact"),
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

DASHBOARD_RENDER_COUNTERS = (
    "dashboard_render_rebuilds",
    "dashboard_render_input_rows",
    "dashboard_render_visible_rows",
    "dashboard_render_group_headers",
    "dashboard_despawn_roots_requested",
)
DASHBOARD_NON_VACUOUS_AI_COUNTERS = (
    "candidate_membership_checks",
    "candidate_snapshot_attempts",
    "candidate_score_attempts",
    "worker_score_attempts",
    "top_k_partition_runs",
    "top_k_retained_candidates",
    "reachable_with_cache_calls",
    "wheelbarrow_arbitration_rebuilds",
    "wheelbarrow_request_bucket_builds",
    "wheelbarrow_bucket_items_scanned",
    "wheelbarrow_candidates_after_top_k",
    "runtime_path_total_core_searches",
)
DASHBOARD_EQUALITY_FIELDS = tuple(
    field
    for field in DETERMINISM_COLUMNS
    if field not in {"schema_version", "dashboard_mode", *DASHBOARD_RENDER_COUNTERS}
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


def apply_dashboard_mode_controlled_audit(
    session_dir: Path,
    manifest: dict[str, Any],
    runs: list[tuple[Path, Validation]],
) -> bool:
    matrix = manifest["matrix"]
    modes = set(matrix.get("dashboard_modes", ["hidden"]))
    comparison_path = session_dir / "dashboard_mode_comparison.json"
    expected_modes = {"hidden", "visible", "active-filter"}
    if matrix.get("workload") != "task-dashboard" or modes != expected_modes:
        if comparison_path.exists():
            comparison_path.unlink()
        return False

    case_metadata = {case["id"]: case for case in manifest.get("cases", [])}
    groups: dict[tuple[Any, ...], dict[str, str]] = {}
    for case_id, case in case_metadata.items():
        contract = (
            case["workload"],
            case["size"],
            case["render"],
            case["seed"],
            case.get("souls"),
            case.get("familiars"),
            case.get("familiar_policy", "baseline"),
            case.get("operation_dialog", "hidden"),
        )
        groups.setdefault(contract, {})[case.get("dashboard_mode", "hidden")] = case_id

    valid_by_case: dict[str, Validation] = {}
    for run_dir, validation in runs:
        if validation.valid and validation.determinism is not None:
            valid_by_case.setdefault(run_dir.parent.name, validation)

    changed = False
    all_failures: list[str] = []
    group_results: list[dict[str, Any]] = []
    for contract, mode_cases in sorted(groups.items()):
        case_ids = set(mode_cases.values())
        failures: list[str] = []
        if set(mode_cases) != expected_modes:
            failures.append("matrix does not contain the three required dashboard modes")
        validations = {
            mode: valid_by_case.get(case_id) for mode, case_id in mode_cases.items()
        }
        if any(validation is None for validation in validations.values()):
            failures.append("one or more dashboard cases has no valid deterministic run")

        post_warmup: dict[str, dict[str, str]] = {}
        if not failures:
            for checkpoint_name in (
                "fixture-pre-update",
                "post-update-1",
                "post-update-8",
                "post-update-32",
                "post-update-128",
                "post-warmup",
                "post-audit-end",
            ):
                rows = {
                    mode: _checkpoint(validation, checkpoint_name)
                    for mode, validation in validations.items()
                    if validation is not None
                }
                if len(rows) != len(expected_modes) or any(row is None for row in rows.values()):
                    failures.append(f"{checkpoint_name} is absent from one or more modes")
                    continue
                hidden = rows["hidden"]
                assert hidden is not None
                for mode in ("visible", "active-filter"):
                    row = rows[mode]
                    assert row is not None
                    differences = [
                        field
                        for field in DASHBOARD_EQUALITY_FIELDS
                        if row[field] != hidden[field]
                    ]
                    if differences:
                        failures.append(
                            f"{checkpoint_name} {mode} differs from hidden in "
                            + ", ".join(differences)
                        )
            post_warmup = {
                mode: _checkpoint(validation, "post-warmup") or {}
                for mode, validation in validations.items()
                if validation is not None
            }

        counters: dict[str, dict[str, int]] = {}
        if not failures:
            counters = {
                mode: {
                    field: int(row[field])
                    for field in (
                        *DASHBOARD_NON_VACUOUS_AI_COUNTERS,
                        *DASHBOARD_RENDER_COUNTERS,
                        "dashboard_state_rebuilds",
                        "dashboard_snapshot_rows_scanned",
                        "dashboard_summary_rows_scanned",
                    )
                }
                for mode, row in post_warmup.items()
            }
            for field in DASHBOARD_NON_VACUOUS_AI_COUNTERS:
                if counters["hidden"][field] <= 0:
                    failures.append(f"task-dashboard fixture did not exercise {field}")
            if any(counters["hidden"][field] != 0 for field in DASHBOARD_RENDER_COUNTERS):
                failures.append("hidden mode performed Task Dashboard render work")
            for mode in ("visible", "active-filter"):
                if counters[mode]["dashboard_render_rebuilds"] <= 0:
                    failures.append(f"{mode} mode never rebuilt the Task Dashboard")
                if counters[mode]["dashboard_render_input_rows"] <= 0:
                    failures.append(f"{mode} mode rendered no dashboard input rows")
                if counters[mode]["dashboard_render_visible_rows"] <= 0:
                    failures.append(f"{mode} mode rendered no visible dashboard rows")
            if (
                counters["visible"]["dashboard_render_rebuilds"]
                != counters["active-filter"]["dashboard_render_rebuilds"]
            ):
                failures.append("visible and active-filter rebuild counts differ")
            if (
                counters["visible"]["dashboard_render_input_rows"]
                != counters["active-filter"]["dashboard_render_input_rows"]
            ):
                failures.append("visible and active-filter input row counts differ")
            if (
                counters["active-filter"]["dashboard_render_visible_rows"]
                >= counters["visible"]["dashboard_render_visible_rows"]
            ):
                failures.append("active-filter did not reduce visible dashboard rows")

        if failures:
            reason = "dashboard mode controlled comparison failed: " + "; ".join(failures)
            changed |= _invalidate_controlled_cases(runs, case_ids, reason)
            all_failures.extend(failures)
        group_results.append(
            {
                "contract": {
                    "workload": contract[0],
                    "size": contract[1],
                    "render": contract[2],
                    "seed": contract[3],
                    "souls": contract[4],
                    "familiars": contract[5],
                    "familiar_policy": contract[6],
                    "operation_dialog": contract[7],
                },
                "status": "fail" if failures else "pass",
                "failures": failures,
                "post_warmup_counters": counters,
            }
        )

    result = {
        "schema_version": 1,
        "status": "fail" if all_failures else "pass",
        "checkpoint": "post-warmup",
        "required_dashboard_modes": ["hidden", "visible", "active-filter"],
        "groups": group_results,
        "failures": all_failures,
    }
    write_json(comparison_path, result)
    return changed
