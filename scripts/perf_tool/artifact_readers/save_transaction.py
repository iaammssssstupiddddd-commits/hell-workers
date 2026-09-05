from __future__ import annotations

import re
from pathlib import Path

from ..artifact_io import read_exact_csv_rows

SAVE_TRANSACTION_SCHEMA_VERSION = "4"
SAVE_TRANSACTION_COLUMNS = (
    "schema_version", "workload", "size", "render", "seed", "soul_count",
    "familiar_count", "fixture_checksum", "sample_kind", "measure_virtual_ns",
    "measure_real_ns", "body_bytes", "serialize_ns", "write_file_sync_ns",
    "commit_directory_sync_ns", "total_ns", "peak_live_growth_bytes",
)

def read_save_transaction(
    path: Path,
) -> tuple[dict[str, str] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=SAVE_TRANSACTION_COLUMNS,
        artifact_name="save_transaction.csv",
    )
    if rows is None:
        return None, errors
    if len(rows) != 1:
        return None, [
            *errors,
            "save_transaction.csv must contain exactly one data row; "
            f"got {len(rows)}",
        ]
    row = rows[0]
    if row.get("schema_version") != SAVE_TRANSACTION_SCHEMA_VERSION:
        errors.append(
            "save_transaction.csv schema_version is "
            f"{row.get('schema_version')!r}, expected {SAVE_TRANSACTION_SCHEMA_VERSION}"
        )
    if row.get("workload") != "save-transaction":
        errors.append("save_transaction.csv workload must be save-transaction")
    if row.get("sample_kind") not in {"preflight", "measured"}:
        errors.append("save_transaction.csv sample_kind must be preflight or measured")
    for column in (
        "measure_virtual_ns",
        "measure_real_ns",
        "body_bytes",
        "serialize_ns",
        "write_file_sync_ns",
        "commit_directory_sync_ns",
        "total_ns",
    ):
        value = row.get(column, "")
        try:
            parsed = int(value)
            if parsed < 0 or str(parsed) != value:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                f"save_transaction.csv {column} must be a canonical nonnegative integer"
            )
    peak_live_growth = row.get("peak_live_growth_bytes", "")
    if peak_live_growth:
        try:
            parsed = int(peak_live_growth)
            if parsed < 0 or str(parsed) != peak_live_growth:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                "save_transaction.csv peak_live_growth_bytes must be empty or a canonical nonnegative integer"
            )
    checksum = row.get("fixture_checksum", "")
    if not re.fullmatch(r"[0-9a-f]{16}", checksum or ""):
        errors.append(
            "save_transaction.csv fixture_checksum must be 16-digit lowercase hex"
        )
    return row, errors
