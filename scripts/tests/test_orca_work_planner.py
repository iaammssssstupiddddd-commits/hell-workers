from __future__ import annotations

import unittest

from scripts import orca_work_planner as planner


def task(identity: str, *, complexity: str = "complex", kind: str = "feature",
         reads=None, writes=None, contracts=None, depends=None) -> dict:
    return {"id": identity, "dependsOn": depends or [], "readPaths": reads or [],
            "writePaths": writes or [f"scripts/{identity}"], "contracts": contracts or [],
            "complexity": complexity, "complexityReason": "bounded fixture",
            "taskKind": kind, "acceptance": "exact fixture output"}


class WorkPlannerTests(unittest.TestCase):
    def test_independent_simple_leaf_uses_cursor_b(self) -> None:
        result = planner.plan({"schema": 1, "objective": "two changes", "tasks": [
            task("core", writes=["crates/core"]),
            task("fixture", complexity="simple", kind="fixture", writes=["scripts/fixtures/x"]),
        ]})
        self.assertEqual(result["assignments"]["core"]["slot"], "worker-a")
        self.assertEqual(result["assignments"]["fixture"]["slot"], "worker-b")

    def test_shared_contract_or_read_write_conflict_refuses_parallel(self) -> None:
        cases = [
            [task("a", contracts=["SaveV2"]), task("b", complexity="simple", contracts=["SaveV2"])],
            [task("a", writes=["crates/api"]), task("b", complexity="simple", reads=["crates/api/mod.rs"])],
        ]
        for tasks in cases:
            with self.subTest(tasks=tasks), self.assertRaisesRegex(ValueError, "serialize"):
                planner.plan({"schema": 1, "objective": "conflict", "tasks": tasks})

    def test_cycle_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "cycle"):
            planner.plan({"schema": 1, "objective": "cycle", "tasks": [
                task("a", depends=["b"]), task("b", depends=["a"]),
            ]})

    def test_context_is_generation_and_plan_bound(self) -> None:
        plan = planner.plan({"schema": 1, "objective": "one", "tasks": [task("a")]})
        package = planner.context_package(
            plan, "a", base="a" * 40, specification=["docs/spec.md"], decisions=["use v2"],
            forbidden=["commit", "push"], generation=2,
        )
        self.assertEqual(package["planSha256"], plan["sha256"])
        self.assertEqual(package["questionRoute"], "coordinator")
        self.assertEqual(package["generation"], 2)


if __name__ == "__main__":
    unittest.main()
