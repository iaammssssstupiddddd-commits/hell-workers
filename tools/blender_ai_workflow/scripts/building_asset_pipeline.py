"""Eight-kind authoring projection and resumable installation transactions.

The codec is an explicitly supplied, already-built bevy_app executable. This
module never builds it and never approves art. Wall and Door keep their existing
validators and release paths. All inputs and receipts are byte-bound; JSON
metadata uses sorted compact JSON, while .buildingset uses the Rust codec.
"""
from __future__ import annotations

import argparse
from functools import wraps
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
from typing import Callable

try:
    import fcntl
except ImportError:
    fcntl = None

CONTRACT = Path(__file__).resolve().parents[1] / "fixtures/building-art-v1.contract.json"
KINDS = json.loads(CONTRACT.read_bytes())["kinds"]


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ValueError(reason)


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False) + "\n").encode()


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def no_symlinks(path: Path) -> Path:
    path = path.absolute()
    require(".." not in path.parts, "parent traversal is forbidden")
    for parent in (path, *path.parents):
        require(not parent.is_symlink(), f"symlink is forbidden: {parent}")
    return path


def rooted(root: Path, relative: str) -> Path:
    path = PurePosixPath(relative)
    require(isinstance(relative, str) and relative and path.parts and not path.is_absolute()
            and path.as_posix() == relative and all(p not in {".", ".."} for p in path.parts)
            and not any(c in relative for c in ("\\", ":", "#")), "noncanonical/root-external path")
    result = no_symlinks(no_symlinks(root) / relative)
    require(result.is_relative_to(root.absolute()), "path escapes root")
    return result


def read(path: Path) -> dict:
    value = json.loads(no_symlinks(path).read_bytes())
    require(isinstance(value, dict) and canonical(value) == path.read_bytes(), "metadata is not canonical")
    if "schema_version" in value:
        require(type(value["schema_version"]) is int and value["schema_version"] == 1, "unsupported metadata schema")
    return value


def file_bytes(root: Path, record: dict) -> bytes:
    require(set(record) >= {"path", "bytes", "sha256"}, "incomplete file record")
    payload = rooted(root, record["path"]).read_bytes()
    require(type(record["bytes"]) is int and record["bytes"] > 0
            and len(payload) == record["bytes"] and digest(payload) == record["sha256"], "artifact bytes/hash differs")
    return payload


def record(relative: str, payload: bytes) -> dict:
    return {"path": relative, "bytes": len(payload), "sha256": digest(payload)}


def codec(binary: Path, operation: str, manifest: str, receipt: str | None = None) -> dict:
    binary = no_symlinks(binary)
    process = subprocess.run([str(binary), "--building-asset-codec"],
        input=canonical({"operation": operation, "manifest": manifest, "receipt": receipt}),
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=False)
    require(process.returncode == 0, "runtime codec rejected input: " + process.stderr.decode(errors="replace"))
    value = json.loads(process.stdout)
    require(set(value) == {"manifest", "receipt"}, "codec response differs")
    return value


