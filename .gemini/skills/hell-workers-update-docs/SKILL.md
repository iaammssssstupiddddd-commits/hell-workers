---
name: hell-workers-update-docs
description: Update project documentation, READMEs, and indices after code changes (e.g., crate migrations, type moves, milestone completions). Use when the user requests a documentation sync, "ドキュメントを更新", "READMEを更新", or after finishing a refactoring task.
---

# Hell Workers Update Docs

Sync implementation specs, READMEs, and plan/proposal indices with the latest code state.

## 1. Classify Changes
Run `git status --short` and `git diff --name-status` to identify:
- **Structural**: Type/module moves between root and `crates/hw_*`.
- **Behavioral**: Changes in tasks, logistics, building, or invariants.
- **Organizational**: Milestone completions or added/removed plans.

## 2. Update Map
| Category | Files to Update |
|:--|:--|
| **Crate/Root Boundaries** | `docs/cargo_workspace.md`, `docs/architecture.md`, `crates/hw_*/README.md`, `src/**/README.md`, `_rules.md` |
| **Logistics/Building** | `docs/logistics.md`, `docs/building.md`, `docs/room_detection.md`, `docs/invariants.md` |
| **Tasks/AI** | `docs/tasks.md`, `docs/soul_ai.md`, `docs/familiar_ai.md`, `docs/invariants.md` |
| **Events/State** | `docs/events.md`, `docs/state.md` |
| **Indices** | `docs/README.md`, `docs/plans/README.md`, `docs/proposals/README.md` |
| **Plans** | `docs/plans/[plan-name].md` (check milestones) |

## 3. Execution Steps
1. **Index Sync**: Run `python scripts/update_docs_index.py` if plan/proposal files changed.
2. **Read Before Edit**: Always read the target document before editing. Do not update from memory.
3. **Boundary Tables**: Update "主要モジュール表" (module table) and "境界表" (boundary table) in READMEs.
4. **Shell Files**: If a file became a `pub use` shell, update its description in the parent directory's README.
5. **Invariants**: Update `docs/invariants.md` if game rules or silent-failure conditions changed.
6. **Milestones**: Mark completed milestones in `docs/plans/*.md`.

## 4. Verification
- Run `cargo check --workspace` if code was also changed.
- Verify indices are updated if navigation changed.
- Report a summary table of updated files.
