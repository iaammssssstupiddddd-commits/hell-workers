# Project Rules for Antigravity

このプロジェクト "Hell Workers" で作業を開始する際、以下の情報を最初に読み込み、文脈を把握してください。

## 必須参照ドキュメント
指示やタスクの背景を理解するために、まず以下のファイルを確認すること：
1.  **プロジェクト全体像**: [README.md](README.md)
2.  **開発ガイドライン**: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
3.  **ドキュメント目次**: [docs/README.md](docs/README.md)
4.  **アーキテクチャ詳細**: [docs/architecture.md](docs/architecture.md)

## 技術的制約・ルール
- **エンジン**: Bevy 0.19 を使用。
- **ECS Relationships**: エンティティ間の参照には必ず Relationship を使用する。
- **検証**: 完了判定は末尾の変更別CIルールに従う。Rust変更時は `python3 scripts/dev.py check` とRust-analyzer診断を確認する。
- **画像生成**: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) に記載されたマゼンタ背景のワークフローを厳守すること。
- You MUST use the repository `hell-workers-review-help-impact` Skill after implementing, changing, or removing functionality, code, or runtime data and before reporting completion, committing, or publishing.
- Complete the Skill's `Update required` / `No impact` decision from the actual player-visible path; a passing Help impact gate alone does not count as the review.
- If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-review-help-impact/SKILL.md` directly before completion.
- You MUST use the repository `hell-workers-run-native-acceptance` Skill whenever a task requires real-machine or native acceptance, actual-window, renderer/GPU/backend, or native performance verification, including requests for `実機確認` or `実機テスト`.
- Use the Skill's established no-prompt launcher and fail-closed artifact verification; do not ask the user for repeated display or GUI permissions while that launcher is available.
- If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md` directly.
- Use the primary repository's `python3 scripts/dev.py validation` coordinator for validation planning/execution and pass its storage check before reporting.
- Read the primary `docs/development-infra/validation-storage-workflow.md` on each validation start/resume; keep frozen subjects unchanged and preserve review-active build caches.
- Closed work requires no per-job archive. Retain only concrete active uses and product sources/releases; record the owner and release condition, then dispose of unneeded validation output.

## ドキュメントの更新
機能の追加や大規模な変更を行った際は、`docs/` 内の関連ドキュメントを最新の状態に更新すること。

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