def fsync_dir(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def put(path: Path, payload: bytes, *, replace: bool = False) -> None:
    """Same bytes are a no-op, a partial atomic temporary can be retried."""
    no_symlinks(path)
    if path.exists():
        require(path.is_file(), "destination is not a file")
        if path.read_bytes() == payload:
            return
        require(replace, "immutable destination differs")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = no_symlinks(path.with_name(path.name + ".building-tmp"))
    if temporary.exists():
        require(temporary.read_bytes() == payload, "interrupted temporary differs")
    else:
        with temporary.open("xb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
    os.replace(temporary, path)
    fsync_dir(path.parent)


def identity(kind: str, generation: int, authority: str) -> dict:
    require(kind in KINDS and type(generation) is int and generation > 0, "unsupported kind/generation")
    return {"kind": kind, "generation": generation, "authority": authority, "manifest_sha256": ""}


def locator(kind: str) -> str:
    require(kind in KINDS, "unsupported building kind")
    return f"manifests/building-{KINDS[kind]['slug']}-v1.buildingset"


RELEASE_GROUPS = {
    "m4-bridge": ("Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking",
                  "SandPile", "BonePile", "OutdoorLamp", "Bridge"),
    "m2": ("Tank", "MudMixer"),
    "m3": ("Tank", "MudMixer", "RestArea", "SoulSpa"),
    "m5": ("Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking",
           "SandPile", "BonePile", "OutdoorLamp"),
}


def release_bindings(asset_root: Path, binary: Path, group: str = "m2") -> list[dict]:
    """Return exact bindings for an explicitly selected, complete release group.

    The authority pointers and manifests are re-read together, so this helper
    cannot turn a candidate or a stale locator into a production request.  It
    only emits bindings; approval, installation, and rollback remain owned by
    the transaction journal and its release receipt.
    """
    asset_root = no_symlinks(asset_root)
    require(group in RELEASE_GROUPS, "unsupported release group")
    result = []
    for kind in RELEASE_GROUPS[group]:
        active = pointer(asset_root, kind)
        require(active is not None, f"{kind} has no active release authority")
        manifest, _text, _receipt = load_set(asset_root, kind, binary)
        require(manifest["identity"] == active["identity"]
                and manifest["identity"]["authority"] == "release_approved",
                f"{kind} authority pointer/locator is not release approved")
        result.append({"identity": manifest["identity"], "locator": locator(kind)})
    return result


def export(recipe_path: Path, source_root: Path, destination: Path, binary: Path,
           *, authority: str, approval_path: Path | None = None,
           numeric_approval_path: Path | None = None) -> dict:
    """Package existing per-part GLB/PNG exports; do not invent geometry or art."""
    require(authority in {"art_preview", "isolated_candidate"}, "export cannot grant release authority")
    recipe = read(recipe_path)
    require(set(recipe) == {"schema_version", "kind", "generation", "source", "geometry_contract", "artifacts",
                           "parts", "world_preview", "catalog_preview"} and recipe["schema_version"] == 1,
            "export recipe fields differ")
    ident = identity(recipe["kind"], recipe["generation"], "art_preview")
    kind = KINDS[ident["kind"]]
    roles = ["mesh:" + r for r in kind["mesh_roles"]] + ["image:" + r for r in kind["image_roles"]]
    require([r["role"] for r in recipe["artifacts"]] == roles, "export role inventory differs")
    source = file_bytes(source_root, recipe["source"])
    geometry = file_bytes(source_root, recipe["geometry_contract"])
    files = {}
    artifacts = []
    for entry in recipe["artifacts"]:
        payload = file_bytes(source_root, entry)
        extension = "glb" if entry["role"].startswith("mesh:") else "png"
        relative = f"building_sets/{kind['slug']}/{ident['generation']}/{digest(payload)}.{extension}"
        artifacts.append({"role": entry["role"], **record(relative, payload)})
        files[relative] = payload
    manifest = {"schema_version": 1, "asset_set_id": f"building-{kind['slug']}-v1", "identity": ident,
        "source_sha256": digest(source), "export_sha256": digest(canonical(artifacts)),
        "geometry_contract_sha256": digest(geometry), "art_approval_sha256": None,
        "artifacts": artifacts, "parts": recipe["parts"], "world_preview": recipe["world_preview"],
        "catalog_preview": recipe["catalog_preview"], "receipt": None}
    preview = codec(binary, "seal", json.dumps(manifest))["manifest"]
    if authority == "isolated_candidate":
        require(approval_path is not None, "candidate requires separate art approval")
        approval = read(approval_path)
        require(approval == {"schema_version": 1, "decision": "building_art_approved",
                            "art_preview_identity": json.loads(preview)["identity"]}, "art approval identity differs")
        manifest["identity"]["authority"] = authority
        approval_bytes = canonical(approval)
        manifest["art_approval_sha256"] = digest(approval_bytes)
        files["provenance/art-approval.json"] = approval_bytes
        if ident["kind"] in {"Tank", "MudMixer"}:
            require(numeric_approval_path is not None, "M2 candidate requires separate numeric approval")
            numeric = read(numeric_approval_path)
            state = approved_numeric_state(numeric, json.loads(preview)["identity"], digest(approval_bytes))
            manifest["production_state"] = state
            numeric_bytes = canonical(numeric)
            manifest["numeric_approval_sha256"] = digest(numeric_bytes)
            files["provenance/numeric-approval.json"] = numeric_bytes
        else:
            require(numeric_approval_path is None, "numeric approval is M2-only")
    else:
        require(approval_path is None and numeric_approval_path is None, "art preview cannot claim approval")
    projection = codec(binary, "seal", json.dumps(manifest))["manifest"]
    if "production_state" in manifest:
        require(json.loads(projection)["production_state"] == manifest["production_state"],
                "numeric approval values are not preserved by the runtime codec")
    files[locator(ident["kind"])] = projection.encode()
    files["provenance/source"] = source
    files["provenance/geometry.json"] = geometry
    # Validate every destination before writing even the first artifact.
    for relative, payload in files.items():
        target = rooted(destination, relative)
        require(not target.exists() or target.read_bytes() == payload, "export generation already differs")
    for relative, payload in files.items():
        put(rooted(destination, relative), payload)
    return {"status": "exported", "identity": json.loads(projection)["identity"],
            "manifest": str(destination / locator(ident["kind"]))}


def approved_numeric_state(approval: dict, preview_identity: dict, art_digest: str) -> dict:
    require(isinstance(approval, dict) and set(approval) == {
        "schema_version", "decision", "art_preview_identity", "art_approval_sha256", "production_state"}
        and type(approval["schema_version"]) is int and approval["schema_version"] == 1
        and approval["decision"] == "building_numeric_approved"
        and approval["art_preview_identity"] == preview_identity
        and preview_identity["authority"] == "art_preview"
        and approval["art_approval_sha256"] == art_digest, "numeric approval preview/art binding differs")
    state = approval["production_state"]
    require(isinstance(state, dict) and state.get("kind") == preview_identity["kind"]
            and state["kind"] in {"Tank", "MudMixer"}, "numeric approval kind differs")
    # The runtime codec remains the authority for f32 finiteness/ranges/axis.
    # This function neither chooses values nor creates a decision.
    canonical(state)
    return state


def preview_projection(manifest: dict, binary: Path) -> dict:
    preview = {**manifest, "identity": {**manifest["identity"], "authority": "art_preview"},
               "art_approval_sha256": None, "receipt": None}
    preview.pop("production_state", None)
    preview.pop("numeric_approval_sha256", None)
    return json.loads(codec(binary, "seal", json.dumps(preview))["manifest"])


def check_numeric_provenance(root: Path, manifest: dict, binary: Path) -> None:
    if manifest["identity"]["kind"] not in {"Tank", "MudMixer"} or manifest["identity"]["authority"] == "art_preview":
        return
    numeric = read(rooted(root, "provenance/numeric-approval.json"))
    require(digest(canonical(numeric)) == manifest.get("numeric_approval_sha256"), "numeric approval digest differs")
    art = read(rooted(root, "provenance/art-approval.json"))
    require(digest(canonical(art)) == manifest["art_approval_sha256"], "numeric provenance art digest differs")
    preview = preview_projection(manifest, binary)
    require(art == {"schema_version": 1, "decision": "building_art_approved",
                    "art_preview_identity": preview["identity"]}, "numeric provenance art identity differs")
    require(approved_numeric_state(numeric, preview["identity"], manifest["art_approval_sha256"])
            == manifest["production_state"], "approved numeric values differ")
    for relative, field in (("provenance/source", "source_sha256"), ("provenance/geometry.json", "geometry_contract_sha256")):
        require(digest(rooted(root, relative).read_bytes()) == manifest[field], "numeric source provenance differs")


def load_set(root: Path, kind: str, binary: Path) -> tuple[dict, str, str | None]:
    text = rooted(root, locator(kind)).read_text()
    manifest = json.loads(text)
    require(manifest["identity"]["kind"] == kind, "kind dispatch mismatch")
    receipt = file_bytes(root, manifest["receipt"]).decode() if manifest["receipt"] else None
    codec(binary, "validate", text, receipt)
    provenance = root if manifest["identity"]["authority"] != "release_approved" else rooted(
        root, f"building_sets/{KINDS[kind]['slug']}/{manifest['identity']['generation']}")
    check_numeric_provenance(provenance, manifest, binary)
    for artifact in manifest["artifacts"]:
        file_bytes(root, artifact)
    return manifest, text, receipt


def pointer(root: Path, kind: str) -> dict | None:
    path = rooted(root, f"authority/building-{KINDS[kind]['slug']}.json")
    return read(path) if path.exists() else None


def plan(source_root: Path, asset_root: Path, kind: str, binary: Path) -> dict:
    source_root, asset_root = no_symlinks(source_root), no_symlinks(asset_root)
    require(not source_root.is_relative_to(asset_root) and not asset_root.is_relative_to(source_root),
            "source and installation roots must be disjoint")
    candidate, text, _ = load_set(source_root, kind, binary)
    require(candidate["identity"]["authority"] == "isolated_candidate", "promotion requires isolated candidate")
    approval = rooted(source_root, "provenance/art-approval.json").read_bytes()
    require(digest(approval) == candidate["art_approval_sha256"], "candidate art approval hash differs")
    expected_preview = preview_projection(candidate, binary)
    require(read(source_root / "provenance/art-approval.json") == {
        "schema_version": 1, "decision": "building_art_approved", "art_preview_identity": expected_preview["identity"]},
        "candidate art approval identity differs")
    for relative, field in (("provenance/source", "source_sha256"), ("provenance/geometry.json", "geometry_contract_sha256")):
        require(digest(rooted(source_root, relative).read_bytes()) == candidate[field], "source provenance differs")
    require(candidate["export_sha256"] == digest(canonical(candidate["artifacts"])), "export inventory identity differs")
    release = {**candidate, "identity": {**candidate["identity"], "authority": "release_approved"}}
    projected = codec(binary, "seal", json.dumps(release))
    generation = candidate["identity"]["generation"]
    allocated = [int(entry.name) for entry in rooted(asset_root, f"building_sets/{KINDS[kind]['slug']}").glob("*")
                 if entry.name.isdigit()]
    prefix = f"building-{KINDS[kind]['slug']}-"
    allocated.extend(int(entry.stem[len(prefix):])
        for entry in rooted(asset_root, "transactions").glob(prefix + "*.json")
        if entry.stem[len(prefix):].isdigit())
    require(not allocated or generation > max(allocated), "generation must exceed all allocated attempts")
    current = pointer(asset_root, kind)
    if current:
        active, _, _ = load_set(asset_root, kind, binary)
        require(current["identity"] == active["identity"], "active pointer/locator differs")
        require(generation > current["identity"]["generation"], "generation must increase")
    else:
        require(not rooted(asset_root, locator(kind)).exists(), "unowned runtime locator")
    generation_root = rooted(asset_root, f"building_sets/{KINDS[kind]['slug']}/{generation}")
    require(not generation_root.exists(), "generation already allocated; use the existing sealed plan")
    return {"schema_version": 1, "operation": "promote_building_asset_set", "kind": kind,
            "source_root": str(source_root), "asset_root": str(asset_root),
            "candidate_identity": candidate["identity"], "candidate_sha256": digest(text.encode()),
            "previous": current, "previous_locator": (rooted(asset_root, locator(kind)).read_text() if current else None),
            "projection": projected, "codec_sha256": digest(no_symlinks(binary).read_bytes())}


def checked_plan(path: Path, binary: Path, *, source_required: bool = True) -> dict:
    value = read(path)
    require(set(value) == {"schema_version", "operation", "kind", "source_root", "asset_root", "candidate_identity",
                          "candidate_sha256", "previous", "previous_locator", "projection", "codec_sha256"}
            and value["schema_version"] == 1 and value["operation"] == "promote_building_asset_set", "plan fields differ")
    require(value["kind"] in KINDS and digest(no_symlinks(binary).read_bytes()) == value["codec_sha256"], "plan kind/codec differs")
    root = no_symlinks(Path(value["asset_root"]))
    source = no_symlinks(Path(value["source_root"]))
    require(str(root) == value["asset_root"] and str(source) == value["source_root"]
            and not root.is_relative_to(source) and not source.is_relative_to(root), "plan roots differ")
    if source_required:
        candidate, text, _ = load_set(source, value["kind"], binary)
        provenance = source
    else:
        projection = value["projection"]
        codec(binary, "validate", projection["manifest"], projection["receipt"])
        candidate = json.loads(projection["manifest"])
        candidate["identity"]["authority"] = "isolated_candidate"
        candidate["receipt"] = None
        text = codec(binary, "seal", json.dumps(candidate))["manifest"]
        candidate = json.loads(text)
        for entry in candidate["artifacts"]:
            file_bytes(root, entry)
        provenance = rooted(root,
            f"building_sets/{KINDS[value['kind']]['slug']}/{candidate['identity']['generation']}")
    require(candidate["identity"] == value["candidate_identity"]
            and candidate["identity"]["authority"] == "isolated_candidate"
            and digest(text.encode()) == value["candidate_sha256"], "planned candidate changed")
    require(digest(rooted(provenance, "provenance/art-approval.json").read_bytes()) == candidate["art_approval_sha256"],
            "planned art approval changed")
    check_numeric_provenance(provenance, candidate, binary)
    for relative, field in (("provenance/source", "source_sha256"), ("provenance/geometry.json", "geometry_contract_sha256")):
        require(digest(rooted(provenance, relative).read_bytes()) == candidate[field], "planned source provenance changed")
    expected = {**candidate, "identity": {**candidate["identity"], "authority": "release_approved"}}
    require(codec(binary, "seal", json.dumps(expected)) == value["projection"], "planned release differs")
    return value


def transaction_path(root: Path, value: dict) -> Path:
    generation = value["candidate_identity"]["generation"]
    return rooted(root, f"transactions/building-{KINDS[value['kind']]['slug']}-{generation}.json")


def transaction_locked(function):
    """Serialize mutations of one asset root; dry-run remains read-only."""
    @wraps(function)
    def wrapped(*args, **kwargs):
        if kwargs.get("execute") is False:
            return function(*args, **kwargs)
        require(fcntl is not None, "transaction locking is unavailable")
        root = args[1] if function.__name__ == "install" else Path(read(args[0])["asset_root"])
        root = no_symlinks(root)
        root.mkdir(parents=True, exist_ok=True)
        lock = rooted(root, ".building-pipeline.lock")
        with lock.open("a+b") as stream:
            try:
                fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise ValueError("building asset transaction is busy") from error
            try:
                return function(*args, **kwargs)
            finally:
                fcntl.flock(stream, fcntl.LOCK_UN)
    return wrapped


def verify_candidate_evidence(evidence_path: Path) -> dict:
    scripts = Path(__file__).resolve().parents[3] / "scripts"
    if str(scripts) not in sys.path:
        sys.path.insert(0, str(scripts))
    import building_art_acceptance
    require(evidence_path.name == "manifest.json", "candidate evidence must be the native job manifest")
    return building_art_acceptance.verify(evidence_path.parent)


@transaction_locked
def apply(path: Path, binary: Path, evidence_path: Path, approval_path: Path,
          hook: Callable[[str], None] = lambda _stage: None) -> dict:
    value = checked_plan(path, binary)
    root, source, kind = Path(value["asset_root"]), Path(value["source_root"]), value["kind"]
    evidence, approval = read(evidence_path), read(approval_path)
    require(evidence.get("schema_version") == 1 and evidence.get("profile") == "building-art"
            and evidence.get("mode") == "candidate" and evidence.get("status") == "pass"
            and evidence.get("identity") == value["candidate_identity"]
            and evidence.get("feedback_only") is False, "formal candidate evidence differs")
    require(approval == {"schema_version": 1, "decision": "building_release_approved",
                        "candidate_identity": value["candidate_identity"], "evidence_sha256": digest(evidence_path.read_bytes())},
            "release approval/evidence binding differs")
    projected = json.loads(value["projection"]["manifest"])
    new = {"schema_version": 1, "identity": projected["identity"], "plan_sha256": digest(path.read_bytes()),
           "evidence_sha256": digest(evidence_path.read_bytes()), "approval_sha256": digest(approval_path.read_bytes())}
    journal_path = transaction_path(root, value)
    if not journal_path.exists():
        require(verify_candidate_evidence(evidence_path) == evidence, "raw candidate evidence verification differs")
        require(plan(source, root, kind, binary) == value, "plan no longer matches its source/active preimage")
    journal = {"schema_version": 1, "plan_sha256": digest(path.read_bytes()), "previous": value["previous"],
               "previous_locator": value["previous_locator"], "new": new,
               "evidence": evidence, "approval": approval, "plan": value, "state": "applying"}
    if journal_path.exists():
        saved = read(journal_path)
        require(saved == {**journal, "state": saved["state"]}, "transaction journal differs")
        require(saved["state"] in {"applying", "applied"}, "rolled-back transaction cannot be reapplied")
    require(pointer(root, kind) in (value["previous"], new), "active authority changed")
    current_locator = rooted(root, locator(kind))
    require((current_locator.read_text() if current_locator.exists() else None)
            in (value["previous_locator"], value["projection"]["manifest"]), "runtime locator changed")
    files = {entry["path"]: file_bytes(source, entry) for entry in projected["artifacts"]}
    files[projected["receipt"]["path"]] = value["projection"]["receipt"].encode()
    generation_prefix = f"building_sets/{KINDS[kind]['slug']}/{projected['identity']['generation']}"
    for relative in ("provenance/source", "provenance/geometry.json", "provenance/art-approval.json"):
        files[f"{generation_prefix}/{relative}"] = rooted(source, relative).read_bytes()
    if kind in {"Tank", "MudMixer"}:
        files[f"{generation_prefix}/provenance/numeric-approval.json"] = rooted(source, "provenance/numeric-approval.json").read_bytes()
    for relative, payload in files.items():
        target = rooted(root, relative)
        require(not target.exists() or target.read_bytes() == payload, "immutable installed bytes differ")
    put(journal_path, canonical(journal) if not journal_path.exists() else journal_path.read_bytes())
    hook("after_snapshot")
    for relative, payload in files.items():
        put(rooted(root, relative), payload)
        hook("after_payload")
    codec(binary, "validate", value["projection"]["manifest"], value["projection"]["receipt"])
    put(rooted(root, f"authority/building-{KINDS[kind]['slug']}.json"), canonical(new), replace=True)
    hook("after_authority")
    put(current_locator, value["projection"]["manifest"].encode(), replace=True)
    hook("after_locator")
    put(journal_path, canonical({**journal, "state": "applied"}), replace=True)
    return {"status": "applied", "identity": projected["identity"]}


def recover(path: Path, binary: Path, *, execute: bool) -> dict:
    pending = read(path)
    saved_path = transaction_path(Path(pending["asset_root"]), pending)
    finished = saved_path.exists() and read(saved_path)["state"] in {"applied", "rolled_back"}
    value = checked_plan(path, binary, source_required=not finished)
    root = Path(value["asset_root"])
    journal_path = transaction_path(root, value)
    if not journal_path.exists():
        return {"status": "clean"}
    journal = read(journal_path)
    require(journal["plan_sha256"] == digest(path.read_bytes()), "recovery plan binding differs")
    if journal["state"] == "rolled_back":
        require(pointer(root, value["kind"]) == value["previous"], "rollback authority changed")
        return {"status": "rolled_back"}
    if journal["state"] == "applied":
        require(pointer(root, value["kind"]) == journal["new"]
                and rooted(root, locator(value["kind"])).read_text() == value["projection"]["manifest"],
                "completed transaction authority/locator differs")
        return {"status": "applied", "identity": journal["new"]["identity"]}
    if not execute:
        return {"status": journal["state"]}
    # Persisted inputs are retained as transaction authority, never fabricated.
    evidence = rooted(root, f"transactions/{journal['plan_sha256']}.evidence.json")
    approval = rooted(root, f"transactions/{journal['plan_sha256']}.approval.json")
    put(evidence, canonical(journal["evidence"]))
    put(approval, canonical(journal["approval"]))
    return apply(path, binary, evidence, approval)


@transaction_locked
def rollback(path: Path, binary: Path, *, execute: bool) -> dict:
    value = checked_plan(path, binary, source_required=False)
    root, kind = Path(value["asset_root"]), value["kind"]
    journal_path = transaction_path(root, value)
    journal = read(journal_path)
    require(journal["plan_sha256"] == digest(path.read_bytes()) and journal["previous"] == value["previous"]
            and journal["previous_locator"] == value["previous_locator"], "rollback snapshot binding differs")
    require(journal["state"] in {"applied", "rolling_back", "rolled_back"}, "recover incomplete apply before rollback")
    require(pointer(root, kind) in (journal["new"], value["previous"]), "rollback current authority differs")
    target = rooted(root, locator(kind))
    require((target.read_text() if target.exists() else None)
            in (value["projection"]["manifest"], value["previous_locator"]), "rollback locator differs")
    if value["previous_locator"] is not None:
        old = json.loads(value["previous_locator"])
        receipt = file_bytes(root, old["receipt"]).decode()
        codec(binary, "validate", value["previous_locator"], receipt)
        require(old["identity"] == value["previous"]["identity"], "rollback previous identity differs")
        for entry in old["artifacts"]:
            file_bytes(root, entry)
    if not execute:
        return {"status": "planned"}
    put(journal_path, canonical({**journal, "state": "rolling_back"}), replace=True)
    for relative, payload in ((locator(kind), value["previous_locator"]),
            (f"authority/building-{KINDS[kind]['slug']}.json", canonical(value["previous"]).decode() if value["previous"] else None)):
        target = rooted(root, relative)
        if payload is None:
            if target.exists():
                target.unlink()
                fsync_dir(target.parent)
        else:
            put(target, payload.encode(), replace=True)
    put(journal_path, canonical({**journal, "state": "rolled_back"}), replace=True)
    return {"status": "rolled_back"}


@transaction_locked
def install(source: Path, destination: Path, kind: str, binary: Path, *, execute: bool,
            allow_rollback: bool = False) -> dict:
    """Mirror an approved transaction and switch the runtime locator last."""
    source, destination = no_symlinks(source), no_symlinks(destination)
    require(not source.is_relative_to(destination) and not destination.is_relative_to(source), "install roots must be disjoint")
    active, old = pointer(source, kind), pointer(destination, kind)
    if allow_rollback and old != active:
        require(old is not None, "mirror rollback has no installed preimage")
        rollback_journal = read(rooted(source,
            f"transactions/building-{KINDS[kind]['slug']}-{old['identity']['generation']}.json"))
        require(rollback_journal["state"] == "rolled_back" and rollback_journal["new"] == old
                and rollback_journal["previous"] == active
                and digest(canonical(rollback_journal["plan"])) == old["plan_sha256"],
                "mirror rollback is not authorized by the canonical transaction")
    if allow_rollback and active is None:
        if old is None:
            require(not rooted(destination, locator(kind)).exists(), "unowned mirror locator")
            return {"status": "rolled_back"}
        if rooted(destination, locator(kind)).exists():
            current, _, _ = load_set(destination, kind, binary)
            require(current["identity"] == old["identity"], "mirror rollback locator/authority differs")
        if execute:
            for relative in (locator(kind), f"authority/building-{KINDS[kind]['slug']}.json"):
                target = rooted(destination, relative)
                target.unlink(missing_ok=True)
                fsync_dir(target.parent)
        return {"status": "rolled_back" if execute else "planned"}
    manifest, text, receipt = load_set(source, kind, binary)
    require(manifest["identity"]["authority"] == "release_approved", "install requires release authority")
    require(active is not None and active["identity"] == manifest["identity"], "install active identity differs")
    journal_relative = f"transactions/building-{KINDS[kind]['slug']}-{manifest['identity']['generation']}.json"
    journal = read(rooted(source, journal_relative))
    require(journal["state"] == "applied" and journal["new"] == active
            and digest(canonical(journal["plan"])) == active["plan_sha256"]
            and journal["plan"]["projection"] == {"manifest": text, "receipt": receipt}
            and digest(canonical(journal["approval"])) == active["approval_sha256"]
            and digest(canonical(journal["evidence"])) == active["evidence_sha256"], "install receipt/transaction binding differs")
    require(journal["approval"] == {"schema_version": 1, "decision": "building_release_approved",
        "candidate_identity": journal["plan"]["candidate_identity"], "evidence_sha256": active["evidence_sha256"]},
        "install release approval differs")
    files = {r["path"]: file_bytes(source, r) for r in [*manifest["artifacts"], manifest["receipt"]]}
    if kind in {"Tank", "MudMixer"}:
        prefix = f"building_sets/{KINDS[kind]['slug']}/{manifest['identity']['generation']}/provenance"
        for name in ("source", "geometry.json", "art-approval.json", "numeric-approval.json"):
            files[f"{prefix}/{name}"] = rooted(source, f"{prefix}/{name}").read_bytes()
    files[journal_relative] = canonical(journal)
    for relative, payload in files.items():
        target = rooted(destination, relative)
        require(not target.exists() or target.read_bytes() == payload, "immutable mirror differs")
    authority = f"authority/building-{KINDS[kind]['slug']}.json"
    if old is not None:
        require(allow_rollback or old == active or old["identity"]["generation"] < manifest["identity"]["generation"], "mirror generation regression")
    rooted(destination, locator(kind))
    rooted(destination, authority)
    if execute:
        for relative, payload in files.items():
            put(rooted(destination, relative), payload)
        require(pointer(source, kind) == active and pointer(destination, kind) == old, "mirror authority changed")
        put(rooted(destination, authority), canonical(active), replace=True)
        put(rooted(destination, locator(kind)), text.encode(), replace=True)
    return {"status": "installed" if execute else "planned", "identity": manifest["identity"]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("export", "plan", "apply", "recover", "rollback", "install", "bindings"))
    parser.add_argument("--kind", choices=tuple(KINDS))
    parser.add_argument("--release-group", choices=tuple(RELEASE_GROUPS), default="m2")
    parser.add_argument("--codec", type=Path, required=True)
    for name in ("recipe", "source-root", "asset-root", "output", "plan", "evidence", "approval", "numeric-approval"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--authority", choices=("art_preview", "isolated_candidate"), default="art_preview")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--rollback", action="store_true", help="mirror an explicitly rolled-back canonical transaction")
    args = parser.parse_args(argv)
    if args.command == "bindings":
        require(args.asset_root and args.codec, "bindings needs --asset-root/--codec")
        result = release_bindings(args.asset_root, args.codec, args.release_group)
    elif args.command == "export":
        require(args.kind is not None, "export needs --kind")
        require(all((args.recipe, args.source_root, args.asset_root)), "export needs recipe/source-root/asset-root")
        require(read(args.recipe)["kind"] == args.kind, "export kind dispatch differs")
        result = export(args.recipe, args.source_root, args.asset_root, args.codec,
                        authority=args.authority, approval_path=args.approval, numeric_approval_path=args.numeric_approval)
    elif args.command == "install":
        require(args.kind is not None, "install needs --kind")
        require(args.source_root and args.asset_root, "install needs source-root/asset-root")
        result = install(args.source_root, args.asset_root, args.kind, args.codec,
                         execute=args.apply, allow_rollback=args.rollback)
    elif args.command == "plan":
        require(args.kind is not None, "plan needs --kind")
        require(all((args.source_root, args.asset_root, args.output)), "plan needs source-root/asset-root/output")
        if args.output.exists():
            result = checked_plan(args.output, args.codec)
            require(result["kind"] == args.kind and result["source_root"] == str(args.source_root.absolute())
                    and result["asset_root"] == str(args.asset_root.absolute()), "existing plan arguments differ")
        else:
            result = plan(args.source_root, args.asset_root, args.kind, args.codec)
            put(no_symlinks(args.output), canonical(result))
    else:
        require(args.kind is not None, f"{args.command} needs --kind")
        require(args.plan is not None, "transaction needs --plan")
        require(read(args.plan)["kind"] == args.kind, "transaction kind dispatch differs")
        if args.command == "apply":
            require(args.apply and args.evidence and args.approval, "apply requires explicit --apply, evidence and approval")
            result = apply(args.plan, args.codec, args.evidence, args.approval)
        elif args.command == "recover":
            result = recover(args.plan, args.codec, execute=args.apply)
        else:
            result = rollback(args.plan, args.codec, execute=args.apply)
    print(canonical(result).decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
