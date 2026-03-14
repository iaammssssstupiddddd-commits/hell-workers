---
name: hell-workers-update-docs
description: Update hell-workers documentation after code changes, crate boundary changes, type moves, new WorkType or TransportRequest additions, task/state changes, or milestone completions. Use when the user says "ドキュメントを更新", "READMEを更新", "docs更新", "実装ドキュメントを更新", asks to sync specs or README files, or after finishing a refactor in this repository.
---

# Hell Workers Update Docs

実装変更に追従して、このリポジトリの仕様書・README・計画書インデックスを同期する。変更箇所だけでなく、関連する索引と crate 境界ドキュメントまで追う。

## Start

1. Read `README.md`, `docs/DEVELOPMENT.md`, and `docs/README.md`.
2. Inspect `git status --short` and `git diff --name-status` to classify the change:
   - code behavior change
   - type or module move
   - crate boundary change
   - doc add/remove/rename
   - milestone or plan status change
3. Read every target document before editing it. Do not update docs from memory.

## Update Map

Use this map to decide the minimum complete set of edits.

| Change | Update |
|:--|:--|
| File added, removed, or renamed under `docs/` | `docs/README.md` |
| Plan or proposal files added, removed, or moved | `python scripts/update_docs_index.py`, then review `docs/plans/README.md` and `docs/proposals/README.md` |
| Type, module, or responsibility moved between root and `crates/hw_*` | `docs/cargo_workspace.md`, `docs/architecture.md`, affected `crates/hw_*/README.md`, affected `src/**/README.md`, affected `_rules.md` |
| Root file reduced to a shell or `pub use` re-export | matching `src/**/README.md` with explicit shell/re-export wording, matching `_rules.md` if local AI rules exist |
| Task lifecycle, assignment, relationship, observer, or unassign behavior changed | `docs/tasks.md`; also `docs/invariants.md` (I-S* / I-T* section) and `docs/soul_ai.md` or `docs/familiar_ai.md` when ownership or behavior changes |
| Logistics, reservation, transport request, hauling, or shared-resource behavior changed | `docs/logistics.md`; `docs/invariants.md` (I-L* section) if a silent-failure trap changes; update `docs/building.md` too when blueprint or construction delivery is affected |
| Building, blueprint, site, room, or placement behavior changed | `docs/building.md`, `docs/room_detection.md`, and any relevant crate or src README |
| `TaskMode`, game state, or world/grid responsibilities changed | `docs/state.md`, `docs/architecture.md`, and related README files |
| Game invariant added, removed, or changed (silent failures, ownership contracts, ECS rules) | `docs/invariants.md` (correct I-* section); also update the `_rules.md` of the affected crate |
| Event added, removed, renamed, or Producer/Consumer changed | `docs/events.md` (correct table row); if a new `EntityEvent` or `Message` is added to `hw_core/src/events.rs`, add it to both notification and request tables as appropriate |
| `_rules.md` created for a new directory | Update `docs/plans/multi-tool-ai-rules-plan.md` milestone checkbox and progress percentage |
| Crate dependency added or removed (`Cargo.toml`) | `docs/cargo_workspace.md`, `docs/crate-boundaries.md`, and the affected crate's `_rules.md` dependency constraint section |

## Apply Project Rules

Follow the repo rules from `docs/DEVELOPMENT.md`.

- Keep `docs/*.md` timeless and specification-focused. Put progress reporting only in `docs/plans/` or `docs/proposals/`.
- Update navigation when documentation structure changes. Broken indexes are a bug.
- Prefer documenting information MCP cannot recover quickly: write/remove ownership, cross-system side effects, ordering requirements, silent failure conditions, and timing caveats.
- Do not mirror obvious struct fields, enum variants, or raw API signatures when rust-analyzer or docsrs already shows them.
- When relationships, WorkTypes, TransportRequests, TaskMode variants, spatial grids, crate responsibilities, or local AI rule ownership change, update the corresponding tables or sections immediately.

## Review Pattern

Use this sequence every time:

1. List changed code and docs.
2. Map each code change to the docs it invalidates.
3. Update index files first if navigation changed.
4. Update the specific spec docs next.
5. Update crate and `src/**/README.md` boundaries last.
6. Update affected `_rules.md` files when local AI rule ownership, crate dependency, or shell/leaf boundaries changed.
   Also update `docs/invariants.md` and `docs/events.md` when game contracts or event ownership changes.
7. Re-read the edited docs for contradictions, stale file paths, removed-file references, and stale crate-boundary claims.

## Verification

- Run `python scripts/update_docs_index.py` if any plan or proposal index could be stale.
- Run `cargo check --workspace` if the same task also changed Rust code and it has not been verified yet.
- Report which files changed and why, not just that docs were updated.
