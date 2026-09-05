from __future__ import annotations

import csv
import hashlib
import json
from pathlib import Path
from typing import Any, Iterable

def reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for key, value in pairs:
        if key in payload:
            raise ValueError(f"duplicate JSON key: {key}")
        payload[key] = value
    return payload

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
