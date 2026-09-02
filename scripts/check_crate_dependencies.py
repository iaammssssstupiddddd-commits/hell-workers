#!/usr/bin/env python3
"""Validate the exact internal Cargo dependency graph.

Cargo manifests are the implementation source.  This policy makes intentional
workspace edges reviewable and rejects accidental leaf-to-root edges or cycles.
"""

from __future__ import annotations

import sys
import tomllib
from collections.abc import Mapping
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent

EXPECTED_INTERNAL_DEPENDENCIES: dict[str, set[str]] = {
    "bevy_app": {
        "hw_core",
        "hw_energy",
        "hw_familiar_ai",
        "hw_infra",
        "hw_jobs",
        "hw_logistics",
        "hw_soul_ai",
        "hw_spatial",
        "hw_ui",
        "hw_visual",
        "hw_world",
    },
    "hw_core": set(),
    "hw_energy": set(),
    "hw_familiar_ai": {
        "hw_core",
        "hw_energy",
        "hw_jobs",
        "hw_logistics",
        "hw_spatial",
        "hw_world",
    },
    "hw_infra": {"hw_core"},
    "hw_jobs": {"hw_core", "hw_energy"},
    "hw_logistics": {"hw_core", "hw_jobs", "hw_spatial", "hw_world"},
    "hw_soul_ai": {
        "hw_core",
        "hw_energy",
        "hw_jobs",
        "hw_logistics",
        "hw_spatial",
        "hw_world",
    },
    "hw_spatial": {"hw_core", "hw_jobs", "hw_world"},
    "hw_ui": {"hw_core", "hw_jobs", "hw_logistics"},
    "hw_visual": {"hw_core", "hw_spatial", "hw_world"},
    "hw_world": {"hw_core", "hw_jobs"},
    "visual_test": {"hw_core", "hw_visual", "hw_world"},
}


def load_toml(path: Path) -> dict[str, object]:
    with path.open("rb") as source:
        return tomllib.load(source)


def workspace_manifests(repo_root: Path) -> dict[str, Path]:
    manifests: dict[str, Path] = {}
    for manifest in sorted((repo_root / "crates").glob("*/Cargo.toml")):
        payload = load_toml(manifest)
        package = payload.get("package")
        if not isinstance(package, Mapping):
            raise ValueError(f"missing [package] table: {manifest}")
        name = package.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError(f"missing package.name: {manifest}")
        if name in manifests:
            raise ValueError(f"duplicate package name {name}: {manifest}")
        manifests[name] = manifest.resolve()
    return manifests


def dependency_tables(payload: Mapping[str, object]) -> list[Mapping[str, object]]:
    tables: list[Mapping[str, object]] = []
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = payload.get(section)
        if isinstance(table, Mapping):
            tables.append(table)
    targets = payload.get("target")
    if isinstance(targets, Mapping):
        for target in targets.values():
            if isinstance(target, Mapping):
                tables.extend(dependency_tables(target))
    return tables


def internal_dependency_graph(repo_root: Path) -> dict[str, set[str]]:
    manifests = workspace_manifests(repo_root)
    manifests_by_path = {path: name for name, path in manifests.items()}
    graph = {name: set() for name in manifests}
    for owner, manifest in manifests.items():
        payload = load_toml(manifest)
        for table in dependency_tables(payload):
            for alias, specification in table.items():
                if not isinstance(specification, Mapping):
                    continue
                relative_path = specification.get("path")
                if not isinstance(relative_path, str):
                    continue
                dependency_manifest = (manifest.parent / relative_path / "Cargo.toml").resolve()
                dependency = manifests_by_path.get(dependency_manifest)
                if dependency is None:
                    continue
                declared_package = specification.get("package", alias)
                if declared_package != dependency:
                    raise ValueError(
                        f"{owner}: dependency {alias} declares package "
                        f"{declared_package}, but path owns {dependency}"
                    )
                graph[owner].add(dependency)
    return graph


def find_cycles(graph: Mapping[str, set[str]]) -> list[tuple[str, ...]]:
    cycles: set[tuple[str, ...]] = set()
    visited: set[str] = set()
    active: list[str] = []

    def visit(node: str) -> None:
        if node in active:
            start = active.index(node)
            cycle = tuple(active[start:] + [node])
            cycles.add(cycle)
            return
        if node in visited:
            return
        active.append(node)
        for dependency in sorted(graph.get(node, set())):
            visit(dependency)
        active.pop()
        visited.add(node)

    for node in sorted(graph):
        visit(node)
    return sorted(cycles)


def validate_graph(
    graph: Mapping[str, set[str]],
    expected: Mapping[str, set[str]],
) -> list[str]:
    errors: list[str] = []
    actual_names = set(graph)
    expected_names = set(expected)
    if missing := expected_names - actual_names:
        errors.append(f"missing workspace crates: {', '.join(sorted(missing))}")
    if unexpected := actual_names - expected_names:
        errors.append(f"unreviewed workspace crates: {', '.join(sorted(unexpected))}")

    for owner in sorted(actual_names & expected_names):
        actual_dependencies = graph[owner]
        expected_dependencies = expected[owner]
        if added := actual_dependencies - expected_dependencies:
            errors.append(f"{owner}: unreviewed edges: {', '.join(sorted(added))}")
        if removed := expected_dependencies - actual_dependencies:
            errors.append(f"{owner}: policy has stale edges: {', '.join(sorted(removed))}")
        if owner.startswith("hw_") and "bevy_app" in actual_dependencies:
            errors.append(f"{owner}: leaf crate must not depend on bevy_app")

    for cycle in find_cycles(graph):
        errors.append(f"dependency cycle: {' -> '.join(cycle)}")
    return errors


def main() -> int:
    try:
        graph = internal_dependency_graph(REPO_ROOT)
        errors = validate_graph(graph, EXPECTED_INTERNAL_DEPENDENCIES)
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"Crate dependency check failed: {error}", file=sys.stderr)
        return 1
    if errors:
        print("Crate dependency policy violations:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    edge_count = sum(len(dependencies) for dependencies in graph.values())
    print(f"Crate dependency graph: pass ({len(graph)} crates, {edge_count} internal edges)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
