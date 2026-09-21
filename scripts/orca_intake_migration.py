"""Bind one legacy front-desk request to one immutable Linear intake.

The mapping is explicit, durable and one-to-one. It never deletes old intake or
consultation state, uploads history, starts an agent, or creates Orca work. Once
linked, the legacy request cannot start or resume coordinator work; operators
must use the mapped Linear snapshot while old state remains readable.
"""

from __future__ import annotations

import argparse
import json
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path

try:
    import orca_frontdesk as frontdesk
    import orca_issue_context as linear
    from host_coordination import acquire_host, state_root
except ModuleNotFoundError:
    from scripts import orca_frontdesk as frontdesk, orca_issue_context as linear
    from scripts.host_coordination import acquire_host, state_root


class IntakeMigrationError(RuntimeError):
    """Fail-closed migration error with no intake body content."""


def migration_root() -> Path:
    return frontdesk.checked_directory(state_root().parent / "intake-migration")


def ledger_path() -> Path:
    return migration_root() / "mappings.json"


def read_ledger(path: Path) -> dict:
    data = frontdesk.read_private_json(path, {"schema": 1, "mappings": []})
    if (
        not isinstance(data, dict)
        or set(data) != {"schema", "mappings"}
        or data.get("schema") != 1
        or not isinstance(data.get("mappings"), list)
    ):
        raise IntakeMigrationError(
            "invalid intake migration ledger; preserve it for recovery"
        )
    legacy_ids: set[str] = set()
    linear_ids: set[str] = set()
    for item in data["mappings"]:
        if not isinstance(item, dict) or set(item) != {
            "legacy_request_id",
            "linear_request_id",
            "created_at",
        }:
            raise IntakeMigrationError("invalid intake migration record")
        legacy_id = linear.canonical_uuid(
            item["legacy_request_id"], "legacy request id"
        )
        linear_id = linear.canonical_uuid(
            item["linear_request_id"], "Linear request id"
        )
        if legacy_id == linear_id or legacy_id in legacy_ids or linear_id in linear_ids:
            raise IntakeMigrationError("duplicate or cyclic intake migration record")
        try:
            created = datetime.fromisoformat(item["created_at"])
        except (TypeError, ValueError) as error:
            raise IntakeMigrationError("invalid intake migration timestamp") from error
        if created.tzinfo is None:
            raise IntakeMigrationError("invalid intake migration timestamp")
        legacy_ids.add(legacy_id)
        linear_ids.add(linear_id)
    return data


@contextmanager
def ledger():
    # Serialize with Linear import so an intake cannot be linked while its
    # immutable snapshot mapping is only partially durable.
    with acquire_host("linear-intake-state", inherit=False):
        path = ledger_path()
        yield path, read_ledger(path)


def imported_linear_ids() -> set[str]:
    return {
        item["request_id"]
        for item in linear.read_ledger(linear.ledger_path())["imports"]
    }


def link_requests(legacy_request_id: str, linear_request_id: str) -> dict:
    legacy_request_id = linear.canonical_uuid(legacy_request_id, "legacy request id")
    linear_request_id = linear.canonical_uuid(linear_request_id, "Linear request id")
    if legacy_request_id == linear_request_id:
        raise IntakeMigrationError("legacy and Linear request ids must differ")

    requests = {item["id"] for item in frontdesk.list_requests()}
    if legacy_request_id not in requests or linear_request_id not in requests:
        raise IntakeMigrationError("both intake requests must already exist")
    linear_ids = imported_linear_ids()
    if linear_request_id not in linear_ids:
        raise IntakeMigrationError("target request is not an immutable Linear intake")
    if legacy_request_id in linear_ids:
        raise IntakeMigrationError("source request is already a Linear intake")

    with ledger() as (path, data):
        for item in data["mappings"]:
            if (
                item["legacy_request_id"] == legacy_request_id
                or item["linear_request_id"] == linear_request_id
            ):
                if (
                    item["legacy_request_id"] != legacy_request_id
                    or item["linear_request_id"] != linear_request_id
                ):
                    raise IntakeMigrationError(
                        "intake request is already mapped elsewhere"
                    )
                frontdesk.sync_directory(path.parent)
                return item
        record = {
            "legacy_request_id": legacy_request_id,
            "linear_request_id": linear_request_id,
            "created_at": datetime.now(timezone.utc).isoformat(),
        }
        data["mappings"].append(record)
        frontdesk.write_ledger(path, data)
        return record


def mapping_for_legacy(request_id: str) -> dict | None:
    request_id = linear.canonical_uuid(request_id, "request id")
    with ledger() as (_, data):
        return next(
            (
                item
                for item in data["mappings"]
                if item["legacy_request_id"] == request_id
            ),
            None,
        )


def ensure_start_allowed(request_id: str) -> None:
    mapping = mapping_for_legacy(request_id)
    if mapping is not None:
        raise IntakeMigrationError(
            "legacy intake was migrated; use its mapped Linear request and keep old state read-only"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    link = subparsers.add_parser(
        "link", help="bind an old request to an imported Linear snapshot"
    )
    link.add_argument("--legacy-request", required=True)
    link.add_argument("--linear-request", required=True)
    subparsers.add_parser("list", help="list migration metadata without intake bodies")
    args = parser.parse_args()
    try:
        if args.action == "link":
            result = link_requests(args.legacy_request, args.linear_request)
        else:
            with ledger() as (_, data):
                result = data
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except (IntakeMigrationError, OSError, ValueError, RuntimeError) as error:
        print(f"受付移行を停止しました: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
