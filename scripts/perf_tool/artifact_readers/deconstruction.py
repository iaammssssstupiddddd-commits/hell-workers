from __future__ import annotations

from pathlib import Path

from ..artifact_io import read_exact_csv_rows
from ..model import DECONSTRUCTION_FIXTURE_COLUMNS, DECONSTRUCTION_FIXTURE_SCHEMA_VERSION

def read_deconstruction_fixture(
    path: Path,
) -> tuple[dict[str, str] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=DECONSTRUCTION_FIXTURE_COLUMNS,
        artifact_name="deconstruction_fixture.csv",
    )
    if rows is None:
        return None, errors
    if len(rows) != 1:
        return None, [
            *errors,
            "deconstruction_fixture.csv must contain exactly one data row; "
            f"got {len(rows)}",
        ]
    row = rows[0]
    if row.get("schema_version") != DECONSTRUCTION_FIXTURE_SCHEMA_VERSION:
        errors.append(
            "deconstruction_fixture.csv schema_version is "
            f"{row.get('schema_version')!r}, expected {DECONSTRUCTION_FIXTURE_SCHEMA_VERSION}"
        )
    expected = {
        "initial_completed_buildings": 100,
        "final_completed_buildings": 99,
        "building_type_count": 12,
        "commit_requests": 1,
        "committed": 1,
        "recovery_items": 5,
        "commit_validation_passes": 1,
        "successful_cleanup_transactions": 1,
        "recovery_items_spawned": 5,
        "steady_state_validation_delta": 0,
    }
    for column, expected_value in expected.items():
        value = row.get(column, "")
        try:
            parsed = int(value)
            if parsed < 0 or str(parsed) != value:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                f"deconstruction_fixture.csv {column} must be a canonical nonnegative integer"
            )
            continue
        if parsed != expected_value:
            errors.append(
                f"deconstruction_fixture.csv {column} is {parsed}, expected {expected_value}"
            )
    for column, label in (
        ("post_commit_updates", "post_commit_updates"),
        ("successful_transaction_elapsed_ns", "successful_transaction_elapsed_ns"),
    ):
        value = row.get(column, "")
        try:
            parsed = int(value)
            if parsed <= 0 or str(parsed) != value:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                f"deconstruction_fixture.csv {label} must be a canonical positive integer"
            )
    return (row if not errors else None), errors
