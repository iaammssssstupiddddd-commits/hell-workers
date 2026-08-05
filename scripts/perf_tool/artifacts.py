from __future__ import annotations

from .arguments import *

def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def determinism_records_checksum(payloads: Iterable[bytes]) -> str:
    """Match Rust's checksum_from_audit_records payload hash exactly."""
    ordered = sorted(payloads)
    checksum = 0xCBF29CE484222325
    for byte in len(ordered).to_bytes(8, byteorder="little"):
        checksum ^= byte
        checksum = (checksum * 0x00000100000001B3) & 0xFFFFFFFFFFFFFFFF
    for payload in ordered:
        for byte in payload:
            checksum ^= byte
            checksum = (checksum * 0x00000100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{checksum:016x}"


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_summary(path: Path) -> tuple[dict[str, str] | None, list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
            headers = set(fieldnames)
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse summary.csv: {error}"]
    missing = sorted(EXPECTED_SUMMARY_COLUMNS - headers)
    if missing:
        errors.append("summary.csv missing columns: " + ", ".join(missing))
    unexpected = sorted(headers - EXPECTED_SUMMARY_COLUMNS)
    if unexpected:
        errors.append("summary.csv has unexpected columns: " + ", ".join(unexpected))
    if len(fieldnames) != len(headers):
        errors.append("summary.csv has duplicate columns")
    if len(rows) != 1:
        errors.append(f"summary.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    row = rows[0]
    if None in row:
        errors.append("summary.csv row has more values than columns")
    for column in sorted(SUMMARY_INTEGER_COLUMNS):
        try:
            if int(row[column]) < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"summary.csv {column} must be a nonnegative integer")
    for column in sorted(SUMMARY_FLOAT_COLUMNS):
        try:
            value = float(row[column])
            if not math.isfinite(value) or value < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"summary.csv {column} must be a finite nonnegative number")
    for column in sorted(SUMMARY_CHECKSUM_COLUMNS):
        if not re.fullmatch(r"[0-9a-f]{16}", row.get(column, "")):
            errors.append(f"summary.csv {column} must be a 16-digit lowercase hex checksum")
    return row, errors


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


def read_render_inventory(
    path: Path,
) -> tuple[dict[str, str] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=RENDER_INVENTORY_COLUMNS,
        artifact_name="render_inventory.csv",
    )
    if rows is None:
        return None, errors
    if len(rows) != 1:
        return None, [
            *errors,
            f"render_inventory.csv must contain exactly one data row; got {len(rows)}",
        ]
    row = rows[0]
    if row.get("schema_version") != RENDER_INVENTORY_SCHEMA_VERSION:
        errors.append(
            "render_inventory.csv schema_version is "
            f"{row.get('schema_version')!r}, expected {RENDER_INVENTORY_SCHEMA_VERSION}"
        )
    parsed: dict[str, int] = {}
    for column in RENDER_INVENTORY_COLUMNS[1:]:
        value = row.get(column, "")
        try:
            parsed_value = int(value)
            if parsed_value < 0 or str(parsed_value) != value:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                f"render_inventory.csv {column} must be a canonical nonnegative integer"
            )
            continue
        parsed[column] = parsed_value
    if len(parsed) == len(RENDER_INVENTORY_COLUMNS) - 1:
        if parsed["scene_target_count"] != 1:
            errors.append("render_inventory.csv must observe exactly one Scene target")
        if parsed["camera_3d_rtt_count"] != (
            parsed["scene_target_count"] + parsed["mask_target_count"]
        ):
            errors.append(
                "render_inventory.csv camera_3d_rtt_count differs from Scene + mask targets"
            )
        if parsed["layer_2d_pass_count"] > parsed["camera_2d_count"]:
            errors.append(
                "render_inventory.csv layer_2d_pass_count exceeds camera_2d_count"
            )
    return (row if not errors else None), errors


