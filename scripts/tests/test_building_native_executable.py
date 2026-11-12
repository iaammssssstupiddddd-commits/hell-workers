"""Executable-input lifetime guards; synthetic files never grant runtime evidence."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_native_executable as executable


class ExecutableSnapshotTests(unittest.TestCase):
    def setup_paths(self, directory):
        base = Path(directory).resolve()
        source = base / "shared-bevy_app"
        source.write_bytes(b"capture executable")
        source.chmod(0o755)
        root = base / "job"
        root.mkdir()
        return source, root

    def test_memory_rebuild_does_not_replace_the_formal_capture_input(self):
        with tempfile.TemporaryDirectory() as directory:
            source, root = self.setup_paths(directory)
            snapshot = executable.freeze(source, root)
            expected = executable.digest(snapshot)
            source.write_bytes(b"memory executable")
            executable.validate(snapshot, root, expected)
            self.assertEqual(snapshot.read_bytes(), b"capture executable")
            self.assertNotEqual(executable.digest(source), expected)
            self.assertTrue(os.access(snapshot, os.X_OK))
            self.assertEqual(snapshot.stat().st_mode & 0o222, 0)

    def test_existing_destination_is_never_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            source, root = self.setup_paths(directory)
            existing = executable.freeze(source, root)
            with self.assertRaises(FileExistsError):
                executable.freeze(source, root)
            self.assertEqual(existing.read_bytes(), b"capture executable")

    def test_source_change_during_copy_is_retained_as_a_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            source, root = self.setup_paths(directory)
            original = executable.shutil.copyfileobj
            def mutate(input_file, output_file, **options):
                original(input_file, output_file, **options)
                source.write_bytes(b"concurrent replacement")
            with patch.object(executable.shutil, "copyfileobj", side_effect=mutate):
                with self.assertRaisesRegex(ValueError, "changed while"):
                    executable.freeze(source, root)
            self.assertTrue(executable.snapshot_path(root).exists())

    def test_symlink_and_foreign_paths_or_changed_bytes_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source, root = self.setup_paths(directory)
            alias = source.with_name("alias")
            alias.symlink_to(source)
            with self.assertRaises(ValueError):
                executable.freeze(alias, root)
            snapshot = executable.freeze(source, root)
            expected = executable.digest(snapshot)
            with self.assertRaises(ValueError):
                executable.validate(source, root, expected)
            with self.assertRaises(ValueError):
                executable.validate(snapshot, root, "0" * 64)
            snapshot.chmod(0o755)
            with self.assertRaises(ValueError):
                executable.validate(snapshot, root, expected)
            snapshot.write_bytes(b"tampered")
            with self.assertRaises(ValueError):
                executable.validate(snapshot, root, expected)
            snapshot.unlink()
            snapshot.symlink_to(source)
            with self.assertRaises(ValueError):
                executable.validate(snapshot, root, executable.digest(source))

    def test_formal_art_rejects_foreign_executable_before_codec_inspection(self):
        from scripts import building_art_acceptance as art
        with tempfile.TemporaryDirectory() as directory:
            source, root = self.setup_paths(directory)
            for snapshot in (False, True):
                value = {"mode": "candidate", "repo": str(Path(directory).resolve())}
                if snapshot:
                    value["executable_snapshot"] = {"contract": executable.CONTRACT, "verifier_sha256": executable.verifier_sha256()}
                manifest = {"plan": value, "binary": {"path": str(source), "sha256": executable.digest(source)}}
                (root / "manifest.json").write_bytes(art.pipeline.canonical(manifest))
                with patch.object(art, "check_plan") as codec:
                    with self.assertRaises(ValueError):
                        art.verify(root)
                    codec.assert_not_called()

    def test_snapshot_root_is_absolute_canonical_and_existing(self):
        with tempfile.TemporaryDirectory() as directory:
            source, root = self.setup_paths(directory)
            alias = root.with_name("job-link")
            alias.symlink_to(root, target_is_directory=True)
            for invalid in (Path("relative"), root / "missing", alias):
                with self.assertRaises(ValueError):
                    executable.freeze(source, invalid)


if __name__ == "__main__":
    unittest.main()
