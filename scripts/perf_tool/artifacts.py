from __future__ import annotations

from .arguments import *

def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_summary(path: Path) -> tuple[dict[str, str] | None, list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
            headers = set(rows[0].keys()) if rows else set()
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse summary.csv: {error}"]
    missing = sorted(EXPECTED_SUMMARY_COLUMNS - headers)
    if missing:
        errors.append("summary.csv missing columns: " + ", ".join(missing))
    if len(rows) != 1:
        errors.append(f"summary.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    return rows[0], errors


def read_scene_roots(path: Path) -> tuple[dict[str, str] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse scene_roots.csv: {error}"]

    errors: list[str] = []
    missing = sorted(set(SCENE_ROOT_COLUMNS) - headers)
    if missing:
        errors.append("scene_roots.csv missing columns: " + ", ".join(missing))
    if len(rows) != 1:
        errors.append(f"scene_roots.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    for column in SCENE_ROOT_COLUMNS:
        try:
            if int(rows[0][column]) < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"scene_roots.csv {column} must be a nonnegative integer")
    return rows[0], errors


def read_frames(path: Path, expected_samples: int | None) -> list[str]:
    errors: list[str] = []
    if not path.is_file():
        return [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
    except (csv.Error, OSError, UnicodeError) as error:
        return [f"cannot parse frames.csv: {error}"]
    if not rows:
        errors.append("frames.csv has no samples")
    if expected_samples is not None and len(rows) != expected_samples:
        errors.append(f"frames.csv has {len(rows)} rows but summary declares {expected_samples}")
    for index, row in enumerate(rows):
        try:
            float(row["frame_time_ms"])
        except (KeyError, TypeError, ValueError):
            errors.append(f"frames.csv row {index} has an invalid frame_time_ms")
            break
    return errors


def read_determinism(
    path: Path, *, warmup_ticks: int, audit_ticks: int
) -> tuple[list[dict[str, str]] | None, list[str]]:
    expected_columns = set(DETERMINISM_COLUMNS)
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse determinism.csv: {error}"]

    errors: list[str] = []
    if headers != expected_columns:
        missing = sorted(expected_columns - headers)
        unexpected = sorted(headers - expected_columns)
        if missing:
            errors.append("determinism.csv missing columns: " + ", ".join(missing))
        if unexpected:
            errors.append("determinism.csv has unexpected columns: " + ", ".join(unexpected))
    expected_checkpoints = [
        ("fixture-pre-update", 0),
        *DETERMINISM_EARLY_CHECKPOINTS,
        ("post-warmup", warmup_ticks),
        ("post-audit-end", warmup_ticks + audit_ticks),
    ]
    observed_checkpoints = [
        (row.get("checkpoint", ""), row.get("update_tick", "")) for row in rows
    ]
    expected_pairs = [(name, str(tick)) for name, tick in expected_checkpoints]
    if observed_checkpoints != expected_pairs:
        errors.append(
            "determinism.csv checkpoints are "
            + ",".join(f"{name}@{tick}" for name, tick in observed_checkpoints)
            + "; expected "
            + ",".join(f"{name}@{tick}" for name, tick in expected_pairs)
        )
    for index, row in enumerate(rows):
        if row.get("schema_version") != DETERMINISM_SCHEMA_VERSION:
            errors.append(
                f"determinism.csv row {index} schema_version is {row.get('schema_version')!r}, "
                f"expected {DETERMINISM_SCHEMA_VERSION}"
            )
        for field in (
            "update_tick",
            "fixed_timestep_ns",
            "virtual_delta_ns",
            "virtual_elapsed_ns",
            "fixed_delta_ns",
            "fixed_elapsed_ns",
            "fixed_overstep_ns",
            "souls",
            "familiars",
            "designations",
        ):
            try:
                if int(row[field]) < 0:
                    raise ValueError
            except (KeyError, TypeError, ValueError):
                errors.append(f"determinism.csv row {index} has invalid {field}")
                break
        checksum = row.get("state_checksum", "")
        if not re.fullmatch(r"[0-9a-f]{16}", checksum):
            errors.append(f"determinism.csv row {index} has invalid state_checksum")
        if row.get("virtual_paused") not in {"0", "1"}:
            errors.append(f"determinism.csv row {index} has invalid virtual_paused")
        for field in ("virtual_relative_speed_bits", "virtual_effective_speed_bits"):
            if not re.fullmatch(r"[0-9a-f]{16}", row.get(field, "")):
                errors.append(f"determinism.csv row {index} has invalid {field}")

    if rows:
        try:
            timestep_ns = int(rows[0]["fixed_timestep_ns"])
        except (KeyError, TypeError, ValueError):
            timestep_ns = 0
        if timestep_ns <= 0:
            errors.append("determinism.csv fixed_timestep_ns must be greater than zero")
        for index, row in enumerate(rows):
            if timestep_ns <= 0:
                break
            try:
                tick = int(row["update_tick"])
            except (KeyError, TypeError, ValueError):
                continue
            is_initial = index == 0
            expected_elapsed = tick * timestep_ns
            if row.get("fixed_timestep_ns") != str(timestep_ns):
                errors.append(f"determinism.csv row {index} changes fixed_timestep_ns")
            if is_initial:
                expected_values = {
                    "virtual_paused": "1",
                    "virtual_delta_ns": "0",
                    "virtual_elapsed_ns": "0",
                    "fixed_delta_ns": "0",
                    "fixed_elapsed_ns": "0",
                    "fixed_overstep_ns": "0",
                    "virtual_relative_speed_bits": ONE_F64_BITS,
                    "virtual_effective_speed_bits": ZERO_F64_BITS,
                }
            else:
                expected_values = {
                    "virtual_paused": "0",
                    "virtual_delta_ns": str(timestep_ns),
                    "virtual_elapsed_ns": str(expected_elapsed),
                    "fixed_delta_ns": str(timestep_ns),
                    "fixed_elapsed_ns": str(expected_elapsed),
                    "fixed_overstep_ns": "0",
                    "virtual_relative_speed_bits": ONE_F64_BITS,
                    "virtual_effective_speed_bits": ONE_F64_BITS,
                }
            for field, expected in expected_values.items():
                if row.get(field) != expected:
                    errors.append(
                        f"determinism.csv row {index} {field} is {row.get(field)!r}, expected {expected!r}"
                    )
    return (rows if not errors else None), errors


def parse_adapter(log_text: str) -> dict[str, str] | None:
    match = ADAPTER_RE.search(log_text)
    return match.groupdict() if match else None


def classify_log_warnings(log_text: str, allow_patterns: Iterable[str]) -> tuple[list[str], list[str]]:
    compiled_allow = [re.compile(pattern) for pattern in allow_patterns]
    pre_capture_problems: list[str] = []
    post_capture_warnings: list[str] = []
    capture_completed = False
    for line in log_text.splitlines():
        if "PERF_CAPTURE: wrote" in line or "PERF_DETERMINISM_AUDIT: wrote" in line:
            capture_completed = True
            continue
        if not LOG_LEVEL_RE.search(line):
            continue
        if any(pattern.search(line) for pattern in compiled_allow):
            continue
        if capture_completed:
            post_capture_warnings.append(line)
        else:
            pre_capture_problems.append(line)
    return pre_capture_problems, post_capture_warnings


def validate_run(
    run_dir: Path,
    *,
    returncode: int,
    expected_case: Case,
    expected_adapter: str | None,
    expected_backend: str | None,
    allow_log_patterns: Iterable[str],
    capture_kind: str = "frame-time",
    expected_warmup_secs: float | None = None,
    expected_measure_secs: float | None = None,
    expected_fixed_hz: int | None = None,
    expected_warmup_ticks: int | None = None,
    expected_audit_ticks: int | None = None,
) -> Validation:
    reasons: list[str] = []
    data_dir = run_dir / "data"
    summary = None
    determinism = None
    scene_roots = None
    if capture_kind == "frame-time":
        summary, summary_errors = read_summary(data_dir / "summary.csv")
        reasons.extend(summary_errors)
        scene_roots, scene_root_errors = read_scene_roots(data_dir / "scene_roots.csv")
        reasons.extend(scene_root_errors)
    elif capture_kind == "fixed-step-determinism":
        if expected_fixed_hz is None or expected_warmup_ticks is None or expected_audit_ticks is None:
            reasons.append("fixed-step audit validation is missing tick configuration")
        else:
            determinism, determinism_errors = read_determinism(
                data_dir / "determinism.csv",
                warmup_ticks=expected_warmup_ticks,
                audit_ticks=expected_audit_ticks,
            )
            reasons.extend(determinism_errors)
    else:
        reasons.append(f"unsupported capture kind {capture_kind!r}")
    if capture_kind == "fixed-step-determinism" and (
        (data_dir / "summary.csv").exists()
        or (data_dir / "frames.csv").exists()
        or (data_dir / "scene_roots.csv").exists()
    ):
        reasons.append("fixed-step audit must not write frame-time artifacts")
    if capture_kind == "frame-time" and (data_dir / "determinism.csv").exists():
        reasons.append("frame-time capture must not write determinism.csv")
    if returncode != 0:
        reasons.append(f"process exited with status {returncode}")

    if summary is not None:
        if summary.get("schema_version") != SUMMARY_SCHEMA_VERSION:
            reasons.append(
                f"summary schema is {summary.get('schema_version')!r}, expected {SUMMARY_SCHEMA_VERSION}"
            )
        expected_values = {
            "seed": str(expected_case.seed),
            "workload": expected_case.workload,
            "size": expected_case.size,
            "render": expected_case.render,
        }
        for key, expected in expected_values.items():
            if summary.get(key) != expected:
                reasons.append(f"summary {key} is {summary.get(key)!r}, expected {expected!r}")
        try:
            samples = int(summary["samples"])
        except (KeyError, ValueError):
            samples = None
            reasons.append("summary samples is invalid")
        if samples is not None and samples <= 0:
            reasons.append("summary samples must be greater than zero")
        reasons.extend(read_frames(data_dir / "frames.csv", samples))

    if summary is not None and scene_roots is not None:
        try:
            expected_souls = int(summary["initial_souls"])
            expected_familiars = int(summary["initial_familiars"])
        except (KeyError, ValueError):
            reasons.append("summary initial population is invalid for scene root validation")
        else:
            expected_counts = {
                "soul_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                "soul_mask_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                "soul_shadow_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                "familiar_proxy_3d": 0 if expected_case.render == "cpu" else expected_familiars,
            }
            for column, expected in expected_counts.items():
                if scene_roots.get(column) != str(expected):
                    reasons.append(
                        f"scene_roots.csv {column} is {scene_roots.get(column)!r}, "
                        f"expected {expected!r} for {expected_case.render}"
                    )

    log_path = run_dir / "run.log"
    if not log_path.is_file():
        log_text = ""
        reasons.append("missing run.log")
    else:
        log_text = log_path.read_text(encoding="utf-8", errors="replace")
        completion_marker = (
            "PERF_CAPTURE: wrote"
            if capture_kind == "frame-time"
            else "PERF_DETERMINISM_AUDIT: wrote"
        )
        if completion_marker not in log_text:
            reasons.append(f"{completion_marker} completion marker is absent")
        expected_clock_mode = "fixed" if capture_kind == "fixed-step-determinism" else "realtime"
        if f"clock={expected_clock_mode}" not in log_text:
            reasons.append(
                f"PERF_SCENARIO clock marker is absent or does not match {expected_clock_mode!r}"
            )
        for marker in (
            f"seed={expected_case.seed}",
            f"workload={expected_case.workload}",
            f"size={expected_case.size}",
            f"render={expected_case.render}",
        ):
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
        if capture_kind == "fixed-step-determinism" and expected_fixed_hz is not None:
            marker = f"fixed_hz={expected_fixed_hz}"
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
    warnings, teardown_warnings = classify_log_warnings(log_text, allow_log_patterns)
    reasons.extend(f"unexpected log warning/error: {line}" for line in warnings)

    adapter = parse_adapter(log_text)
    if expected_adapter:
        if adapter is None:
            reasons.append("actual WGPU adapter was not found in run.log")
        elif expected_adapter.casefold() not in adapter["name"].casefold():
            reasons.append(
                f"actual adapter {adapter['name']!r} does not match requested {expected_adapter!r}"
            )
    if expected_backend and expected_backend != "auto":
        if adapter is None:
            reasons.append("actual WGPU backend was not found in run.log")
        elif adapter["backend"].casefold() != expected_backend.casefold():
            reasons.append(
                f"actual backend {adapter['backend']!r} does not match requested {expected_backend!r}"
            )

    return Validation(
        valid=not reasons,
        reasons=reasons,
        summary=summary,
        adapter=adapter,
        warning_lines=warnings,
        teardown_warning_lines=teardown_warnings,
        determinism=determinism,
        scene_roots=scene_roots,
    )
