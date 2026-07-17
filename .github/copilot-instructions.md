# Copilot Instructions for Hell Workers

## Start here

Read `README.md`, `docs/DEVELOPMENT.md`, `docs/README.md`, and the nearest
`_rules.md` (exposed through sibling `AGENTS.md` / `CLAUDE.md` symlinks) before
editing. The project uses Bevy 0.19 and Rust 2024.

## Build and verification

- `python3 scripts/dev.py doctor` — read-only environment diagnosis.
- `python3 scripts/dev.py check` — fast format/policy/workspace compile gate.
- `python3 scripts/dev.py verify` — full local/CI gate; run before completion.
- `cargo run` — native game run.
- `trunk serve` — optional WASM workflow.

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
