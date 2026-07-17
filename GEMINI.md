# Hell Workers Project Rules (Gemini CLI)

This project uses **Bevy 0.19**. Adhere to the following rules and conventions.

## 1. Bevy Version & Documentation
- **Bevy 0.19** is strictly required. Avoid using APIs from older versions (0.14 or earlier).
- Verify API signatures using `docs.rs/bevy/0.19.0` or local source code in `~/.cargo/registry/src/` if unsure.
- Always run `cargo check` after implementation to verify API compatibility.

## 2. Crate Boundaries & Architecture
- Follow `docs/crate-boundaries.md` for all structural changes.
- **Leaf Crates (`hw_*`)**: Pure domain logic, data models, and systems. Can depend on Bevy types.
- **Root Crate (`bevy_app`)**: App shell, wiring, asset management, and UI injection.
- **Reverse Dependency**: NEVER import from `bevy_app` into `hw_*` crates.
- **Communication**: Use the Pub/Sub pattern via events in `hw_core` for cross-domain interaction.

## 3. Interaction Protocol
- **Inquiry vs. Directive**: 
  - **Inquiry**: Requests for analysis, review, or proposals. **DO NOT** modify files. Use only read tools.
  - **Directive**: Explicit instructions to implement, fix, or commit. File modifications are permitted.
- **Directive scope**: An explicit implementation directive covers the requested outcome, including its milestones and verification. Pause only for an explicit checkpoint, new authority/material choice, or a genuine blocker.

## 4. Engineering Standards
- **Rust-analyzer**: Ensure no compilation errors (red squiggles) or significant warnings before reporting completion.
- **Documentation**: Use the `hell-workers-update-docs` skill to keep documentation, indices, and crate READMEs in sync after any structural or logic changes.
- **Command Execution**: Prefer synchronous execution for short-lived commands (file ops, `cargo check`) to avoid unnecessary polling.

## 5. Build & Development Environment
- **Portable environment**: Do not hard-code personal `HOME` / `CARGO_HOME` paths. Run `python3 scripts/dev.py doctor` once and use `python3 scripts/dev.py check` during development.
- **Dead Code**: Do not use `#[allow(dead_code)]` for future use. If code is unused, delete it.
- **Plans**: Copy `docs/plans/plan-template.md` to `<topic>-plan-YYYY-MM-DD.md`; active plans are tracked. Run `python3 scripts/dev.py docs --write` after plan/proposal changes.

## 6. Implementation Conventions
- **AssignedTask**: Define payload structs under `crates/hw_jobs/src/tasks/` and add `Variant(VariantData)` entries to `crates/hw_jobs/src/tasks/mod.rs`.
- **Query Aggregation**: Aggregate Soul execution queries in `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs` and Familiar assignment queries in `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs`.
- **Execution Context**: Access data through `TaskExecutionContext` to keep system arguments minimal.

## 7. Image Generation Workflow
1. **Generate**: Create images with a **solid pure magenta background (#FF00FF)**. Do NOT use transparency during generation.
2. **Convert**: Use `python scripts/convert_to_png.py "source" "assets/textures/dest.png"` to convert to transparent PNG.
3. **Verify**: Verify PNG signature: `head -c 8 [path] | od -An -t x1` (Expected: `89 50 4e 47 0d 0a 1a 0a`).

## 8. Development Workflow
1. **Research**: Map the codebase and validate assumptions.
2. **Strategy**: Formulate a plan and share a concise summary.
3. **Execution**: Iterate through Plan -> Act -> Validate.
   - **Act**: Surgical, idiomatic updates.
   - **Validate**: Run `python3 scripts/dev.py check` during implementation and `python3 scripts/dev.py verify` before broad completion.
4. **Documentation**: Update all affected docs using `hell-workers-update-docs` before finishing.
