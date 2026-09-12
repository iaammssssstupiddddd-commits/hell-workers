from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from provision_wall_surface_preview import seal, source_file
from seal_wall_candidate import SealError, sha256
from validate_wall_formwork_manifest import ManifestError


class SurfacePreviewTests(unittest.TestCase):
    def test_sources_reject_changed_bytes_and_path_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "core.bin"
            path.write_bytes(b"test fixture")
            record = {"path": "core.bin", "bytes": path.stat().st_size, "sha256": sha256(path)}
            self.assertEqual(source_file(root, record), path)
            for mutation in ({"bytes": 0}, {"sha256": "0" * 64}, {"path": "../core.bin"}):
                with self.subTest(mutation=mutation), self.assertRaises((SealError, ManifestError)):
                    source_file(root, {**record, **mutation})

    def test_destination_is_isolated_and_exclusive(self):
        with tempfile.TemporaryDirectory() as directory, patch(
            "provision_wall_surface_preview.clean_git_identity", return_value=("a" * 40, "b" * 40)
        ):
            root = Path(directory)
            for destination in (root / "assets", root / "staging/validation/existing"):
                destination.mkdir(parents=True)
                with self.assertRaises(SealError):
                    seal(root, root, 11, destination)


if __name__ == "__main__":
    unittest.main()
