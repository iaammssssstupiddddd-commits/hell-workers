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
- **検証**: 完了報告の前に必ず `python3 scripts/dev.py check` を実行し、Rust-analyzer の警告・エラーをゼロにすること。
- **画像生成**: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) に記載されたマゼンタ背景のワークフローを厳守すること。
- You MUST use the repository `hell-workers-review-help-impact` Skill after implementing, changing, or removing functionality, code, or runtime data and before reporting completion, committing, or publishing.
- Complete the Skill's `Update required` / `No impact` decision from the actual player-visible path; a passing Help impact gate alone does not count as the review.
- If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-review-help-impact/SKILL.md` directly before completion.
- You MUST use the repository `hell-workers-run-native-acceptance` Skill whenever a task requires real-machine or native acceptance, actual-window, renderer/GPU/backend, or native performance verification, including requests for `実機確認` or `実機テスト`.
- Use the Skill's established no-prompt launcher and fail-closed artifact verification; do not ask the user for repeated display or GUI permissions while that launcher is available.
- If the current product does not expose that Skill natively, read and follow `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md` directly.

## ドキュメントの更新
機能の追加や大規模な変更を行った際は、`docs/` 内の関連ドキュメントを最新の状態に更新すること。
