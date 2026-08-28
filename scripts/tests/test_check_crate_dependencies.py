from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import check_crate_dependencies


def write_manifest(root: Path, crate: str, dependencies: tuple[str, ...] = ()) -> None:
    manifest = root / "crates" / crate / "Cargo.toml"
    manifest.parent.mkdir(parents=True)
    dependency_lines = "\n".join(
        f'{dependency} = {{ path = "../{dependency}" }}' for dependency in dependencies
    )
    manifest.write_text(
        f'[package]\nname = "{crate}"\nversion = "0.1.0"\n\n[dependencies]\n'
        f"{dependency_lines}\n",
        encoding="utf-8",
    )


class CrateDependencyPolicyTests(unittest.TestCase):
    def test_discovers_path_dependencies_and_accepts_exact_policy(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_manifest(root, "bevy_app", ("hw_core",))
            write_manifest(root, "hw_core")

            graph = check_crate_dependencies.internal_dependency_graph(root)

        self.assertEqual(graph, {"bevy_app": {"hw_core"}, "hw_core": set()})
        self.assertEqual(
            check_crate_dependencies.validate_graph(graph, graph),
            [],
        )

    def test_rejects_unreviewed_edge_and_cycle(self) -> None:
        graph = {
            "bevy_app": {"hw_core"},
            "hw_core": {"bevy_app"},
        }
        expected = {"bevy_app": {"hw_core"}, "hw_core": set()}

        errors = check_crate_dependencies.validate_graph(graph, expected)

        self.assertIn("hw_core: unreviewed edges: bevy_app", errors)
        self.assertIn("dependency cycle: bevy_app -> hw_core -> bevy_app", errors)

    def test_rejects_stale_policy_and_new_crate(self) -> None:
        graph = {"hw_core": set(), "hw_new": set()}
        expected = {"hw_core": {"hw_jobs"}}

        errors = check_crate_dependencies.validate_graph(graph, expected)

        self.assertIn("unreviewed workspace crates: hw_new", errors)
        self.assertIn("hw_core: policy has stale edges: hw_jobs", errors)


if __name__ == "__main__":
    unittest.main()
