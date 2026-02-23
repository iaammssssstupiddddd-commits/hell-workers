# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust source code
- `src/entities/`: entity definitions (Soul, Familiar, buildings)
- `src/systems/`: game logic systems (`familiar_ai/`, `soul_ai/`, `jobs/`, `visual/`)
- `src/interface/`: UI components
- `src/plugins/`: Bevy plugin wiring
- `assets/`: sprites, fonts, and other game resources
- `docs/`: technical specs and developer docs (start with `docs/README.md`)
- `proposals/`: feature/refactor proposals
- `scripts/`: utility scripts (image conversion, etc.)

## Tech Stack & Targets
- Engine: Bevy 0.18 (see `Cargo.toml`).
- Language: Rust 2024 edition.
- Build target: use native `cargo run` by default; if you need a Windows GNU build, `cargo build --target x86_64-pc-windows-gnu` is referenced in `CLAUDE.md`.

## Build, Test, and Development Commands
- `cargo run`: build and run the game locally.
- `cargo check`: compile check only; required before reporting work as complete.
- `python scripts/convert_to_png.py "src" "assets/textures/dest.png"`: convert magenta-backed images to transparent PNGs.
- `trunk serve`: serve the web build using `Trunk.toml` (optional; for WASM workflows).

## Coding Style & Naming Conventions
- Follow Rust defaults: 4-space indentation and idiomatic naming (`snake_case` for functions/vars, `UpperCamelCase` for types, `SCREAMING_SNAKE_CASE` for constants).
- Keep systems and components organized by feature area under `src/systems/` and `src/entities/`.
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
- Avoid dead code and `#[allow(dead_code)]` unless currently required.
- Task system conventions: add new `AssignedTask` variants as struct variants and keep task queries aggregated in `TaskQueries` (see `src/systems/soul_ai/execute/task_execution/`).
- Context hygiene: respect `.cursorignore` and `.geminiignore` by avoiding large build artifacts/logs (`target/`, `dist/`, `.trunk/`, `logs/`, `build_*.txt`, `*_output*.txt`) unless explicitly needed.

### Bevy バージョンの厳守とドキュメント確認
- 本プロジェクトは **Bevy 0.18** を使用している。
- AIの学習データにある過去のバージョン（0.14以前など）のAPIを無自覚に使用しないこと。
- 新しい機能やシステムを実装する（特に Window, UI, Query, Commands周りなど）際は、推測でコードを書く前に以下のいずれかを行うこと：
  1. すでに正しく動いている他のプロジェクト内ソースコードの書き方を参考にする
  2. Web検索ツール等で `https://docs.rs/bevy/0.18.0/bevy/` や関連ドキュメントを確認する
  3. ローカルの `~/.cargo/registry/src/` にあるBevyのソースコード（関数のシグネチャ）を検索して直接確認する
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
