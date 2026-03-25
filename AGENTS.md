# Repository Guidelines

## Project Structure & Module Organization
- `crates/bevy_app/src/`: Rust source code
- `crates/bevy_app/src/entities/`: entity definitions (Soul, Familiar, buildings)
- `crates/bevy_app/src/systems/`: game logic systems (`familiar_ai/`, `soul_ai/`, `jobs/`, `visual/`)
- `crates/bevy_app/src/interface/`: UI components
- `crates/bevy_app/src/plugins/`: Bevy plugin wiring
- `assets/`: sprites, fonts, and other game resources
- `docs/`: technical specs and developer docs (start with `docs/README.md`)
- `proposals/`: feature/refactor proposals
- `scripts/`: utility scripts (image conversion, etc.)

## Tech Stack & Targets
- Engine: Bevy 0.18 (see `Cargo.toml`).
- Language: Rust 2024 edition.
- Build target: use native `cargo run` by default.

## Build, Test, and Development Commands
- `cargo run`: build and run the game locally.
- `cargo check`: compile check only; required before reporting work as complete.
- `python scripts/convert_to_png.py "src" "assets/textures/dest.png"`: convert magenta-backed images to transparent PNGs.
- `trunk serve`: serve the web build using `Trunk.toml` (optional; for WASM workflows).

## Coding Style & Naming Conventions
- Follow Rust defaults: 4-space indentation and idiomatic naming (`snake_case` for functions/vars, `UpperCamelCase` for types, `SCREAMING_SNAKE_CASE` for constants).
- Keep systems and components organized by feature area under `crates/bevy_app/src/systems/` and `crates/bevy_app/src/entities/`.
- Avoid dead code and `#[allow(dead_code)]` unless actively justified by current usage.

## Testing Guidelines
- There is no dedicated test suite yet; favor `cargo check` as the baseline verification step.
- If adding tests, use standard Rust naming (`mod tests` and `*_test` functions) and note how to run them in the PR.

## Commit & Pull Request Guidelines
- Commit messages follow a simple conventional style (examples seen: `feat: ...`, `refactor: ...`). Keep summaries short; Japanese or English is acceptable.
- PRs should include a concise description, the testing/verification command(s) run (e.g., `cargo check`), and screenshots or clips for UI/visual changes.

## AI/Agent-Specific Instructions
- Before starting work, skim `README.md`, `docs/DEVELOPMENT.md`, and `docs/README.md` for current rules and specs.
- Keep `cargo check` green; do not report completion with Rust-analyzer errors.
- Avoid dead code and `#[allow(dead_code)]` unless currently required. Do not leave implementations not documented in `docs/`.
- Task system conventions: add new `AssignedTask` variants as struct variants and keep task queries aggregated in `TaskQueries` (see `crates/bevy_app/src/systems/soul_ai/execute/task_execution/`).
- Context hygiene: respect `.cursorignore` and `.geminiignore` by avoiding large build artifacts/logs (`target/`, `dist/`, `.trunk/`, `logs/`, `build_*.txt`, `*_output*.txt`) unless explicitly needed.

### Background Agent Policy (STRICT — DO NOT VIOLATE)
**Do NOT use background/subprocess agents for code editing tasks** (e.g., `general-purpose` agent with file edits).

Reasons:
- Agents share the same repository and routinely make out-of-scope changes to unrelated files
- Progress cannot be monitored in real time; damage is discovered only after the fact
- Multiple agents running in parallel conflict with each other and with other ongoing sessions

**Permitted agent uses:**
- `explore` agent — read-only codebase investigation only
- `code-review` agent — read-only review only
- All code edits must be made directly by the main agent using `view` / `edit` tools

### Git Revert Policy
**NEVER run `git checkout -- <file>` or any destructive git command without first:**
1. Running `git log --oneline -5` to understand recent commit history
2. Running `git diff HEAD -- <file>` to read and understand every line being discarded
3. Confirming that NO parallel task or session produced those changes

Parallel sessions may have legitimately modified files that appear "unexpected" to a new agent.

### Task Lifecycle
**On task start**: Review `docs/` to understand current specs and implementation status.
**On task completion**: Update or create documentation in `docs/` as needed.

### Planning Workflow

#### When to Create a Plan
Create an implementation plan in `docs/plans/` when:
- The task involves significant optimization or refactoring
- Multiple files or systems will be modified
- The implementation approach requires analysis and evaluation
- The user explicitly requests a plan

#### Plan File Management
- **Location**: `docs/plans/` (gitignored - working documents only)
- **Naming**: Use descriptive kebab-case names (e.g., `blueprint-spatial-grid.md`, `taskarea-optimization.md`)
- **Format**: Markdown with clear sections:
  - Problem description
  - Solution approach
  - Expected performance impact
  - Implementation steps
  - Files to modify
  - Verification methods

#### Plan Lifecycle
1. **Creation**: Write detailed plan before implementation
2. **Implementation**: Follow plan steps, updating as needed
3. **Completion**:
   - If successful: Delete plan file or move to archive
   - If relevant for future: Document in `docs/architecture.md` or system-specific docs
   - Plans are temporary working documents, not permanent documentation

#### Why Plans are Gitignored
- Plans are AI working documents for organizing complex tasks
- Completed features should be documented in permanent docs (`docs/*.md`)
- Prevents clutter in version control
- User can manually commit specific plans if needed

### Bevy バージョンの厳守とドキュメント確認
- 本プロジェクトは **Bevy 0.18** を使用している。
- AIの学習データにある過去のバージョン（0.14以前など）のAPIを無自覚に使用しないこと。
- 新しい機能やシステムを実装する（特に Window, UI, Query, Commands周りなど）際は、推測でコードを書く前に以下のいずれかを行うこと：
  1. すでに正しく動いている他のプロジェクト内ソースコードの書き方を参考にする
  2. Web検索ツール等で `https://docs.rs/bevy/0.18.0/bevy/` や関連ドキュメントを確認する
  3. ローカルの `/home/satotakumi/.cargo/registry/src/` にあるBevyのソースコード（関数のシグネチャ）を検索して直接確認する
- 実装後は `cargo check` を実行し、APIの変更によるエラー（メソッドが存在しない等）がないか必ず確認すること。

### MCP ツール運用フロー（rust-analyzer-mcp / docsrs-mcp）
- ローカルコード解析（定義ジャンプ、参照、型確認）は `rust-analyzer-mcp` を優先する。
- 外部 crate API の仕様確認は `docsrs-mcp` を優先し、推測で実装しない。
- Bevy API は必ず 0.18 系の情報で確認する（`docsrs-mcp` / `~/.cargo/registry/src/`）。
- 実装後は rust-analyzer 診断確認に加えて `cargo check` を実行する。
- MCP が利用できない場合は、`~/.cargo/registry/src/` と `docs.rs` の一次情報を使って代替確認する。

## Assets & Configuration Tips
- For generated icons or sprites, create with magenta background (`#FF00FF`) and convert via `scripts/convert_to_png.py`.
- If Windows linking fails with too many symbols, disable `dynamic_linking` in `Cargo.toml` as documented in `docs/DEVELOPMENT.md`.
