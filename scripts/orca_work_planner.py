"""Validate a bounded dependency DAG and select at most two supervised Orca lanes."""

from __future__ import annotations

import hashlib
import json
from pathlib import PurePosixPath


COMPLEX_KINDS = {"shared-contract", "save", "renderer", "infrastructure", "cross-crate"}


def relative_paths(values: object, label: str) -> list[str]:
    if not isinstance(values, list) or len(values) > 64:
        raise ValueError(f"invalid {label}")
    result = []
    for value in values:
        path = PurePosixPath(value) if isinstance(value, str) else PurePosixPath("/")
        if path.is_absolute() or not path.parts or any(part in {".", "..", ".git", "target"} for part in path.parts):
            raise ValueError(f"unsafe {label}")
        result.append(str(path))
    return sorted(set(result))


def overlaps(left: str, right: str) -> bool:
    a, b = PurePosixPath(left), PurePosixPath(right)
    return a == b or a in b.parents or b in a.parents


def validate_task(task: dict) -> dict:
    required = {"id", "dependsOn", "readPaths", "writePaths", "contracts", "complexity",
                "complexityReason", "taskKind", "acceptance"}
    if not isinstance(task, dict) or set(task) != required:
        raise ValueError("planner task fields are incomplete")
    if (not isinstance(task["id"], str) or not task["id"]
            or task["complexity"] not in {"simple", "complex"}
            or not isinstance(task["complexityReason"], str) or not task["complexityReason"].strip()
            or not isinstance(task["taskKind"], str) or not task["taskKind"].strip()
            or not isinstance(task["acceptance"], str) or not task["acceptance"].strip()
            or not isinstance(task["dependsOn"], list)
            or not all(isinstance(item, str) and item for item in task["dependsOn"])
            or not isinstance(task["contracts"], list)
            or not all(isinstance(item, str) and item for item in task["contracts"])):
        raise ValueError("invalid planner task")
    result = dict(task)
    result["readPaths"] = relative_paths(task["readPaths"], "read paths")
    result["writePaths"] = relative_paths(task["writePaths"], "write paths")
    result["contracts"] = sorted(set(task["contracts"]))
    if not result["writePaths"]:
        raise ValueError("editing task requires a bounded write set")
    return result


def plan(spec: dict) -> dict:
    if (not isinstance(spec, dict) or set(spec) != {"schema", "objective", "tasks"}
            or spec.get("schema") != 1 or not isinstance(spec.get("objective"), str)
            or not spec["objective"].strip() or not isinstance(spec.get("tasks"), list)
            or not 1 <= len(spec["tasks"]) <= 32):
        raise ValueError("invalid planner specification")
    tasks = [validate_task(item) for item in spec["tasks"]]
    by_id = {item["id"]: item for item in tasks}
    if len(by_id) != len(tasks):
        raise ValueError("planner task identities must be unique")
    if any(dep not in by_id or dep == item["id"] for item in tasks for dep in item["dependsOn"]):
        raise ValueError("planner dependency is unknown or self-referential")
    waves: list[list[str]] = []
    remaining = set(by_id)
    completed: set[str] = set()
    while remaining:
        wave = sorted(item for item in remaining if set(by_id[item]["dependsOn"]) <= completed)
        if not wave:
            raise ValueError("planner dependency graph contains a cycle")
        waves.append(wave)
        remaining.difference_update(wave)
        completed.update(wave)
    assignments: dict[str, dict] = {}
    for wave_index, wave in enumerate(waves):
        if len(wave) > 2:
            raise ValueError("one generation supports at most two ready tasks")
        nodes = [by_id[item] for item in wave]
        for index, left in enumerate(nodes):
            for right in nodes[index + 1:]:
                if (any(overlaps(a, b) for a in left["writePaths"] for b in right["writePaths"])
                        or any(overlaps(a, b) for a in left["writePaths"] for b in right["readPaths"])
                        or any(overlaps(a, b) for a in right["writePaths"] for b in left["readPaths"])
                        or set(left["contracts"]) & set(right["contracts"])):
                    raise ValueError("parallel tasks share a path or contract; serialize them")
        ordered = sorted(nodes, key=lambda item: (
            item["complexity"] == "simple" and item["taskKind"] not in COMPLEX_KINDS,
            item["id"],
        ))
        for index, task in enumerate(ordered):
            b_eligible = (task["complexity"] == "simple"
                          and task["taskKind"] not in COMPLEX_KINDS
                          and not task["contracts"] and len(task["writePaths"]) == 1
                          and not task["dependsOn"])
            slot = "worker-b" if len(nodes) == 2 and index == 1 and b_eligible else "worker-a"
            if slot == "worker-a" and any(value["slot"] == "worker-a" and value["wave"] == wave_index
                                          for value in assignments.values()):
                raise ValueError("second parallel task is not eligible for Cursor worker-b")
            assignments[task["id"]] = {
                "slot": slot, "wave": wave_index,
                "bEligible": b_eligible,
                "bReason": ("単純leaf・単一write set・共有契約なし"
                            if b_eligible else "複雑度、依存、write set、共有契約のいずれかがB条件外"),
            }
    material = {"schema": 1, "objective": spec["objective"], "tasks": tasks,
                "waves": waves, "assignments": assignments}
    material["sha256"] = hashlib.sha256(json.dumps(
        material, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()).hexdigest()
    return material


def context_package(plan: dict, task_id: str, *, base: str, specification: list[str],
                    decisions: list[str], forbidden: list[str], generation: int) -> dict:
    if task_id not in plan.get("assignments", {}) or task_id not in {item["id"] for item in plan.get("tasks", [])}:
        raise ValueError("context package task is not in the reviewed plan")
    if type(generation) is not int or generation < 1:
        raise ValueError("context package generation is invalid")
    task = next(item for item in plan["tasks"] if item["id"] == task_id)
    package = {"schema": 1, "planSha256": plan["sha256"], "taskId": task_id,
               "generation": generation, "base": base, "objective": plan["objective"],
               "specification": specification, "decisions": decisions,
               "allowedPaths": task["writePaths"], "forbidden": forbidden,
               "acceptance": task["acceptance"], "dependsOn": task["dependsOn"],
               "questionRoute": "coordinator"}
    package["sha256"] = hashlib.sha256(json.dumps(
        package, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()).hexdigest()
    return package