def read_exact_csv_rows(
    path: Path,
    *,
    columns: tuple[str, ...],
    artifact_name: str,
) -> tuple[list[dict[str, str]] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse {artifact_name}: {error}"]
    errors: list[str] = []
    if fieldnames != list(columns):
        errors.append(f"{artifact_name} columns differ from schema: " + ", ".join(fieldnames))
    if any(None in row for row in rows):
        errors.append(f"{artifact_name} has a row with more values than columns")
    return (rows if not errors else None), errors


def compare_exact_rows(
    artifact_name: str,
    observed: list[dict[str, str]] | None,
    expected: list[dict[str, str]],
) -> list[str]:
    if observed is None:
        return []
    errors: list[str] = []
    if len(observed) != len(expected):
        errors.append(
            f"{artifact_name} must contain exactly {len(expected)} data rows; got {len(observed)}"
        )
    for index, (observed_row, expected_row) in enumerate(zip(observed, expected)):
        for column, expected_value in expected_row.items():
            observed_value = observed_row.get(column)
            if observed_value != expected_value:
                errors.append(
                    f"{artifact_name} row {index} {column} is {observed_value!r}, "
                    f"expected {expected_value!r}"
                )
                if len(errors) >= 10:
                    return errors
    return errors


def expected_indoor_light_fixture_row(
    contract: dict[str, Any],
    case: Case,
    *,
    contract_id: str,
    stage_id: str,
    lane: str,
) -> dict[str, str]:
    layout = build_fixture_layout(contract, case.size)
    size_contract = contract["fixture"]["sizes"][case.size]
    counts = layout["counts"]
    hashes = contract_fingerprints(contract)
    lamp_demand = contract["fixture"]["outdoor_lamp_demand"]
    return {
        "schema_version": INDOOR_LIGHT_FIXTURE_SCHEMA_VERSION,
        "contract_id": contract_id,
        "stage_id": stage_id,
        "lane": lane,
        "checkpoint": "fixture-pre-update",
        "case_id": f"indoor-light-{case.size}-{case.render}-seed-{case.seed}",
        "fixture_id": layout["fixture_id"],
        "size": case.size,
        "layout_checksum": layout["layout_checksum"],
        "measurement_contract_sha256": hashes["measurement_contract_sha256"],
        "fixture_contract_sha256": hashes["fixture_contract_sha256"],
        "completed_floors": str(counts["completed_floors"]),
        "completed_walls": str(counts["completed_walls"]),
        "doors": str(counts["doors"]),
        "supplied_lamp_candidates": str(counts["supplied_lamp_candidates"]),
        "unsupplied_lamp_candidates": str(counts["unsupplied_lamp_candidates"]),
        "rooms": str(counts["rooms"]),
        "room_tiles": str(counts["completed_floors"]),
        "room_boundary_lookup_cells": str(counts["room_boundary_lookup_cells"]),
        "souls": str(counts["souls"]),
        "familiars": str(counts["familiars"]),
        "yards": str(counts["yards"]),
        "operational_soul_spas": str(counts["operational_soul_spas"]),
        "generator_souls": str(counts["generator_souls"]),
        "main_generation": f'{layout["energy"]["generation"]:.6f}',
        "main_demand": f'{size_contract["runtime_f32_active_lamp_demand"]:.6f}',
        "main_headroom": f'{size_contract["runtime_f32_headroom"]:.6f}',
        "main_supplied_count": str(counts["supplied_lamp_candidates"]),
        "main_shed_count": "0",
        "control_generation": "0.000000",
        "control_demand": f"{lamp_demand:.6f}",
        "control_supplied_count": "0",
        "control_shed_count": "1",
    }


def read_indoor_light_sidecars(
    data_dir: Path,
    *,
    expected_case: Case,
    contract_id: str,
    stage_id: str,
    lane: str,
) -> tuple[
    dict[str, str] | None,
    list[dict[str, str]] | None,
    list[dict[str, str]] | None,
    list[str],
]:
    errors: list[str] = []
    try:
        contract = load_rtt_light_contract(contract_id)
        validate_stage_lane(contract, stage_id, lane)
    except ValueError as error:
        return None, None, None, [f"invalid indoor-light selection: {error}"]

    fixture_rows, parse_errors = read_exact_csv_rows(
        data_dir / "indoor_light_fixture.csv",
        columns=INDOOR_LIGHT_FIXTURE_COLUMNS,
        artifact_name="indoor_light_fixture.csv",
    )
    errors.extend(parse_errors)
    expected_fixture = expected_indoor_light_fixture_row(
        contract,
        expected_case,
        contract_id=contract_id,
        stage_id=stage_id,
        lane=lane,
    )
    errors.extend(
        compare_exact_rows(
            "indoor_light_fixture.csv", fixture_rows, [expected_fixture]
        )
    )

    layout_rows, parse_errors = read_exact_csv_rows(
        data_dir / "indoor_light_layout.csv",
        columns=INDOOR_LIGHT_LAYOUT_COLUMNS,
        artifact_name="indoor_light_layout.csv",
    )
    errors.extend(parse_errors)
    errors.extend(
        compare_exact_rows(
            "indoor_light_layout.csv",
            layout_rows,
            build_fixture_ledger(contract, expected_case.size),
        )
    )

    presentation_rows, parse_errors = read_exact_csv_rows(
        data_dir / "indoor_light_presentation.csv",
        columns=INDOOR_LIGHT_PRESENTATION_COLUMNS,
        artifact_name="indoor_light_presentation.csv",
    )
    errors.extend(parse_errors)
    errors.extend(
        compare_exact_rows(
            "indoor_light_presentation.csv",
            presentation_rows,
            build_fixture_presentation_rows(contract, expected_case.size),
        )
    )
    fixture = fixture_rows[0] if fixture_rows is not None and len(fixture_rows) == 1 else None
    return fixture, layout_rows, presentation_rows, errors




def read_window(
    path: Path,
    *,
    expect_headless: bool,
    expected_width: int | None,
    expected_height: int | None,
    expected_scale_factor: float | None,
    expected_rtt_quality: str | None,
    expected_window_backend: str,
    expected_backend: str | None,
    expected_present_mode: str,
) -> tuple[dict[str, str] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse window.csv: {error}"]

    errors: list[str] = []
    if fieldnames != list(WINDOW_COLUMNS):
        errors.append("window.csv columns differ from schema: " + ", ".join(fieldnames))
    if len(rows) != 1:
        errors.append(f"window.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    row = rows[0]
    if None in row:
        errors.append("window.csv row has more values than columns")
    if row.get("schema_version") != WINDOW_SCHEMA_VERSION:
        errors.append(
            f"window.csv schema_version is {row.get('schema_version')!r}, "
            f"expected {WINDOW_SCHEMA_VERSION!r}"
        )

    paired_fields = (
        ("window_present", "end_window_present"),
        ("logical_width", "end_logical_width"),
        ("logical_height", "end_logical_height"),
        ("physical_width", "end_physical_width"),
        ("physical_height", "end_physical_height"),
        ("scale_factor", "end_scale_factor"),
        ("rtt_quality", "end_rtt_quality"),
        ("scene_target_width", "end_scene_target_width"),
        ("scene_target_height", "end_scene_target_height"),
        ("mask_target_width", "end_mask_target_width"),
        ("mask_target_height", "end_mask_target_height"),
        ("target_scale_factor", "end_target_scale_factor"),
        ("resolved_window_backend", "end_resolved_window_backend"),
        ("adapter_name", "end_adapter_name"),
        ("adapter_backend", "end_adapter_backend"),
        ("requested_present_mode", "end_requested_present_mode"),
        ("effective_present_mode", "end_effective_present_mode"),
    )
    for initial_field, final_field in paired_fields:
        if row.get(initial_field) != row.get(final_field):
            errors.append(
                f"window.csv changed {initial_field} during capture: "
                f"{row.get(initial_field)!r} -> {row.get(final_field)!r}"
            )

    present_text = row.get("window_present", "")
    if present_text not in {"true", "false"}:
        errors.append("window.csv window_present must be true or false")
    present = present_text == "true"
    expected_present = not expect_headless
    if present != expected_present:
        errors.append(
            f"window.csv window_present is {present_text!r}, expected {str(expected_present).lower()!r}"
        )

    render_environment_fields = (
        "resolved_window_backend",
        "adapter_name",
        "adapter_backend",
        "requested_present_mode",
        "effective_present_mode",
    )
    if present:
        for field in render_environment_fields:
            if not row.get(field):
                errors.append(f"window.csv {field} is required with a primary window")
        resolved_window_backend = row.get("resolved_window_backend", "")
        if resolved_window_backend not in {"x11", "wayland"}:
            errors.append("window.csv resolved_window_backend must be x11 or wayland")
        elif expected_window_backend not in {"auto", resolved_window_backend}:
            errors.append(
                "window.csv resolved_window_backend is "
                f"{resolved_window_backend!r}, expected {expected_window_backend!r}"
            )
        adapter_backend = row.get("adapter_backend", "")
        if expected_backend not in {None, "auto"} and adapter_backend != expected_backend:
            errors.append(
                f"window.csv adapter_backend is {adapter_backend!r}, "
                f"expected {expected_backend!r}"
            )
        requested_present = row.get("requested_present_mode", "")
        expected_requested_present = {
            "novsync": "auto_no_vsync",
            "auto_vsync": "auto_vsync",
            "fifo": "fifo",
            "mailbox": "mailbox",
            "immediate": "immediate",
        }.get(expected_present_mode)
        if requested_present != expected_requested_present:
            errors.append(
                f"window.csv requested_present_mode is {requested_present!r}, "
                f"expected {expected_requested_present!r}"
            )
        effective_present = row.get("effective_present_mode", "")
        if effective_present not in {"fifo", "fifo_relaxed", "mailbox", "immediate"}:
            errors.append(
                "window.csv effective_present_mode must be a concrete present mode"
            )
        allowed_effective = {
            "auto_no_vsync": {"immediate", "mailbox", "fifo"},
            "auto_vsync": {"fifo_relaxed", "fifo"},
            "fifo": {"fifo"},
            "mailbox": {"mailbox", "immediate", "fifo"},
            "immediate": {"immediate", "fifo"},
        }.get(requested_present, set())
        if effective_present not in allowed_effective:
            errors.append(
                "window.csv effective_present_mode is not a valid Bevy 0.19 fallback"
            )
    else:
        for field in render_environment_fields:
            if row.get(field, "") != "":
                errors.append(f"window.csv {field} must be empty without a primary window")

    quality = row.get("rtt_quality", "")
    if quality not in {"high", "medium", "low"}:
        errors.append("window.csv rtt_quality must be high, medium, or low")
    effective_quality = expected_rtt_quality or "high"
    if quality != effective_quality:
        errors.append(
            f"window.csv rtt_quality is {quality!r}, expected {effective_quality!r}"
        )
    quality_scale = {"high": 1.0, "medium": 0.75, "low": 0.5}.get(quality)

    parsed_window: dict[str, float | int] = {}
    window_float_fields = ("logical_width", "logical_height", "scale_factor")
    window_integer_fields = ("physical_width", "physical_height")
    if present:
        for field in window_float_fields:
            try:
                value = float(row[field])
                if not math.isfinite(value) or value <= 0:
                    raise ValueError
                parsed_window[field] = value
            except (KeyError, TypeError, ValueError):
                errors.append(f"window.csv {field} must be a finite positive number")
        for field in window_integer_fields:
            try:
                value = int(row[field])
                if value <= 0:
                    raise ValueError
                parsed_window[field] = value
            except (KeyError, TypeError, ValueError):
                errors.append(f"window.csv {field} must be a positive integer")
    else:
        for field in (*window_float_fields, *window_integer_fields):
            if row.get(field, "") != "":
                errors.append(f"window.csv {field} must be empty without a primary window")

    target_values: dict[str, float | int] = {}
    for field in (
        "scene_target_width",
        "scene_target_height",
        "mask_target_width",
        "mask_target_height",
    ):
        try:
            value = int(row[field])
            if value <= 0:
                raise ValueError
            target_values[field] = value
        except (KeyError, TypeError, ValueError):
            errors.append(f"window.csv {field} must be a positive integer")
    try:
        target_factor = float(row["target_scale_factor"])
        if not math.isfinite(target_factor) or target_factor <= 0:
            raise ValueError
        target_values["target_scale_factor"] = target_factor
    except (KeyError, TypeError, ValueError):
        errors.append("window.csv target_scale_factor must be a finite positive number")

    if target_values.get("scene_target_width") != target_values.get("mask_target_width"):
        errors.append("window.csv scene and mask target widths differ")
    if target_values.get("scene_target_height") != target_values.get("mask_target_height"):
        errors.append("window.csv scene and mask target heights differ")

    if present:
        if {
            "logical_width",
            "logical_height",
            "physical_width",
            "physical_height",
            "scale_factor",
        } <= parsed_window.keys():
            scale_factor = float(parsed_window["scale_factor"])
            expected_logical_width = float(parsed_window["physical_width"]) / scale_factor
            expected_logical_height = float(parsed_window["physical_height"]) / scale_factor
            if not math.isclose(
                float(parsed_window["logical_width"]),
                expected_logical_width,
                rel_tol=0.0,
                abs_tol=1e-3,
            ):
                errors.append(
                    "window.csv logical_width does not match physical_width / scale_factor"
                )
            if not math.isclose(
                float(parsed_window["logical_height"]),
                expected_logical_height,
                rel_tol=0.0,
                abs_tol=1e-3,
            ):
                errors.append(
                    "window.csv logical_height does not match physical_height / scale_factor"
                )
        if expected_width is not None and parsed_window.get("physical_width") != expected_width:
            errors.append(
                f"window.csv physical_width is {parsed_window.get('physical_width')!r}, "
                f"expected {expected_width!r}"
            )
        if expected_height is not None and parsed_window.get("physical_height") != expected_height:
            errors.append(
                f"window.csv physical_height is {parsed_window.get('physical_height')!r}, "
                f"expected {expected_height!r}"
            )
        if expected_scale_factor is not None and not math.isclose(
            float(parsed_window.get("scale_factor", math.nan)),
            expected_scale_factor,
            rel_tol=0.0,
            abs_tol=1e-5,
        ):
            errors.append(
                f"window.csv scale_factor is {parsed_window.get('scale_factor')!r}, "
                f"expected {expected_scale_factor!r}"
            )
        if quality_scale is not None and {
            "physical_width",
            "physical_height",
            "scale_factor",
        } <= parsed_window.keys():
            expected_target_width = max(
                1,
                math.floor(float(parsed_window["physical_width"]) * quality_scale + 0.5),
            )
            expected_target_height = max(
                1,
                math.floor(float(parsed_window["physical_height"]) * quality_scale + 0.5),
            )
            expected_target_factor = float(parsed_window["scale_factor"]) * quality_scale
            if target_values.get("scene_target_width") != expected_target_width:
                errors.append(
                    f"window.csv scene_target_width is {target_values.get('scene_target_width')!r}, "
                    f"expected {expected_target_width!r}"
                )
            if target_values.get("scene_target_height") != expected_target_height:
                errors.append(
                    f"window.csv scene_target_height is {target_values.get('scene_target_height')!r}, "
                    f"expected {expected_target_height!r}"
                )
            if not math.isclose(
                float(target_values.get("target_scale_factor", math.nan)),
                expected_target_factor,
                rel_tol=0.0,
                abs_tol=1e-5,
            ):
                errors.append(
                    "window.csv target_scale_factor does not match window scale and RtT quality"
                )
    elif quality_scale is not None:
        expected_target_width = max(1, math.floor(1280 * quality_scale + 0.5))
        expected_target_height = max(1, math.floor(720 * quality_scale + 0.5))
        if target_values.get("scene_target_width") != expected_target_width:
            errors.append(
                f"window.csv headless scene_target_width is {target_values.get('scene_target_width')!r}, "
                f"expected {expected_target_width!r}"
            )
        if target_values.get("scene_target_height") != expected_target_height:
            errors.append(
                f"window.csv headless scene_target_height is {target_values.get('scene_target_height')!r}, "
                f"expected {expected_target_height!r}"
            )
        if not math.isclose(
            float(target_values.get("target_scale_factor", math.nan)),
            quality_scale,
            rel_tol=0.0,
            abs_tol=1e-5,
        ):
            errors.append("window.csv headless target_scale_factor does not match RtT quality")

    return (row if not errors else None), errors


def read_frames(
    path: Path, expected_samples: int | None
) -> tuple[list[float] | None, list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse frames.csv: {error}"]
    if fieldnames != ["frame_index", "frame_time_ms"]:
        errors.append(
            "frames.csv columns differ from schema: " + ", ".join(fieldnames)
        )
    if not rows:
        errors.append("frames.csv has no samples")
    if expected_samples is not None and len(rows) != expected_samples:
        errors.append(f"frames.csv has {len(rows)} rows but summary declares {expected_samples}")
    samples: list[float] = []
    for index, row in enumerate(rows):
        try:
            frame_index = int(row["frame_index"])
            frame_time_ms = float(row["frame_time_ms"])
            if frame_index != index or not math.isfinite(frame_time_ms) or frame_time_ms < 0:
                raise ValueError
            samples.append(frame_time_ms)
        except (KeyError, TypeError, ValueError):
            errors.append(
                f"frames.csv row {index} must have sequential frame_index and finite nonnegative frame_time_ms"
            )
            break
    return (samples if not errors else None), errors


def frame_summary(samples: list[float]) -> dict[str, float]:
    ordered = sorted(samples)

    def percentile(ratio: float) -> float:
        index = math.floor((len(ordered) - 1) * ratio + 0.5)
        return ordered[index]

    return {
        "p50_ms": percentile(0.50),
        "p95_ms": percentile(0.95),
        "p99_ms": percentile(0.99),
        "max_ms": ordered[-1],
    }


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
            "delegation_cycles",
            "incoming_snapshot_builds",
            "delegation_familiars_processed",
            "candidate_membership_checks",
            "policy_disabled_rejections",
            "candidate_snapshot_attempts",
            "candidate_score_attempts",
            "worker_score_attempts",
            "top_k_partition_runs",
            "top_k_retained_candidates",
            "top_k_fallback_candidates",
            "source_selector_calls",
            "source_selector_cache_build_scanned_items",
            "source_selector_candidate_scanned_items",
            "source_selector_scanned_items",
            "reachable_with_cache_calls",
            "wheelbarrow_arbitration_rebuilds",
            "wheelbarrow_request_bucket_builds",
            "wheelbarrow_bucket_items_scanned",
            "wheelbarrow_candidates_after_top_k",
            "runtime_path_actor_new_core_searches",
            "runtime_path_actor_new_deferred",
            "runtime_path_actor_reuse_core_searches",
            "runtime_path_actor_reuse_deferred",
            "runtime_path_actor_rest_fallback_core_searches",
            "runtime_path_actor_rest_fallback_deferred",
            "runtime_path_escape_core_searches",
            "runtime_path_escape_deferred",
            "runtime_path_task_execution_core_searches",
            "runtime_path_task_execution_deferred",
            "runtime_path_bucket_transport_core_searches",
            "runtime_path_bucket_transport_deferred",
            "runtime_path_total_core_searches",
            "runtime_path_expanded_nodes",
            "runtime_path_max_expanded_nodes_per_search",
            "runtime_path_active_task_max_defer_frames",
            "runtime_path_idle_or_rest_max_defer_frames",
            "runtime_path_deferred_actor_retries",
            "dashboard_state_rebuilds",
            "dashboard_snapshot_rows_scanned",
            "dashboard_summary_rows_scanned",
            "dashboard_snapshot_changes",
            "dashboard_summary_changes",
            "dashboard_render_rebuilds",
            "dashboard_render_input_rows",
            "dashboard_render_visible_rows",
            "dashboard_render_group_headers",
            "dashboard_despawn_roots_requested",
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
        structural_checksum = row.get("structural_checksum", "")
        if not re.fullmatch(r"[0-9a-f]{16}", structural_checksum):
            errors.append(f"determinism.csv row {index} has invalid structural_checksum")
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


def read_determinism_records(
    path: Path,
    determinism: list[dict[str, str]],
    *,
    expected_workload: str,
    expected_indoor_actor_counts: dict[str, int] | None = None,
) -> tuple[list[dict[str, str]] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse determinism_records.csv: {error}"]

    errors: list[str] = []
    if fieldnames != list(DETERMINISM_RECORD_COLUMNS):
        errors.append(
            "determinism_records.csv columns differ from schema: "
            + ", ".join(fieldnames)
        )
    expected_checkpoints = [
        (row["checkpoint"], row["update_tick"]) for row in determinism
    ]
    checkpoint_order = {
        checkpoint: index for index, checkpoint in enumerate(expected_checkpoints)
    }
    seen: set[tuple[str, str, str]] = set()
    observed_sort_keys: list[tuple[int, str, int]] = []
    expected_actor_counts = {
        "soul": None,
        "familiar": None,
        "designation": None,
        **(expected_indoor_actor_counts or {}),
    }
    if expected_workload == "indoor-light" and expected_indoor_actor_counts is None:
        errors.append("indoor-light determinism validation is missing size-specific actor counts")
    if expected_workload != "indoor-light" and expected_indoor_actor_counts is not None:
        errors.append("non-indoor determinism validation received indoor actor counts")
    population_counts: dict[str, dict[str, int]] = {
        checkpoint: {actor_kind: 0 for actor_kind in expected_actor_counts}
        for checkpoint, _tick in expected_checkpoints
    }
    checkpoint_payloads: dict[str, list[bytes]] = {
        checkpoint: [] for checkpoint, _tick in expected_checkpoints
    }
    invalid_payload_checkpoints: set[str] = set()
    allowed_kinds = {*expected_actor_counts, "fixture"}
    for index, row in enumerate(rows):
        checkpoint_pair = (row.get("checkpoint", ""), row.get("update_tick", ""))
        if row.get("schema_version") != DETERMINISM_SCHEMA_VERSION:
            errors.append(
                f"determinism_records.csv row {index} has invalid schema_version"
            )
        if checkpoint_pair not in checkpoint_order:
            errors.append(
                f"determinism_records.csv row {index} has unknown checkpoint/tick"
            )
            continue
        actor_kind = row.get("actor_kind", "")
        if actor_kind not in allowed_kinds:
            errors.append(
                f"determinism_records.csv row {index} has invalid actor_kind "
                f"{actor_kind!r} for workload {expected_workload!r}"
            )
        try:
            actor_key = int(row["actor_key"])
            if actor_key < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"determinism_records.csv row {index} has invalid actor_key")
            continue
        record_hex = row.get("record_hex", "")
        if not record_hex or len(record_hex) % 2 != 0 or not re.fullmatch(
            r"[0-9a-f]+", record_hex
        ):
            errors.append(f"determinism_records.csv row {index} has invalid record_hex")
            invalid_payload_checkpoints.add(checkpoint_pair[0])
        else:
            checkpoint_payloads[checkpoint_pair[0]].append(bytes.fromhex(record_hex))
        identity = (checkpoint_pair[0], actor_kind, str(actor_key))
        if identity in seen:
            errors.append(f"determinism_records.csv row {index} duplicates an actor record")
        seen.add(identity)
        observed_sort_keys.append(
            (checkpoint_order[checkpoint_pair], actor_kind, actor_key)
        )
        if actor_kind in population_counts[checkpoint_pair[0]]:
            population_counts[checkpoint_pair[0]][actor_kind] += 1

    if observed_sort_keys != sorted(observed_sort_keys):
        errors.append("determinism_records.csv rows are not in stable checkpoint/actor order")
    for checkpoint_row in determinism:
        checkpoint = checkpoint_row["checkpoint"]
        for actor_kind, field in (
            ("soul", "souls"),
            ("familiar", "familiars"),
            ("designation", "designations"),
        ):
            observed = population_counts[checkpoint][actor_kind]
            expected = int(checkpoint_row[field])
            if observed != expected:
                errors.append(
                    f"determinism_records.csv {checkpoint} has {observed} {actor_kind} "
                    f"records; expected {expected}"
                )
        for actor_kind, expected in (expected_indoor_actor_counts or {}).items():
            observed = population_counts[checkpoint][actor_kind]
            if observed != expected:
                errors.append(
                    f"determinism_records.csv {checkpoint} has {observed} {actor_kind} "
                    f"records; expected {expected}"
                )
        if checkpoint not in invalid_payload_checkpoints:
            calculated_checksum = determinism_records_checksum(
                checkpoint_payloads[checkpoint]
            )
            if checkpoint_row.get("state_checksum") != calculated_checksum:
                errors.append(
                    f"determinism_records.csv {checkpoint} computes state_checksum "
                    f"{calculated_checksum}, but determinism.csv declares "
                    f"{checkpoint_row.get('state_checksum')!r}"
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
        if (
            "PERF_CAPTURE: wrote" in line
            or "PERF_DETERMINISM_AUDIT: wrote" in line
            or "PERF_BEHAVIOR: wrote" in line
        ):
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


def read_behavior_timeline(
    data_dir: Path,
    *,
    expected_case: Case,
    contract_id: str,
    stage_id: str,
) -> tuple[list[dict[str, Any]] | None, dict[str, Any] | None, list[str]]:
    errors: list[str] = []
    behavior_case = expected_case.behavior_case
    if behavior_case is None:
        return None, None, ["behavior validation requires a selected behavior case"]
    contract = load_rtt_light_contract(contract_id)
    if stage_id != "current":
        return None, None, ["behavior timeline validator currently requires stage current"]
    timeline_contract = contract["behavior_fixture"]["timeline"]
    path = data_dir / "timeline.json"
    def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result
    try:
        payload = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        return None, None, [f"cannot parse timeline.json: {error}"]
    if not isinstance(payload, dict) or list(payload) != [
        "schema_version",
        "complete",
        "rows",
    ]:
        return None, None, ["timeline.json top-level schema or key order differs"]
    if payload.get("schema_version") != timeline_contract["schema_version"]:
        errors.append("timeline.json schema_version differs from the behavior contract")
    if payload.get("complete") is not True:
        errors.append("timeline.json is not marked complete")
    rows = payload.get("rows")
    if not isinstance(rows, list):
        return None, None, [*errors, "timeline.json rows must be a list"]
    case_contract = contract["behavior_fixture"].get(
        behavior_case.replace("-", "_")
    )
    if not isinstance(case_contract, dict):
        return None, None, [*errors, f"unknown behavior timeline case {behavior_case}"]
    expected_steps = case_contract["steps"]
    if len(rows) != len(expected_steps):
        errors.append(
            f"timeline.json must contain exactly {len(expected_steps)} rows; got {len(rows)}"
        )
    columns = timeline_contract["columns"]
    fixture_checksum = build_fixture_layout(contract, expected_case.size)["layout_checksum"]
    integer_fields = {
        "step_index",
        "script_update",
        "simulation_tick",
        "world_epoch",
    }
    bool_fields = {"attempted", "applied"}
    nullable_fields = {
        "registry_step_id",
        "wake_count",
        "field_input_revision",
        "field_output_revision",
        "field_read_count",
        "old_epoch_field_read_count",
        "field_is_dark",
        "field_checksum",
        "gpu_upload_epoch",
        "gpu_checksum",
    }
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or list(row) != columns:
            errors.append(f"timeline.json row {index} columns or key order differ")
            continue
        for field in integer_fields:
            value = row.get(field)
            if (
                not isinstance(value, int)
                or isinstance(value, bool)
                or value < 0
                or value > (1 << 64) - 1
            ):
                errors.append(f"timeline.json row {index} {field} is not a nonnegative integer")
        for field in bool_fields:
            if not isinstance(row.get(field), bool):
                errors.append(f"timeline.json row {index} {field} is not boolean")
        if row.get("case_id") != behavior_case:
            errors.append(f"timeline.json row {index} has the wrong case_id")
        if row.get("step_index") != index:
            errors.append(f"timeline.json row {index} has the wrong step_index")
        if row.get("fixture_checksum") != fixture_checksum:
            errors.append(f"timeline.json row {index} has the wrong fixture_checksum")
        if row.get("registry_phase") != "stage_before_registry_owner":
            errors.append(f"timeline.json row {index} has the wrong registry availability")
        if row.get("field_availability") != "stage_before_field_owner":
            errors.append(f"timeline.json row {index} has the wrong field availability")
        if row.get("gpu_availability") != "stage_before_gpu_owner":
            errors.append(f"timeline.json row {index} has the wrong GPU availability")
        for field in nullable_fields:
            if row.get(field) is not None:
                errors.append(f"timeline.json row {index} {field} must be null at current")
        if row.get("pause_state") not in {"running", "paused"}:
            errors.append(f"timeline.json row {index} has an invalid pause_state")
        if row.get("terminal_outcome") not in timeline_contract["terminal_outcomes"]:
            errors.append(f"timeline.json row {index} has an invalid terminal_outcome")

    comparable_rows = rows[: len(expected_steps)]
    if behavior_case == "door-state-v1":
        for index, (row, expected) in enumerate(zip(comparable_rows, expected_steps)):
            exact = {
                "step_index": expected["step_index"],
                "script_update": expected["script_update"],
                "intent": expected["intent"],
                "pause_state": expected["pause_state"],
                "attempted": expected["attempted"],
                "applied": expected["current_applied"],
                "semantic_state": expected["current_semantic_state"],
                "active_presentation_state": expected[
                    "current_active_presentation_state"
                ],
                "terminal_outcome": (
                    "succeeded" if index == len(expected_steps) - 1 else "in_progress"
                ),
            }
            for field, expected_value in exact.items():
                if row.get(field) != expected_value:
                    errors.append(
                        f"timeline.json Door row {index} {field} differs from the contract"
                    )
        epochs = {row.get("world_epoch") for row in comparable_rows}
        if len(epochs) != 1:
            errors.append("timeline.json Door case changed WorldEpoch")
        ticks = [row.get("simulation_tick") for row in comparable_rows]
        if any(not isinstance(tick, int) for tick in ticks) or any(
            right < left for left, right in zip(ticks, ticks[1:])
        ):
            errors.append("timeline.json Door simulation ticks are not monotonic")
        if len(ticks) == 5 and not (ticks[3] == ticks[2] and ticks[4] == ticks[3]):
            errors.append("timeline.json Door pause did not freeze simulation ticks")
    elif behavior_case == "load-normal-v1":
        applied_by_step = [False, False, True, False, True, True]
        attempted_by_step = [False, True, False, True, False, False]
        for index, (row, expected) in enumerate(zip(comparable_rows, expected_steps)):
            exact = {
                "script_update": index,
                "intent": expected["intent"],
                "attempted": attempted_by_step[index],
                "applied": applied_by_step[index],
                "semantic_state": None,
                "active_presentation_state": None,
                "terminal_outcome": expected["terminal_outcome"],
            }
            for field, expected_value in exact.items():
                if row.get(field) != expected_value:
                    errors.append(
                        f"timeline.json load row {index} {field} differs from the contract"
                    )
        if len(comparable_rows) == 6:
            initial_epoch = comparable_rows[0].get("world_epoch")
            expected_epochs = [initial_epoch] * 4 + [initial_epoch + 1] * 2 if isinstance(
                initial_epoch, int
            ) else []
            if [row.get("world_epoch") for row in comparable_rows] != expected_epochs:
                errors.append("timeline.json normal load did not advance WorldEpoch exactly once")
            if len({row.get("pause_state") for row in comparable_rows}) != 1:
                errors.append("timeline.json normal load changed pause state")
            if any(row.get("pause_state") != "running" for row in comparable_rows):
                errors.append("timeline.json normal load did not remain running")
            ticks = [row.get("simulation_tick") for row in comparable_rows]
            if any(not isinstance(tick, int) for tick in ticks) or any(
                right < left for left, right in zip(ticks, ticks[1:])
            ):
                errors.append("timeline.json normal-load simulation ticks are not monotonic")

    save_path = data_dir / "behavior-save.scn.ron"
    save_artifact: dict[str, Any] | None = None
    if behavior_case == "load-normal-v1":
        if not save_path.is_file():
            errors.append("normal-load behavior is missing behavior-save.scn.ron")
        else:
            size = save_path.stat().st_size
            if size <= 0:
                errors.append("behavior-save.scn.ron is empty")
            else:
                try:
                    contents = save_path.read_text(encoding="utf-8")
                except (OSError, UnicodeError) as error:
                    errors.append(f"cannot decode behavior-save.scn.ron: {error}")
                else:
                    header_match = re.fullmatch(
                        r"HELL_WORKERS_SAVE\n\(format_version: ([0-9]+), "
                        r"worldgen_seed: ([0-9]+)\)\n---\n([\s\S]+)",
                        contents,
                    )
                    if header_match is None:
                        errors.append("behavior-save.scn.ron has an invalid v1 header or empty body")
                    else:
                        format_version = int(header_match.group(1))
                        worldgen_seed = int(header_match.group(2))
                        body = header_match.group(3)
                        if format_version != 1:
                            errors.append("behavior-save.scn.ron format_version is not 1")
                        if worldgen_seed != expected_case.seed:
                            errors.append(
                                "behavior-save.scn.ron worldgen_seed differs from the case"
                            )
                        save_artifact = {
                            "path": "data/behavior-save.scn.ron",
                            "size_bytes": size,
                            "sha256": sha256(save_path),
                            "format_version": format_version,
                            "worldgen_seed": worldgen_seed,
                            "payload_size_bytes": len(body.encode("utf-8")),
                            "payload_sha256": hashlib.sha256(
                                body.encode("utf-8")
                            ).hexdigest(),
                            "terminal_fixture_checksum": fixture_checksum,
                        }
    elif save_path.exists():
        errors.append("Door behavior unexpectedly wrote behavior-save.scn.ron")

    if errors:
        return None, save_artifact, errors
    return rows, save_artifact, []


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
    expected_window_backend: str = "auto",
    expected_present_mode: str = "novsync",
    expected_window_width: int | None = None,
    expected_window_height: int | None = None,
    expected_window_scale_factor: float | None = None,
    expected_rtt_quality: str | None = None,
    expected_contract: str | None = None,
    expected_stage: str | None = None,
    expected_lane: str | None = None,
) -> Validation:
    reasons: list[str] = []
    data_dir = run_dir / "data"
    summary = None
    determinism = None
    determinism_records = None
    scene_roots = None
    render_inventory = None
    indoor_light_fixture = None
    indoor_light_layout = None
    indoor_light_presentation = None
    timeline = None
    behavior_save_artifact = None
    window, window_errors = read_window(
        data_dir / "window.csv",
        expect_headless=expected_window_backend == "headless",
        expected_width=expected_window_width,
        expected_height=expected_window_height,
        expected_scale_factor=expected_window_scale_factor,
        expected_rtt_quality=expected_rtt_quality,
        expected_window_backend=expected_window_backend,
        expected_backend=expected_backend,
        expected_present_mode=expected_present_mode,
    )
    reasons.extend(window_errors)
    indoor_sidecar_paths = (
        data_dir / "indoor_light_fixture.csv",
        data_dir / "indoor_light_layout.csv",
        data_dir / "indoor_light_presentation.csv",
    )
    if expected_case.workload == "indoor-light":
        if expected_contract is None or expected_stage is None or expected_lane is None:
            reasons.append(
                "indoor-light validation requires expected contract, stage, and lane"
            )
        else:
            (
                indoor_light_fixture,
                indoor_light_layout,
                indoor_light_presentation,
                indoor_errors,
            ) = read_indoor_light_sidecars(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
                lane=expected_lane,
            )
            reasons.extend(indoor_errors)
    else:
        unexpected_sidecars = [path.name for path in indoor_sidecar_paths if path.exists()]
        if unexpected_sidecars:
            reasons.append(
                "non-indoor workload must not write indoor-light sidecars: "
                + ", ".join(unexpected_sidecars)
            )
    if capture_kind == "frame-time":
        summary, summary_errors = read_summary(data_dir / "summary.csv")
        reasons.extend(summary_errors)
        scene_roots, scene_root_errors = read_scene_roots(data_dir / "scene_roots.csv")
        reasons.extend(scene_root_errors)
        if expected_case.workload == "indoor-light":
            render_inventory, render_inventory_errors = read_render_inventory(
                data_dir / "render_inventory.csv"
            )
            reasons.extend(render_inventory_errors)
        elif (data_dir / "render_inventory.csv").exists():
            reasons.append(
                "non-indoor workload must not write render_inventory.csv"
            )
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
            if determinism is not None:
                determinism_records, record_errors = read_determinism_records(
                    data_dir / "determinism_records.csv",
                    determinism,
                    expected_workload=expected_case.workload,
                    expected_indoor_actor_counts=(
                        build_fixture_audit_actor_counts(
                            load_rtt_light_contract(expected_contract),
                            expected_case.size,
                        )
                        if expected_case.workload == "indoor-light"
                        and expected_contract is not None
                        else None
                    ),
                )
                reasons.extend(record_errors)
    elif capture_kind == "fixed-step-behavior":
        if expected_contract is None or expected_stage is None:
            reasons.append("behavior validation requires expected contract and stage")
        else:
            timeline, behavior_save_artifact, timeline_errors = read_behavior_timeline(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
            )
            reasons.extend(timeline_errors)
    else:
        reasons.append(f"unsupported capture kind {capture_kind!r}")
    if capture_kind != "frame-time" and (data_dir / "render_inventory.csv").exists():
        reasons.append("fixed-step capture must not write render_inventory.csv")
    if capture_kind == "fixed-step-determinism" and (
        (data_dir / "summary.csv").exists()
        or (data_dir / "frames.csv").exists()
        or (data_dir / "scene_roots.csv").exists()
    ):
        reasons.append("fixed-step audit must not write frame-time artifacts")
    if capture_kind == "frame-time" and (
        (data_dir / "determinism.csv").exists()
        or (data_dir / "determinism_records.csv").exists()
    ):
        reasons.append("frame-time capture must not write determinism artifacts")
    if capture_kind == "fixed-step-behavior":
        unexpected_behavior_artifacts = [
            path.name
            for path in (
                data_dir / "summary.csv",
                data_dir / "frames.csv",
                data_dir / "scene_roots.csv",
                data_dir / "determinism.csv",
                data_dir / "determinism_records.csv",
            )
            if path.exists()
        ]
        if unexpected_behavior_artifacts:
            reasons.append(
                "behavior capture must not write frame-time or determinism artifacts: "
                + ", ".join(unexpected_behavior_artifacts)
            )
        expected_behavior_files = {
            "window.csv",
            "indoor_light_fixture.csv",
            "indoor_light_layout.csv",
            "indoor_light_presentation.csv",
            "timeline.json",
        }
        if expected_case.behavior_case == "load-normal-v1":
            expected_behavior_files.add("behavior-save.scn.ron")
        actual_behavior_files = {
            path.name for path in data_dir.iterdir()
        } if data_dir.is_dir() else set()
        unknown_behavior_files = sorted(actual_behavior_files - expected_behavior_files)
        missing_behavior_files = sorted(expected_behavior_files - actual_behavior_files)
        if unknown_behavior_files:
            reasons.append(
                "behavior capture wrote unknown data artifacts: "
                + ", ".join(unknown_behavior_files)
            )
        if missing_behavior_files:
            reasons.append(
                "behavior capture is missing data artifacts: "
                + ", ".join(missing_behavior_files)
            )
    elif (data_dir / "timeline.json").exists() or (
        data_dir / "behavior-save.scn.ron"
    ).exists():
        reasons.append("non-behavior capture must not write behavior artifacts")
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
            "dashboard_mode": expected_case.dashboard_mode,
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
        frame_samples, frame_errors = read_frames(data_dir / "frames.csv", samples)
        reasons.extend(frame_errors)
        if frame_samples:
            computed_summary = frame_summary(frame_samples)
            for field, computed in computed_summary.items():
                try:
                    declared = float(summary[field])
                except (KeyError, TypeError, ValueError):
                    continue
                if not math.isclose(declared, computed, rel_tol=0.0, abs_tol=1e-6):
                    reasons.append(
                        f"summary {field} is {declared:.6f}, but frames.csv computes "
                        f"{computed:.6f}"
                    )
        for field, expected in (
            ("warmup_virtual_secs", expected_warmup_secs),
            ("measure_virtual_secs", expected_measure_secs),
        ):
            if expected is None:
                continue
            try:
                observed = float(summary[field])
            except (KeyError, TypeError, ValueError):
                continue
            if observed + 1e-6 < expected:
                reasons.append(
                    f"summary {field} is {observed:.6f}, below requested {expected:.6f}"
                )

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

    if scene_roots is not None and render_inventory is not None:
        for column in (
            "soul_proxy_3d",
            "soul_mask_proxy_3d",
            "soul_shadow_proxy_3d",
            "familiar_proxy_3d",
        ):
            if render_inventory.get(column) != scene_roots.get(column):
                reasons.append(
                    f"render_inventory.csv {column} differs from scene_roots.csv"
                )

    if determinism is not None:
        if expected_fixed_hz is not None:
            expected_timestep_ns = round(1_000_000_000 / expected_fixed_hz)
            observed_timestep_ns = int(determinism[0]["fixed_timestep_ns"])
            if observed_timestep_ns != expected_timestep_ns:
                reasons.append(
                    "determinism.csv fixed_timestep_ns is "
                    f"{observed_timestep_ns}, expected {expected_timestep_ns} "
                    f"for {expected_fixed_hz} Hz"
                )
        for index, row in enumerate(determinism):
            if row.get("dashboard_mode") != expected_case.dashboard_mode:
                reasons.append(
                    "determinism.csv row "
                    f"{index} dashboard_mode is {row.get('dashboard_mode')!r}, "
                    f"expected {expected_case.dashboard_mode!r}"
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
            if capture_kind == "fixed-step-determinism"
            else "PERF_BEHAVIOR: wrote"
        )
        if completion_marker not in log_text:
            reasons.append(f"{completion_marker} completion marker is absent")
        expected_clock_mode = {
            "frame-time": "realtime",
            "fixed-step-determinism": "fixed",
            "fixed-step-behavior": "fixed-behavior",
        }.get(capture_kind)
        if f"clock={expected_clock_mode}" not in log_text:
            reasons.append(
                f"PERF_SCENARIO clock marker is absent or does not match {expected_clock_mode!r}"
            )
        for marker in (
            f"seed={expected_case.seed}",
            f"workload={expected_case.workload}",
            f"size={expected_case.size}",
            f"render={expected_case.render}",
            f"familiar_policy={expected_case.familiar_policy}",
            f"operation_dialog={expected_case.operation_dialog}",
            f"dashboard_mode={expected_case.dashboard_mode}",
        ):
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
        behavior_marker = f"behavior_case={expected_case.behavior_case or 'none'}"
        if behavior_marker not in log_text:
            reasons.append(f"PERF_SCENARIO marker is absent: {behavior_marker}")
        if capture_kind in {"fixed-step-determinism", "fixed-step-behavior"} and expected_fixed_hz is not None:
            marker = f"fixed_hz={expected_fixed_hz}"
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
        if capture_kind == "frame-time":
            for name, expected in (
                ("warmup", expected_warmup_secs),
                ("measure", expected_measure_secs),
            ):
                if expected is None:
                    continue
                marker = f"{name}={expected:g}s"
                if marker not in log_text:
                    reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
    warnings, teardown_warnings = classify_log_warnings(log_text, allow_log_patterns)
    reasons.extend(f"unexpected log warning/error: {line}" for line in warnings)

    adapter = parse_adapter(log_text)
    if adapter is not None and window is not None and window.get("window_present") == "true":
        if window.get("adapter_name") != adapter.get("name"):
            reasons.append("window.csv adapter_name differs from run.log")
        if window.get("adapter_backend") != adapter.get("backend", "").casefold():
            reasons.append("window.csv adapter_backend differs from run.log")
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
        determinism_records=determinism_records,
        scene_roots=scene_roots,
        render_inventory=render_inventory,
        window=window,
        indoor_light_fixture=indoor_light_fixture,
        indoor_light_layout=indoor_light_layout,
        indoor_light_presentation=indoor_light_presentation,
        timeline=timeline,
        behavior_save_artifact=behavior_save_artifact,
        profile_artifact=None,
    )
