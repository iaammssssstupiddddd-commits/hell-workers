"""Plan and execute atomic promotion of a final Wall asset-set generation."""

from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import os
import re
import shutil
from collections.abc import Callable
from datetime import datetime
from pathlib import Path, PurePosixPath
from types import ModuleType
from typing import Any

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = WORKFLOW_ROOT.parents[1]
MANIFEST_VALIDATOR = WORKFLOW_ROOT / "scripts/validate_asset_set_manifest.py"
ASSET_SET_ID = "wall-production-v1"
ACTIVE_POINTER = Path("authority/wall-production-v1.active.json")
PLAN_FIELDS = {
    "schema_version",
    "operation",
    "asset_set_id",
    "asset_set_generation",
    "manifest_sha256",
    "source_manifest",
    "asset_root",
    "snapshot",
    "target_generation",
    "previous_active",
    "payload",
    "tool_commit",
    "tool_tree",
}
POINTER_FIELDS = {
    "schema_version",
    "asset_set_id",
    "asset_set_generation",
    "manifest_sha256",
    "receipt_path",
    "receipt_sha256",
}
RECEIPT_FIELDS = {
    "schema_version",
    "receipt_id",
    "asset_set_id",
    "asset_set_generation",
    "manifest_sha256",
    "promotion_plan_sha256",
    "m5_evidence_bundle",
    "approval",
    "previous_active",
    "new_active",
    "tool_commit",
    "tool_tree",
}
RECEIPT_ID = re.compile(r"[a-z0-9][a-z0-9._-]{7,127}")
HEX64 = re.compile(r"[0-9a-f]{64}")


class PromotionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PromotionError(message)


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def bytes_sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def load_manifest_validator() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "validate_asset_set_manifest", MANIFEST_VALIDATOR
    )
    if spec is None or spec.loader is None:
        raise PromotionError("cannot load Wall asset-set manifest validator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read_canonical_json(path: Path, label: str) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"{label} is absent")
    validator = load_manifest_validator()
    value = validator.read_json(path)
    require(isinstance(value, dict), f"{label} must be an object")
    require(
        path.read_bytes() == canonical_json(value), f"{label} is not canonical JSON"
    )
    return value


def validate_timestamp(value: Any, label: str) -> None:
    require(isinstance(value, str) and value, f"{label} timestamp is empty")
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise PromotionError(f"{label} timestamp is invalid") from error


def regular_file(path: Path, label: str) -> Path:
    require(path.is_file() and not path.is_symlink(), f"{label} is absent")
    return path.resolve()


def safe_relative(value: str, label: str) -> Path:
    posix = PurePosixPath(value)
    require(
        value
        and not posix.is_absolute()
        and "." not in posix.parts
        and ".." not in posix.parts
        and "\\" not in value,
        f"{label} path is unsafe",
    )
    return Path(*posix.parts)


def ensure_outside(path: Path, root: Path, label: str) -> None:
    require(
        not path.resolve(strict=False).is_relative_to(root.resolve()),
        f"{label} must be outside the asset root",
    )


def file_entry(source: Path, destination: str, expected_hash: str) -> dict[str, str]:
    source = regular_file(source, destination)
    require(sha256(source) == expected_hash, f"payload hash differs: {source}")
    safe_relative(destination, "payload destination")
    return {
        "source": str(source),
        "destination": destination,
        "sha256": expected_hash,
    }


def manifest_context(manifest_path: Path) -> tuple[Path, Path, Path, Path]:
    reports_root = manifest_path.resolve().parent
    staging_root = reports_root.parent
    return (
        staging_root / "blend",
        staging_root / "exports",
        reports_root,
        staging_root.parent / "licenses",
    )


def build_payload(
    manifest_path: Path, manifest: dict[str, Any]
) -> list[dict[str, str]]:
    blend_root, exports_root, reports_root, licenses_root = manifest_context(
        manifest_path
    )
    entries: list[dict[str, str]] = []
    blend = manifest["source"]["blend"]
    entries.append(
        file_entry(
            blend_root / safe_relative(blend["path"], "source blend"),
            f"source/blender/{blend['path']}",
            blend["sha256"],
        )
    )
    for record in manifest["production"]["core"]:
        entries.append(
            file_entry(
                exports_root / safe_relative(record["path"], "core asset"),
                f"exports/{record['path']}",
                record["sha256"],
            )
        )
    report_records: list[dict[str, str]] = []
    for mesh in manifest["meshes"]:
        report_records.extend(mesh["reports"].values())
    report_records.extend(report["file"] for report in manifest["set_reports"])
    report_records.append(manifest["art_review"]["artifact"])
    # The manifest also points at the texture validation report, so a generation
    # without it cannot be validated on its own.
    report_records.append(manifest["texture_report"])
    for record in report_records:
        entries.append(
            file_entry(
                reports_root / safe_relative(record["path"], "report"),
                f"reports/{record['path']}",
                record["sha256"],
            )
        )
    license_record = manifest["license"]["file"]
    entries.append(
        file_entry(
            licenses_root / safe_relative(license_record["path"], "license"),
            f"licenses/{license_record['path']}",
            license_record["sha256"],
        )
    )
    entries.append(
        file_entry(
            manifest_path,
            "manifest/wall-production-v1.asset-set.json",
            sha256(manifest_path),
        )
    )
    destinations = [entry["destination"] for entry in entries]
    require(
        len(destinations) == len(set(destinations)), "payload destination is duplicated"
    )
    return sorted(entries, key=lambda entry: entry["destination"])


def validate_pointer(value: Any) -> dict[str, Any]:
    require(
        isinstance(value, dict) and set(value) == POINTER_FIELDS,
        "active pointer fields differ",
    )
    require(
        value["schema_version"] == 1 and value["asset_set_id"] == ASSET_SET_ID,
        "active pointer identity differs",
    )
    require(
        type(value["asset_set_generation"]) is int
        and value["asset_set_generation"] > 0,
        "active pointer generation differs",
    )
    for field in ("manifest_sha256", "receipt_sha256"):
        require(
            isinstance(value[field], str) and HEX64.fullmatch(value[field]) is not None,
            f"active pointer {field} differs",
        )
    expected_receipt = (
        f"generations/{value['asset_set_generation']}/authority/promotion-receipt.json"
    )
    require(
        value["receipt_path"] == expected_receipt, "active pointer receipt path differs"
    )
    return value


def pointer_identity(asset_root: Path) -> dict[str, Any]:
    path = asset_root / ACTIVE_POINTER
    if not path.exists():
        return {"status": "absent"}
    value = validate_pointer(read_canonical_json(path, "active pointer"))
    return {
        "status": "present",
        "sha256": sha256(path),
        "asset_set_generation": value["asset_set_generation"],
        "manifest_sha256": value["manifest_sha256"],
        "receipt_sha256": value["receipt_sha256"],
    }


def build_plan(
    manifest_path: Path,
    asset_root: Path,
    snapshot: Path,
    *,
    repo: Path | None,
) -> dict[str, Any]:
    manifest_path = regular_file(manifest_path, "final asset-set manifest")
    asset_root = asset_root.resolve()
    snapshot = snapshot.resolve(strict=False)
    ensure_outside(snapshot, asset_root, "snapshot")
    validator = load_manifest_validator()
    manifest = validator.read_json(manifest_path)
    blend_root, exports_root, reports_root, licenses_root = manifest_context(
        manifest_path
    )
    validator.validate_manifest(
        manifest_path,
        mode="final",
        blend_root=blend_root,
        exports_root=exports_root,
        reports_root=reports_root,
        licenses_root=licenses_root,
        repo=repo,
    )
    generation = manifest["asset_set_generation"]
    allocated_generations = [
        int(path.name)
        for path in (asset_root / "generations").glob("*")
        if path.is_dir() and path.name.isdigit()
    ]
    quarantined_generations = [
        int(match.group(1))
        for path in (asset_root / "quarantine").glob("*.recovered")
        if (match := re.fullmatch(r"([0-9]+)\.recovered", path.name))
    ]
    if allocated_generations or quarantined_generations:
        require(
            generation > max(allocated_generations + quarantined_generations),
            "asset-set generation is not monotonically increasing",
        )
    target = asset_root / "generations" / str(generation)
    require(not target.exists(), f"target generation already exists: {target}")
    payload = build_payload(manifest_path, manifest)
    return {
        "schema_version": 1,
        "operation": "promote_wall_asset_set",
        "asset_set_id": ASSET_SET_ID,
        "asset_set_generation": generation,
        "manifest_sha256": sha256(manifest_path),
        "source_manifest": str(manifest_path),
        "asset_root": str(asset_root),
        "snapshot": str(snapshot),
        "target_generation": f"generations/{generation}",
        "previous_active": pointer_identity(asset_root),
        "payload": payload,
        "tool_commit": manifest["source"]["tool_commit"],
        "tool_tree": manifest["source"]["tool_tree"],
    }


def validate_plan(plan_path: Path, *, repo: Path | None) -> dict[str, Any]:
    plan = read_canonical_json(plan_path, "promotion plan")
    validate_plan_shape(plan)
    asset_root = Path(plan["asset_root"])
    snapshot = Path(plan["snapshot"])
    ensure_outside(snapshot, asset_root, "snapshot")
    rebuilt = build_plan(Path(plan["source_manifest"]), asset_root, snapshot, repo=repo)
    require(
        plan == rebuilt, "promotion plan differs from current inputs or active pointer"
    )
    return plan


def validate_plan_shape(plan: dict[str, Any]) -> None:
    require(set(plan) == PLAN_FIELDS, "promotion plan fields differ")
    require(
        plan["schema_version"] == 1
        and plan["operation"] == "promote_wall_asset_set"
        and plan["asset_set_id"] == ASSET_SET_ID,
        "promotion plan identity differs",
    )
    generation = plan["asset_set_generation"]
    require(
        type(generation) is int and generation > 0, "promotion plan generation differs"
    )
    require(
        plan["target_generation"] == f"generations/{generation}",
        "promotion plan target differs",
    )
    require(
        isinstance(plan["manifest_sha256"], str)
        and HEX64.fullmatch(plan["manifest_sha256"]) is not None,
        "promotion plan manifest hash differs",
    )
    require(
        isinstance(plan["source_manifest"], str)
        and Path(plan["source_manifest"]).is_absolute(),
        "promotion plan source manifest differs",
    )
    require(
        all(
            isinstance(plan[field], str)
            and re.fullmatch(r"[0-9a-f]{40}", plan[field]) is not None
            for field in ("tool_commit", "tool_tree")
        ),
        "promotion plan tool identity differs",
    )
    asset_root = Path(plan["asset_root"])
    snapshot = Path(plan["snapshot"])
    require(
        asset_root.is_absolute() and snapshot.is_absolute(),
        "promotion plan paths must be absolute",
    )
    ensure_outside(snapshot, asset_root, "snapshot")
    previous = plan["previous_active"]
    require(
        isinstance(previous, dict) and previous.get("status") in {"absent", "present"},
        "promotion plan preimage differs",
    )
    if previous["status"] == "absent":
        require(
            previous == {"status": "absent"}, "absent promotion preimage fields differ"
        )
    else:
        require(
            set(previous)
            == {
                "status",
                "sha256",
                "asset_set_generation",
                "manifest_sha256",
                "receipt_sha256",
            }
            and all(
                isinstance(previous[field], str)
                and HEX64.fullmatch(previous[field]) is not None
                for field in ("sha256", "manifest_sha256", "receipt_sha256")
            )
            and type(previous["asset_set_generation"]) is int
            and previous["asset_set_generation"] > 0,
            "present promotion preimage fields differ",
        )
    require(
        isinstance(plan["payload"], list) and plan["payload"],
        "promotion payload is empty",
    )
    destinations: set[str] = set()
    for entry in plan["payload"]:
        require(
            isinstance(entry, dict)
            and set(entry) == {"source", "destination", "sha256"}
            and isinstance(entry["source"], str)
            and Path(entry["source"]).is_absolute()
            and isinstance(entry["destination"], str)
            and isinstance(entry["sha256"], str)
            and HEX64.fullmatch(entry["sha256"]) is not None,
            "promotion payload entry differs",
        )
        safe_relative(entry["destination"], "payload destination")
        require(
            entry["destination"] not in destinations,
            "promotion payload destination is duplicated",
        )
        destinations.add(entry["destination"])


def fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def write_exclusive(path: Path, payload: bytes, mode: int = 0o444) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as handle:
            handle.write(payload)
            handle.flush()
            os.fchmod(handle.fileno(), mode)
            os.fsync(handle.fileno())
    finally:
        os.close(descriptor)


def write_atomic(path: Path, payload: bytes, suffix: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{suffix}.tmp")
    if temporary.exists():
        require(
            temporary.is_file()
            and not temporary.is_symlink()
            and temporary.read_bytes() == payload,
            f"atomic temporary file differs: {temporary}",
        )
    else:
        write_exclusive(temporary, payload)
    os.replace(temporary, path)
    return path


def copy_verified(source: Path, destination: Path, expected_hash: str) -> None:
    regular_file(source, "promotion payload")
    require(sha256(source) == expected_hash, f"promotion source hash differs: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    require(
        not destination.exists(), f"promotion destination already exists: {destination}"
    )
    shutil.copyfile(source, destination)
    require(
        sha256(destination) == expected_hash,
        f"promotion copy hash differs: {destination}",
    )
    os.chmod(destination, 0o444)
    with destination.open("rb") as handle:
        os.fsync(handle.fileno())


def fsync_tree(root: Path, hook: Callable[[str], None]) -> None:
    directories = [path for path in root.rglob("*") if path.is_dir()]
    for path in sorted(directories, key=lambda item: len(item.parts), reverse=True):
        os.chmod(path, 0o555)
        fsync_directory(path)
        hook(f"after_directory_fsync:{path.relative_to(root).as_posix()}")
    os.chmod(root, 0o555)
    fsync_directory(root)
    hook("after_directory_fsync:.")


def snapshot_payload(plan: dict[str, Any], plan_hash: str, asset_root: Path) -> bytes:
    active = asset_root / ACTIVE_POINTER
    pointer_bytes = active.read_bytes() if active.exists() else None
    value = {
        "schema_version": 1,
        "asset_set_id": ASSET_SET_ID,
        "promotion_plan_sha256": plan_hash,
        "previous_active": plan["previous_active"],
        "pointer_bytes_base64": (
            base64.b64encode(pointer_bytes).decode()
            if pointer_bytes is not None
            else None
        ),
    }
    return canonical_json(value)


def scan_receipt_id(asset_root: Path, receipt_id: str) -> None:
    receipt_paths = list(
        (asset_root / "generations").glob("*/authority/promotion-receipt.json")
    ) + list(
        (asset_root / "quarantine").glob("*.recovered/authority/promotion-receipt.json")
    )
    for receipt_path in receipt_paths:
        receipt = read_canonical_json(receipt_path, "existing promotion receipt")
        require(
            receipt.get("receipt_id") != receipt_id,
            "promotion receipt ID was already used",
        )


def receipt_value(
    *,
    plan: dict[str, Any],
    plan_hash: str,
    receipt_id: str,
    evidence_hash: str,
    approval_hash: str,
    approved_at_utc: str,
) -> dict[str, Any]:
    generation = plan["asset_set_generation"]
    return {
        "schema_version": 1,
        "receipt_id": receipt_id,
        "asset_set_id": ASSET_SET_ID,
        "asset_set_generation": generation,
        "manifest_sha256": plan["manifest_sha256"],
        "promotion_plan_sha256": plan_hash,
        "m5_evidence_bundle": {
            "path": "evidence/m5-evidence-bundle.json",
            "sha256": evidence_hash,
        },
        "approval": {
            "path": "evidence/release-approval.json",
            "sha256": approval_hash,
            "approved_at_utc": approved_at_utc,
        },
        "previous_active": plan["previous_active"],
        "new_active": {
            "asset_set_id": ASSET_SET_ID,
            "asset_set_generation": generation,
            "manifest_sha256": plan["manifest_sha256"],
        },
        "tool_commit": plan["tool_commit"],
        "tool_tree": plan["tool_tree"],
    }


def active_pointer_value(plan: dict[str, Any], receipt_hash: str) -> dict[str, Any]:
    generation = plan["asset_set_generation"]
    return {
        "schema_version": 1,
        "asset_set_id": ASSET_SET_ID,
        "asset_set_generation": generation,
        "manifest_sha256": plan["manifest_sha256"],
        "receipt_path": f"generations/{generation}/authority/promotion-receipt.json",
        "receipt_sha256": receipt_hash,
    }


def validate_receipt(
    receipt_path: Path,
    *,
    plan: dict[str, Any],
    plan_hash: str,
    generation_root: Path,
) -> dict[str, Any]:
    receipt = read_canonical_json(receipt_path, "promotion receipt")
    require(set(receipt) == RECEIPT_FIELDS, "promotion receipt fields differ")
    require(
        receipt["schema_version"] == 1 and receipt["asset_set_id"] == ASSET_SET_ID,
        "promotion receipt identity differs",
    )
    require(
        receipt["asset_set_generation"] == plan["asset_set_generation"]
        and receipt["manifest_sha256"] == plan["manifest_sha256"],
        "promotion receipt payload binding differs",
    )
    require(
        receipt["promotion_plan_sha256"] == plan_hash,
        "promotion receipt plan binding differs",
    )
    require(
        receipt["previous_active"] == plan["previous_active"],
        "promotion receipt preimage differs",
    )
    require(
        receipt["new_active"]
        == {
            "asset_set_id": ASSET_SET_ID,
            "asset_set_generation": plan["asset_set_generation"],
            "manifest_sha256": plan["manifest_sha256"],
        },
        "promotion receipt new identity differs",
    )
    require(
        receipt["tool_commit"] == plan["tool_commit"]
        and receipt["tool_tree"] == plan["tool_tree"],
        "promotion receipt tool identity differs",
    )
    require(
        isinstance(receipt["receipt_id"], str)
        and RECEIPT_ID.fullmatch(receipt["receipt_id"]) is not None,
        "promotion receipt ID differs",
    )
    for field, expected_path in (
        ("m5_evidence_bundle", "evidence/m5-evidence-bundle.json"),
        ("approval", "evidence/release-approval.json"),
    ):
        record = receipt[field]
        expected_fields = (
            {"path", "sha256", "approved_at_utc"}
            if field == "approval"
            else {"path", "sha256"}
        )
        require(
            isinstance(record, dict) and set(record) == expected_fields,
            f"promotion receipt {field} fields differ",
        )
        require(
            record["path"] == expected_path, f"promotion receipt {field} path differs"
        )
        artifact = generation_root / safe_relative(record["path"], field)
        regular_file(artifact, field)
        require(
            sha256(artifact) == record["sha256"],
            f"promotion receipt {field} hash differs",
        )
    validate_timestamp(receipt["approval"]["approved_at_utc"], "approval")
    return receipt


def apply_plan(
    plan_path: Path,
    *,
    evidence_bundle: Path,
    approval: Path,
    receipt_id: str,
    approved_at_utc: str,
    repo: Path | None,
    kill_hook: Callable[[str], None] | None = None,
) -> dict[str, Any]:
    plan = validate_plan(plan_path, repo=repo)
    require(RECEIPT_ID.fullmatch(receipt_id) is not None, "receipt ID is invalid")
    validate_timestamp(approved_at_utc, "approval")
    evidence_bundle = regular_file(evidence_bundle, "M5 evidence bundle")
    approval = regular_file(approval, "release approval")
    asset_root = Path(plan["asset_root"])
    snapshot = Path(plan["snapshot"])
    scan_receipt_id(asset_root, receipt_id)
    hook = kill_hook or (lambda _stage: None)
    plan_hash = sha256(plan_path)

    expected_snapshot = snapshot_payload(plan, plan_hash, asset_root)
    snapshot.parent.mkdir(parents=True, exist_ok=True)
    if snapshot.exists():
        require(
            snapshot.is_file()
            and not snapshot.is_symlink()
            and snapshot.read_bytes() == expected_snapshot,
            f"existing snapshot differs: {snapshot}",
        )
    else:
        write_exclusive(snapshot, expected_snapshot)
    fsync_directory(snapshot.parent)
    hook("after_snapshot_fsync")

    generations = asset_root / "generations"
    generations.mkdir(parents=True, exist_ok=True)
    fsync_directory(generations.parent)
    generation = plan["asset_set_generation"]
    target = asset_root / plan["target_generation"]
    temporary = generations / f".{ASSET_SET_ID}-g{generation}-{receipt_id}.tmp"
    require(not target.exists(), f"target generation already exists: {target}")
    require(
        not temporary.exists(), f"promotion temporary generation exists: {temporary}"
    )
    temporary.mkdir()
    hook("after_temporary_create")

    for entry in plan["payload"]:
        destination = temporary / safe_relative(
            entry["destination"], "payload destination"
        )
        copy_verified(Path(entry["source"]), destination, entry["sha256"])
        hook(f"after_copy:{entry['destination']}")
    copy_verified(
        evidence_bundle,
        temporary / "evidence/m5-evidence-bundle.json",
        sha256(evidence_bundle),
    )
    hook("after_evidence_copy")
    copy_verified(
        approval,
        temporary / "evidence/release-approval.json",
        sha256(approval),
    )
    hook("after_approval_copy")

    receipt = receipt_value(
        plan=plan,
        plan_hash=plan_hash,
        receipt_id=receipt_id,
        evidence_hash=sha256(evidence_bundle),
        approval_hash=sha256(approval),
        approved_at_utc=approved_at_utc,
    )
    receipt_path = temporary / "authority/promotion-receipt.json"
    write_exclusive(receipt_path, canonical_json(receipt))
    receipt_hash = sha256(receipt_path)
    hook("after_receipt_fsync")
    validate_receipt(
        receipt_path,
        plan=plan,
        plan_hash=plan_hash,
        generation_root=temporary,
    )
    fsync_tree(temporary, hook)
    hook("after_generation_fsync")

    os.rename(temporary, target)
    hook("after_generation_rename")
    fsync_directory(generations)
    hook("after_generation_parent_fsync")

    pointer = active_pointer_value(plan, receipt_hash)
    pointer_path = asset_root / ACTIVE_POINTER
    pointer_path.parent.mkdir(parents=True, exist_ok=True)
    pointer_temporary = pointer_path.with_name(f".{pointer_path.name}.{receipt_id}.tmp")
    write_exclusive(pointer_temporary, canonical_json(pointer))
    hook("after_pointer_temp_fsync")
    os.replace(pointer_temporary, pointer_path)
    hook("after_pointer_rename")
    fsync_directory(pointer_path.parent)
    hook("after_pointer_parent_fsync")
    validate_active_generation(asset_root, plan, plan_path)
    return {
        "status": "applied",
        "asset_set_generation": generation,
        "manifest_sha256": plan["manifest_sha256"],
        "receipt_sha256": receipt_hash,
        "active_pointer_sha256": sha256(pointer_path),
    }


def validate_active_generation(
    asset_root: Path, plan: dict[str, Any], plan_path: Path
) -> None:
    pointer_path = asset_root / ACTIVE_POINTER
    pointer = validate_pointer(read_canonical_json(pointer_path, "active pointer"))
    require(
        pointer["asset_set_generation"] == plan["asset_set_generation"],
        "active generation differs from plan",
    )
    require(
        pointer["manifest_sha256"] == plan["manifest_sha256"],
        "active manifest differs from plan",
    )
    receipt_path = asset_root / safe_relative(pointer["receipt_path"], "receipt")
    require(
        sha256(receipt_path) == pointer["receipt_sha256"],
        "promotion receipt hash differs",
    )
    validate_receipt(
        receipt_path,
        plan=plan,
        plan_hash=sha256(plan_path),
        generation_root=asset_root / plan["target_generation"],
    )


def recover_plan(plan_path: Path, *, apply: bool) -> dict[str, Any]:
    plan = read_canonical_json(plan_path, "promotion plan")
    validate_plan_shape(plan)
    asset_root = Path(plan["asset_root"])
    generation = plan["asset_set_generation"]
    generations = asset_root / "generations"
    target = generations / str(generation)
    temporaries = sorted(generations.glob(f".{ASSET_SET_ID}-g{generation}-*.tmp"))
    pointer_temporaries = sorted(
        (asset_root / ACTIVE_POINTER.parent).glob(f".{ACTIVE_POINTER.name}.*.tmp")
    )
    current = pointer_identity(asset_root)
    new_identity = {
        "status": "present",
        "asset_set_generation": generation,
        "manifest_sha256": plan["manifest_sha256"],
    }
    current_is_new = (
        current.get("status") == "present"
        and current.get("asset_set_generation") == generation
        and current.get("manifest_sha256") == plan["manifest_sha256"]
    )
    if current_is_new:
        validate_active_generation(asset_root, plan, plan_path)
    inert = target.exists() and not current_is_new
    conflict = inert and current != plan["previous_active"]
    require(not conflict, "recovery found a stale active pointer; refusing quarantine")
    moved: list[str] = []
    if apply and (temporaries or pointer_temporaries or inert):
        quarantine = asset_root / "quarantine"
        quarantine.mkdir(parents=True, exist_ok=True)
        for path in [
            *temporaries,
            *pointer_temporaries,
            *([target] if inert else []),
        ]:
            destination = quarantine / f"{path.name}.recovered"
            require(
                not destination.exists(), f"quarantine target exists: {destination}"
            )
            if path.is_dir():
                os.chmod(path, 0o755)
            os.rename(path, destination)
            if destination.is_dir():
                os.chmod(destination, 0o555)
            moved.append(str(destination))
        fsync_directory(generations)
        fsync_directory(quarantine)
    return {
        "status": "active" if current_is_new else "inert" if inert else "clean",
        "temporary_generations": [str(path) for path in temporaries],
        "temporary_pointers": [str(path) for path in pointer_temporaries],
        "quarantined": moved,
        "would_quarantine_generation": inert,
        "expected_new_identity": new_identity,
    }


def rollback_plan(plan_path: Path, snapshot: Path, *, apply: bool) -> dict[str, Any]:
    plan = read_canonical_json(plan_path, "promotion plan")
    validate_plan_shape(plan)
    snapshot_value = read_canonical_json(snapshot, "promotion snapshot")
    require(
        set(snapshot_value)
        == {
            "schema_version",
            "asset_set_id",
            "promotion_plan_sha256",
            "previous_active",
            "pointer_bytes_base64",
        },
        "promotion snapshot fields differ",
    )
    require(
        snapshot_value["promotion_plan_sha256"] == sha256(plan_path),
        "snapshot plan binding differs",
    )
    require(
        snapshot_value["previous_active"] == plan["previous_active"],
        "snapshot preimage differs",
    )
    asset_root = Path(plan["asset_root"])
    current = pointer_identity(asset_root)
    if current == plan["previous_active"]:
        return {"status": "already_rolled_back", "active": current}
    require(
        current.get("status") == "present"
        and current.get("asset_set_generation") == plan["asset_set_generation"]
        and current.get("manifest_sha256") == plan["manifest_sha256"],
        "rollback current pointer differs from promoted generation",
    )
    validate_active_generation(asset_root, plan, plan_path)
    if not apply:
        return {"status": "planned", "restore": plan["previous_active"]}

    pointer_path = asset_root / ACTIVE_POINTER
    encoded = snapshot_value["pointer_bytes_base64"]
    if plan["previous_active"]["status"] == "present":
        require(isinstance(encoded, str), "snapshot pointer bytes are absent")
        payload = base64.b64decode(encoded, validate=True)
        require(
            bytes_sha256(payload) == plan["previous_active"]["sha256"],
            "snapshot pointer hash differs",
        )
        value = json.loads(payload)
        validate_pointer(value)
        require(canonical_json(value) == payload, "snapshot pointer is not canonical")
        write_atomic(pointer_path, payload, "rollback")
        fsync_directory(pointer_path.parent)
    else:
        require(encoded is None, "absent snapshot contains pointer bytes")
        quarantine = asset_root / "quarantine"
        quarantine.mkdir(parents=True, exist_ok=True)
        destination = quarantine / (
            f"wall-production-v1.g{plan['asset_set_generation']}.rolled-back-pointer.json"
        )
        require(
            not destination.exists(), f"rollback pointer backup exists: {destination}"
        )
        os.rename(pointer_path, destination)
        fsync_directory(pointer_path.parent)
        fsync_directory(quarantine)
    return {"status": "rolled_back", "active": pointer_identity(asset_root)}


def write_plan_output(plan: dict[str, Any], output: Path | None) -> None:
    payload = canonical_json(plan)
    if output is not None:
        write_exclusive(output.resolve(strict=False), payload)
        fsync_directory(output.resolve(strict=False).parent)
    print(payload.decode(), end="")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    plan = commands.add_parser(
        "plan", help="Validate inputs and print a read-only promotion plan"
    )
    plan.add_argument("--manifest", required=True, type=Path)
    plan.add_argument("--asset-root", required=True, type=Path)
    plan.add_argument("--snapshot", required=True, type=Path)
    plan.add_argument("--output", type=Path)
    plan.add_argument("--repo", default=PROJECT_ROOT, type=Path)

    apply = commands.add_parser(
        "apply", help="Apply an explicitly confirmed sealed plan"
    )
    apply.add_argument("--plan", required=True, type=Path)
    apply.add_argument("--evidence-bundle", required=True, type=Path)
    apply.add_argument("--approval", required=True, type=Path)
    apply.add_argument("--receipt-id", required=True)
    apply.add_argument("--approved-at-utc", required=True)
    apply.add_argument("--repo", default=PROJECT_ROOT, type=Path)
    apply.add_argument("--confirm", action="store_true", required=True)

    recover = commands.add_parser(
        "recover", help="Inspect or quarantine an interrupted plan"
    )
    recover.add_argument("--plan", required=True, type=Path)
    recover.add_argument("--apply", action="store_true")

    rollback = commands.add_parser(
        "rollback", help="Inspect or restore the sealed preimage pointer"
    )
    rollback.add_argument("--plan", required=True, type=Path)
    rollback.add_argument("--snapshot", required=True, type=Path)
    rollback.add_argument("--apply", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "plan":
        plan = build_plan(
            args.manifest.resolve(),
            args.asset_root.resolve(),
            args.snapshot.resolve(),
            repo=args.repo.resolve(),
        )
        write_plan_output(plan, args.output)
    elif args.command == "apply":
        result = apply_plan(
            args.plan.resolve(),
            evidence_bundle=args.evidence_bundle.resolve(),
            approval=args.approval.resolve(),
            receipt_id=args.receipt_id,
            approved_at_utc=args.approved_at_utc,
            repo=args.repo.resolve(),
        )
        print(json.dumps(result, indent=2, sort_keys=True))
    elif args.command == "recover":
        print(
            json.dumps(
                recover_plan(args.plan.resolve(), apply=args.apply),
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(
            json.dumps(
                rollback_plan(
                    args.plan.resolve(), args.snapshot.resolve(), apply=args.apply
                ),
                indent=2,
                sort_keys=True,
            )
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        PromotionError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Wall asset-set promotion failed: {error}")
        raise SystemExit(1) from error
