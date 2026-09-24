# Copilot Instructions for Hell Workers

## Start here

Read `README.md`, `docs/DEVELOPMENT.md`, `docs/README.md`, and the nearest
`_rules.md` (exposed through sibling `AGENTS.md` / `CLAUDE.md` symlinks) before
editing. The project uses Bevy 0.19 and Rust 2024.

## Build and verification

- `python3 scripts/dev.py doctor` — read-only environment diagnosis.
- `python3 scripts/dev.py check` — fast format/policy/workspace compile gate.
- `python3 scripts/dev.py verify` — full local/CI fallback; see the change-aware completion policy below.
- `python3 scripts/dev.py cargo -- run` — native game run through the persistent-storage guard.
- `trunk serve` — optional WASM workflow; not a native/performance acceptance route and not a substitute for the guarded Cargo workflow.

Do not add Clippy suppressions or dead code. Do not hard-code personal
`HOME`, `CARGO_HOME`, project, or tool paths.

## Architecture

- `crates/bevy_app` is the composition root and Bevy adapter shell.
- Domain/model ownership lives in `hw_core`, `hw_world`, `hw_jobs`,
  `hw_logistics`, `hw_spatial`, and `hw_energy`.
- Behavior ownership lives in `hw_familiar_ai` and `hw_soul_ai`.
- Presentation ownership lives in `hw_ui` and `hw_visual`.
- Keep root adapters thin and follow `docs/crate-boundaries.md` for dependency
  direction.
- Familiar AI uses Perceive → Update → Decide → Execute; Soul AI uses
  Perceive → Update → Decide → Execute. Preserve configured system ordering and
  `ApplyDeferred` boundaries.

## Task execution contracts

- Define task payload structs under `crates/hw_jobs/src/tasks/` and add
  `Variant(VariantData)` entries to `crates/hw_jobs/src/tasks/mod.rs`.
- Aggregate Soul execution/assignment/unassignment queries in
  `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs`.
- Aggregate Familiar assignment queries in
  `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs`.
- Use `TaskExecutionContext` completion/abort APIs. Merely setting
  `AssignedTask::None` must not emit `OnTaskCompleted` (I-S3).

## Change discipline

- Decide systems generate requests; Execute systems apply mutations.
- Prefer centralized plugin registration for observers and messages; do not
  double-register handlers.
- Update the affected permanent docs and run
  `python3 scripts/dev.py docs --write` after plan/proposal navigation changes.
- Preserve unrelated worktree changes.
- You MUST use the repository `hell-workers-review-help-impact` Skill after implementing, changing, or removing functionality, code, or runtime data and before reporting completion, committing, or publishing.
- Complete the Skill's `Update required` / `No impact` decision from the actual player-visible path; a passing Help impact gate alone does not count as the review.
- If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-review-help-impact/SKILL.md` directly before completion.
- You MUST use the repository `hell-workers-run-native-acceptance` Skill whenever a task requires real-machine or native acceptance, actual-window, renderer/GPU/backend, or native performance verification, including requests for `実機確認` or `実機テスト`.
- Use the Skill's established no-prompt launcher and fail-closed artifact verification; do not ask the user for repeated display or GUI permissions while that launcher is available.
- If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md` directly.
- Use the primary repository's `python3 scripts/dev.py validation` coordinator for validation planning/execution and pass its storage check before reporting.
- Read the primary `docs/development-infra/validation-storage-workflow.md` on each validation start/resume; keep frozen subjects unchanged and preserve review-active build caches.
- Closed work requires no per-job archive. Retain only concrete active uses and product sources/releases; record the owner and release condition, then dispose of unneeded validation output.

## Supervised Orca development

- Parallel editing is allowed only through the ticketed `scripts/orca_roles.py` launcher in separate worktrees; never delegate edits in a shared checkout.
- Use at most two implementation slots and one fixed read-only reviewer; reuse the same reviewer terminal for the workstream. Workers must not delegate again.
- Fixed providers: worker-a uses Codex, worker-b uses Cursor CLI for simple leaf tasks only, and the reviewer uses Codex. Require complexity rationale and acceptance criteria for worker-b; route shared-contract, save, renderer and infrastructure work to A/coordinator.
- The coordinator owns authoritative primary docs, shared contracts, builds/tests, commits and serial integration. Workers cannot write Git metadata or approve their own changes.
- Bind review to base/head and the exact source fingerprint; any source/index change invalidates approval. Do not integrate without the fixed reviewer's explicit approval and same-subject validation.
- Use guarded project entrypoints for all heavy work. One host-wide heavy slot, one Cargo job and one Rust test thread; busy means defer, never bypass the guard.
- Raw Orca agent buttons/default YOLO launches are not the controlled worker path. Unsupported isolation or missing admission evidence means stop; see the primary docs/development-infra/orca-development.md.

## Change-aware completion and branches

- Before completion, use same-subject CI evidence or `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`; use `python3 scripts/dev.py verify` for full fallback.
- Use a dedicated branch for an independent change; reuse the branch for fixes to the same purpose. Preserve parallel work, and use a PR as the automatic CI entry point; publishing still requires task authorization.
- Accept CI only for the intended base/head/tested SHA and selected groups, with no additional dirty source; record the run URL and scope. CI does not replace Help review, required native acceptance, or primary storage cleanup.
- Unknown scope or unavailable CI requires local verification. Use full mode or `verify` when classification cannot be trusted; never treat a missing diff base as success.
