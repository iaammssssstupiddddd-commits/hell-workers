# Hell Workers Project Rules (Gemini CLI)

This project uses **Bevy 0.18**. Adhere to the following rules and conventions.

## 1. Bevy Version & Documentation
- **Bevy 0.18** is strictly required. Avoid using APIs from older versions (0.14 or earlier).
- Verify API signatures using `docs.rs/bevy/0.18.0` or local source code in `~/.cargo/registry/src/` if unsure.
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
- **Stop & Wait**: Stop and wait for user confirmation after completing a milestone or a significant task. Do not proceed to the next milestone autonomously.

## 4. Engineering Standards
- **Rust-analyzer**: Ensure no compilation errors (red squiggles) or significant warnings before reporting completion.
- **Documentation**: Use the `hell-workers-update-docs` skill to keep documentation, indices, and crate READMEs in sync after any structural or logic changes.
- **Command Execution**: Prefer synchronous execution for short-lived commands (file ops, `cargo check`) to avoid unnecessary polling.

## 5. Build & Development Environment
- **CARGO_HOME Prefix (CRITICAL)**: Always use the following prefix for `cargo` commands to ensure consistency and avoid full rebuilds:
  `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- **Dead Code**: Do not use `#[allow(dead_code)]` for future use. If code is unused, delete it.
- **Plans**: Use `docs/plans/` for implementation plans. These are gitignored working documents. Name them in `kebab-case`.

## 6. Implementation Conventions
- **AssignedTask**: When adding variants to the `AssignedTask` enum, use **struct variants** (not tuple variants). Define structures in `crates/bevy_app/src/systems/soul_ai/execute/task_execution/types.rs`.
- **Query Aggregation**: Aggregate task queries in the `TaskQueries` struct in `crates/bevy_app/src/systems/soul_ai/execute/task_execution/context.rs`.
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
   - **Validate**: Run `cargo check` with the `CARGO_HOME` prefix.
4. **Documentation**: Update all affected docs using `hell-workers-update-docs` before finishing.
