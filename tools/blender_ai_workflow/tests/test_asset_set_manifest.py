from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import struct
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = WORKFLOW_ROOT / "scripts/validate_asset_set_manifest.py"


def load_validator():
    spec = importlib.util.spec_from_file_location(
        "validate_asset_set_manifest", SCRIPT_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load validate_asset_set_manifest.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def write_record(root: Path, relative: str, payload: bytes) -> dict[str, str]:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return {"path": relative, "sha256": digest(payload)}


def write_json_record(
    root: Path, relative: str, payload: dict[str, object]
) -> dict[str, str]:
    return write_record(
        root,
        relative,
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode(),
    )


def png_1024() -> bytes:
    return b"\x89PNG\r\n\x1a\n" + b"\0" * 8 + struct.pack(">II", 1024, 1024)


class AssetSetFixture:
    def __init__(self, root: Path, validator) -> None:
        self.root = root
        self.validator = validator
        self.blend_root = root / "blend"
        self.exports_root = root / "exports"
        self.reports_root = root / "reports"
        self.licenses_root = root / "licenses"
        for path in (
            self.blend_root,
            self.exports_root,
            self.reports_root,
            self.licenses_root,
        ):
            path.mkdir()

        blend = write_record(self.blend_root, "wall-production-v1.blend", b"blend")
        meshes = []
        core = []
        for family, (collection, output_path) in validator.FAMILIES.items():
            output = write_record(
                self.exports_root, output_path, f"glb:{family}".encode()
            )
            core.append(
                {
                    **copy.deepcopy(output),
                    "role": f"mesh:{family}",
                    "bytes": (self.exports_root / output_path).stat().st_size,
                }
            )
            reports = {
                "scene": write_json_record(
                    self.reports_root,
                    f"{family}.scene.json",
                    {
                        "summary": {"errors": 0},
                        "mesh_count": 1,
                        "selection": {"collection": collection},
                    },
                ),
                "export": write_json_record(
                    self.reports_root,
                    f"{family}.export.json",
                    {
                        "status": "exported",
                        "collection": collection,
                        "sha256": output["sha256"],
                    },
                ),
                "khronos": write_json_record(
                    self.reports_root,
                    f"{family}.khronos.json",
                    {"issues": {"numErrors": 0, "numWarnings": 0}},
                ),
                "post_export": write_json_record(
                    self.reports_root,
                    f"{family}.post-export.json",
                    {
                        "status": "pass",
                        "family": family,
                        "glb_sha256": output["sha256"],
                        "contract_sha256": validator.sha256(
                            validator.GEOMETRY_CONTRACT
                        ),
                    },
                ),
            }
            meshes.append(
                {
                    "family": family,
                    "collection": collection,
                    "output": output,
                    "reports": reports,
                }
            )

        for texture in ("albedo", "emissive"):
            record = write_record(
                self.exports_root, validator.TEXTURES[texture], png_1024()
            )
            core.append(
                {
                    **record,
                    "role": f"texture:{texture}",
                    "bytes": (self.exports_root / record["path"]).stat().st_size,
                }
            )
        normal_file = write_record(
            self.exports_root, validator.TEXTURES["normal"], png_1024()
        )
        normal = {
            **normal_file,
            "role": "texture:normal",
            "bytes": (self.exports_root / normal_file["path"]).stat().st_size,
        }
        texture_report = self.write_texture_report("pending")
        set_reports = [
            {
                "role": role,
                "file": write_json_record(
                    self.reports_root,
                    f"wall-production-v1.{role}.json",
                    {"status": "pass", "asset_set_id": "wall-production-v1"},
                ),
            }
            for role in ("reference_board", "rebuild")
        ]
        license_record = write_record(
            self.licenses_root, "wall-production-v1.md", b"license"
        )
        self.manifest = {
            "schema_version": 2,
            "asset_set_id": "wall-production-v1",
            "manifest_mode": "candidate",
            "asset_set_generation": 1,
            "created_at_utc": "2026-09-01T00:00:00Z",
            "source": {
                "blend": blend,
                "geometry_contract": {
                    "asset_set_id": "wall-production-v1",
                    "schema_version": 1,
                    "sha256": validator.sha256(validator.GEOMETRY_CONTRACT),
                },
                "tool_commit": "a" * 40,
                "tool_tree": "b" * 40,
                "tool_versions": {
                    "blender": "5.1.1",
                    "exporter": "Blender glTF 2.0",
                    "khronos_validator": "2.0.0-dev.3.10",
                },
                "runtime_subject": "pending",
                "source_fingerprint": "c" * 64,
            },
            "meshes": meshes,
            "production": {"core": core, "optional": [normal]},
            "normal_decision": "pending",
            "texture_report": texture_report,
            "set_reports": set_reports,
            "provenance": {
                "generators": [
                    {
                        "role": role,
                        "service": "test-service",
                        "model": "test-model",
                        "version": "1",
                        "prompt_sha256": "d" * 64,
                        "reference_sha256": ["e" * 64],
                    }
                    for role in ("mesh", "texture")
                ]
            },
            "license": {
                "name": "test-license",
                "terms_url": "https://example.invalid/terms",
                "checked_at_utc": "2026-09-01T00:00:00Z",
                "file": license_record,
            },
            "art_review": {
                "status": "candidate",
                "reviewer": "",
                "reviewed_at_utc": "",
                "notes": "",
                "artifact": None,
            },
        }
        self.manifest_path = self.reports_root / "wall-production-v1.asset-set.json"
        self.write()

    def write_texture_report(self, decision: str) -> dict[str, str]:
        roles = ["albedo", "emissive"]
        if decision in {"pending", "adopted"}:
            roles.append("normal")
        textures = {}
        for role in roles:
            path = self.exports_root / self.validator.TEXTURES[role]
            textures[role] = {
                "path": path.name,
                "bytes": path.stat().st_size,
                "sha256": digest(path.read_bytes()),
                "width": 1024,
                "height": 1024,
                "mode": "RGB",
            }
        payload = {
            "schema_version": 1,
            "status": "pass",
            "asset_set_id": "wall-production-v1",
            "normal_decision": decision,
            "textures": textures,
            "emissive": {},
            "albedo_mean_rgb": [0.0, 0.0, 0.0],
        }
        if decision in {"pending", "adopted"}:
            payload["normal"] = {}
        return write_json_record(
            self.reports_root, "wall-production-v1.textures.json", payload
        )

    def write(self) -> None:
        self.manifest_path.write_text(
            json.dumps(self.manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def make_final(self, *, normal: str) -> None:
        self.manifest["manifest_mode"] = "final"
        self.manifest["source"]["runtime_subject"] = "f" * 40
        self.manifest["normal_decision"] = normal
        normal_record = self.manifest["production"]["optional"].pop()
        if normal == "adopted":
            self.manifest["production"]["core"].append(normal_record)
        self.manifest["texture_report"] = self.write_texture_report(normal)
        approval = write_json_record(
            self.reports_root,
            "wall-production-v1.art-approval.json",
            {"status": "approved"},
        )
        self.manifest["art_review"] = {
            "status": "art_approved",
            "reviewer": "art-owner",
            "reviewed_at_utc": "2026-09-01T00:01:00Z",
            "notes": "approved fixture",
            "artifact": approval,
        }
        self.write()


class AssetSetManifestTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = load_validator()

    def validate(self, fixture: AssetSetFixture, mode: str = "candidate"):
        return self.validator.validate_manifest(
            fixture.manifest_path,
            mode=mode,
            blend_root=fixture.blend_root,
            exports_root=fixture.exports_root,
            reports_root=fixture.reports_root,
            licenses_root=fixture.licenses_root,
        )

    def test_candidate_pending_inventory_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            report = self.validate(fixture)
            self.assertEqual(report["status"], "pass")
            self.assertEqual(report["core_files"], 8)
            self.assertEqual(report["optional_files"], 1)
            self.assertEqual(report["mesh_reports"], 24)

    def test_final_rejects_pending_decisions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.manifest["manifest_mode"] = "final"
            fixture.manifest["source"]["runtime_subject"] = "f" * 40
            fixture.write()
            with self.assertRaisesRegex(
                self.validator.ManifestError, "pending normal decision"
            ):
                self.validate(fixture, mode="final")

    def test_final_rejected_normal_passes_with_eight_core_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.make_final(normal="rejected")
            report = self.validate(fixture, mode="final")
            self.assertEqual(report["core_files"], 8)
            self.assertEqual(report["optional_files"], 0)

    def test_final_adopted_normal_passes_with_nine_core_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.make_final(normal="adopted")
            report = self.validate(fixture, mode="final")
            self.assertEqual(report["core_files"], 9)
            self.assertEqual(report["optional_files"], 0)

    def test_changed_output_bytes_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            relative = fixture.manifest["meshes"][0]["output"]["path"]
            (fixture.exports_root / relative).write_bytes(b"changed")
            with self.assertRaisesRegex(self.validator.ManifestError, "sha256 differs"):
                self.validate(fixture)

    def test_declared_byte_length_is_exact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.manifest["production"]["core"][0]["bytes"] += 1
            fixture.write()
            with self.assertRaisesRegex(
                self.validator.ManifestError, "byte length differs"
            ):
                self.validate(fixture)

    def test_production_role_cannot_be_swapped(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.manifest["production"]["core"][0]["role"] = "mesh:cross"
            fixture.write()
            with self.assertRaisesRegex(self.validator.ManifestError, "role differs"):
                self.validate(fixture)

    def test_post_export_report_binds_geometry_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            report_record = fixture.manifest["meshes"][0]["reports"]["post_export"]
            payload = json.loads(
                (fixture.reports_root / report_record["path"]).read_text()
            )
            payload["contract_sha256"] = "0" * 64
            fixture.manifest["meshes"][0]["reports"]["post_export"] = write_json_record(
                fixture.reports_root, report_record["path"], payload
            )
            fixture.write()
            with self.assertRaisesRegex(
                self.validator.ManifestError, "geometry contract hash link"
            ):
                self.validate(fixture)

    def test_report_cannot_be_reused_for_two_roles(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.manifest["meshes"][1]["reports"]["scene"] = copy.deepcopy(
                fixture.manifest["meshes"][0]["reports"]["scene"]
            )
            fixture.write()
            with self.assertRaisesRegex(self.validator.ManifestError, "duplicated"):
                self.validate(fixture)

    def test_report_output_hash_link_is_required(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            report_record = fixture.manifest["meshes"][0]["reports"]["post_export"]
            payload = json.loads(
                (fixture.reports_root / report_record["path"]).read_text()
            )
            payload["glb_sha256"] = "0" * 64
            fixture.manifest["meshes"][0]["reports"]["post_export"] = write_json_record(
                fixture.reports_root, report_record["path"], payload
            )
            fixture.write()
            with self.assertRaisesRegex(
                self.validator.ManifestError, "post-export hash link"
            ):
                self.validate(fixture)

    def test_texture_report_hash_must_match_production(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            record = fixture.manifest["texture_report"]
            payload = json.loads((fixture.reports_root / record["path"]).read_text())
            payload["textures"]["albedo"]["sha256"] = "0" * 64
            fixture.manifest["texture_report"] = write_json_record(
                fixture.reports_root, record["path"], payload
            )
            fixture.write()
            with self.assertRaisesRegex(
                self.validator.ManifestError, "albedo link differs"
            ):
                self.validate(fixture)

    def test_rejected_normal_report_cannot_retain_normal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.make_final(normal="rejected")
            record = fixture.manifest["texture_report"]
            payload = json.loads((fixture.reports_root / record["path"]).read_text())
            normal_path = fixture.exports_root / self.validator.TEXTURES["normal"]
            payload["textures"]["normal"] = {
                "path": normal_path.name,
                "bytes": normal_path.stat().st_size,
                "sha256": digest(normal_path.read_bytes()),
                "width": 1024,
                "height": 1024,
                "mode": "RGB",
            }
            payload["normal"] = {}
            fixture.manifest["texture_report"] = write_json_record(
                fixture.reports_root, record["path"], payload
            )
            fixture.write()
            with self.assertRaisesRegex(
                self.validator.ManifestError, "identity differs"
            ):
                self.validate(fixture, mode="final")

    def test_path_traversal_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.manifest["production"]["core"][0]["path"] = "../wall.glb"
            fixture.write()
            with self.assertRaisesRegex(self.validator.ManifestError, "escapes"):
                self.validate(fixture)

    def test_unknown_top_level_field_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = AssetSetFixture(Path(directory), self.validator)
            fixture.manifest["extra"] = True
            fixture.write()
            with self.assertRaisesRegex(self.validator.ManifestError, "fields differ"):
                self.validate(fixture)


if __name__ == "__main__":
    unittest.main()
