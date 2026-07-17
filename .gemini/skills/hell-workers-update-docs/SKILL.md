---
name: hell-workers-update-docs
description: Update project documentation, READMEs, and indices after code changes (e.g., crate migrations, type moves, milestone completions). Use when the user requests a documentation sync, "ドキュメントを更新", "READMEを更新", or after finishing a refactoring task.
---

# Hell Workers Update Docs

実装変更に追従して、仕様書、README、crate境界、現行計画と索引を同期する。
ツールから推測できる型一覧の転記より、所有権、順序、副作用、silent failure条件を優先する。

## Start

1. Read `README.md`, `docs/DEVELOPMENT.md`, and `docs/README.md`.
2. Inspect `git status --short` and `git diff --name-status`.
3. Classify changes as behavior, type/module move, crate boundary, navigation, or plan status.
4. Read every target document before editing it; do not update from memory.

## Update Map

| Change | Update |
|:--|:--|
| Document added, removed, or moved under `docs/` | `docs/README.md`; for plan/proposal navigation run `python3 scripts/dev.py docs --write` |
| Type/module/responsibility moved between `bevy_app` and `hw_*` | `docs/cargo_workspace.md`, `docs/architecture.md`, affected crate/root README, affected `_rules.md` |
| Task lifecycle, assignment, completion, abort, or relationship behavior | `docs/tasks.md`, `docs/invariants.md`, and affected Soul/Familiar docs |
| Logistics, reservation, hauling, or shared-resource behavior | `docs/logistics.md`, relevant I-L invariant, and `docs/building.md` when construction delivery changes |
| Building, blueprint, site, room, or placement behavior | `docs/building.md`, `docs/room_detection.md`, and affected README |
| State, world/grid, or system ordering | `docs/state.md`, `docs/architecture.md`, and affected README |
| Event/message producer, consumer, or timing | `docs/events.md` and the defining crate/root adapter docs |
| `_rules.md` added or changed | Verify sibling `AGENTS.md` / `CLAUDE.md` symlinks, update only the active associated plan, then run `python3 scripts/check_agent_rules.py` |
| Crate dependency added or removed | `docs/cargo_workspace.md`, `docs/crate-boundaries.md`, affected crate README/rules |

## Ownership References

- Task model and payloads: `crates/hw_jobs/src/tasks/`.
- Soul task context and queries: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/`.
- Familiar task assignment queries: `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs`.
- Root `crates/bevy_app` files are composition/adapters unless current code proves otherwise.
- Game invariants are canonical in `docs/invariants.md`; local `_rules.md` must not contradict them.

## Plan Lifecycle

- Current plans under `docs/plans/` are tracked; only configured archive/rejected paths are ignored.
- Create plans from `docs/plans/plan-template.md` and update metadata/milestones while active.
- On completion, sync durable behavior into `docs/*.md`, then delete or archive the temporary plan and regenerate indexes.
- Never update an archived plan as if it were the current source of truth.

## Review and Verification

1. Re-read edited docs for contradictions, stale paths, and removed-file references.
2. Run `python3 scripts/dev.py docs --write` after plan/proposal navigation changes.
3. Run `python3 scripts/check_agent_rules.py` after rule or skill changes.
4. Run `python3 scripts/dev.py check` when Rust code changed and has not been verified.
5. Before broad completion, run `python3 scripts/dev.py verify`.
6. Report the changed files and the contract each update keeps in sync.
