from __future__ import annotations

import hashlib
import importlib.util
import json
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = WORKFLOW_ROOT / "scripts/seal_wall_candidate.py"


def load_sealer():
    spec = importlib.util.spec_from_file_location("seal_wall_candidate", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load seal_wall_candidate.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path: Path, payload: bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return path


def write_json(path: Path, payload: object) -> Path:
    return write(path, json.dumps(payload, sort_keys=True).encode())


class CandidateFixture:
    def __init__(self, root: Path, sealer) -> None:
        self.root = root
        self.asset_root = root / "assets"
        self.repo = root / "repo"
        self.blend_root = self.asset_root / "staging/blend"
        self.exports_root = self.asset_root / "staging/exports"
        self.reports_root = self.asset_root / "staging/reports"
        self.licenses_root = self.asset_root / "licenses"
        self.source_root = self.asset_root / "source"
        write(self.blend_root / "wall-production-v1.blend", b"blend")
        core_texture_records = {}
        png = b"\x89PNG\r\n\x1a\n" + b"\0" * 8 + struct.pack(">II", 1024, 1024)
        for role, relative in sealer.manifest_validator.TEXTURES.items():
            path = write(self.exports_root / relative, png + role.encode())
            core_texture_records[role] = {
                "path": path.name,
                "bytes": path.stat().st_size,
                "sha256": digest(path),
                "width": 1024,
                "height": 1024,
                "mode": "RGB",
            }
        write_json(
            self.reports_root / "wall-production-v1.textures.json",
            {
                "schema_version": 1,
                "status": "pass",
                "asset_set_id": "wall-production-v1",
                "normal_decision": "pending",
                "textures": core_texture_records,
                "emissive": {},
                "normal": {},
                "albedo_mean_rgb": [0.0, 0.0, 0.0],
            },
        )
        contract_hash = sealer.sha256(sealer.manifest_validator.GEOMETRY_CONTRACT)
        for family, (
            collection,
            output_relative,
        ) in sealer.manifest_validator.FAMILIES.items():
            glb = write(self.exports_root / output_relative, f"glb:{family}".encode())
            stem = Path(output_relative).name.removesuffix(".glb")
            write_json(
                self.reports_root / f"{stem}.scene.json",
                {
                    "summary": {"errors": 0},
                    "mesh_count": 1,
                    "selection": {"collection": collection},
                },
            )
            write_json(
                self.reports_root / f"{output_relative}.export.json",
                {
                    "status": "exported",
                    "collection": collection,
                    "sha256": digest(glb),
                    "blender_version": "5.1.1",
                },
            )
            write_json(
                self.reports_root / f"{output_relative}.khronos.json",
                {
                    "validatorVersion": "2.0.0-dev.3.10",
                    "issues": {"numErrors": 0, "numWarnings": 0},
                    "info": {"generator": "Khronos glTF Blender I/O v5.1.19"},
                },
            )
            write_json(
                self.reports_root / f"{stem}.post-export.json",
                {
                    "status": "pass",
                    "family": family,
                    "glb_sha256": digest(glb),
                    "contract_sha256": contract_hash,
                },
            )
        for name in ("reference-board", "rebuild"):
            write_json(
                self.reports_root / f"wall-production-v1.{name}.json",
                {"status": "pass", "asset_set_id": "wall-production-v1"},
            )
        write(self.source_root / "mesh-prompt.txt", b"procedural geometry contract")
        write(self.source_root / "texture-prompt.txt", b"rough vector wall atlas")
        self.provenance = write_json(
            self.source_root / "wall-production-v1.provenance.json",
            {
                "schema_version": 1,
                "asset_set_id": "wall-production-v1",
                "generators": [
                    {
                        "role": "mesh",
                        "service": "procedural-blender",
                        "model": "Blender",
                        "version": "5.1.1",
                        "prompt": "source/mesh-prompt.txt",
                        "references": [],
                    },
                    {
                        "role": "texture",
                        "service": "OpenAI built-in image generation",
                        "model": "imagegen",
                        "version": "2026-09-01",
                        "prompt": "source/texture-prompt.txt",
                        "references": [],
                    },
                ],
            },
        )
        write(self.licenses_root / "wall-production-v1.md", b"license evidence")
        self.license_metadata = write_json(
            self.source_root / "wall-production-v1.license.json",
            {
                "schema_version": 1,
                "asset_set_id": "wall-production-v1",
                "name": "OpenAI Business Terms output ownership",
                "terms_url": "https://openai.com/policies/may-2025-business-terms/",
                "checked_at_utc": "2026-09-01T00:00:00+00:00",
                "file": "wall-production-v1.md",
            },
        )
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        write(self.repo / "subject.txt", b"clean subject")
        subprocess.run(["git", "-C", str(self.repo), "add", "subject.txt"], check=True)
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
            check=True,
        )

    def seal(self, sealer) -> dict[str, object]:
        return sealer.seal_candidate(
            asset_root=self.asset_root,
            repo=self.repo,
            generation=1,
            created_at_utc="2026-09-01T00:01:00+00:00",
            provenance_path=self.provenance,
            license_metadata_path=self.license_metadata,
        )


class SealWallCandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.sealer = load_sealer()

    def test_candidate_is_sealed_and_validated(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = CandidateFixture(Path(directory), self.sealer)
            manifest = fixture.seal(self.sealer)
            output = fixture.reports_root / "wall-production-v1.asset-set.json"
            self.sealer.write_json_atomic(output, manifest)
            report = self.sealer.manifest_validator.validate_manifest(
                output,
                mode="candidate",
                blend_root=fixture.blend_root,
                exports_root=fixture.exports_root,
                reports_root=fixture.reports_root,
                licenses_root=fixture.licenses_root,
                repo=fixture.repo,
            )
            self.assertEqual(report["status"], "pass")
            self.assertEqual(report["core_files"], 8)
            self.assertEqual(report["optional_files"], 1)

    def test_dirty_tool_repository_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = CandidateFixture(Path(directory), self.sealer)
            write(fixture.repo / "untracked.txt", b"dirty")
            with self.assertRaisesRegex(self.sealer.SealError, "repository is dirty"):
                fixture.seal(self.sealer)

    def test_missing_closed_set_report_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = CandidateFixture(Path(directory), self.sealer)
            (fixture.reports_root / "wall_cross.post-export.json").unlink()
            with self.assertRaisesRegex(self.sealer.SealError, "is absent"):
                fixture.seal(self.sealer)


if __name__ == "__main__":
    unittest.main()
