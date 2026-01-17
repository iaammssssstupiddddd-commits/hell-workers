# Project Rules for Antigravity

このプロジェクト "Hell Workers" で作業を開始する際、以下の情報を最初に読み込み、文脈を把握してください。

## 必須参照ドキュメント
指示やタスクの背景を理解するために、まず以下のファイルを確認すること：
1.  **プロジェクト全体像**: [README.md](file:///f:/DevData\projects\hell-workers\README.md)
2.  **開発ガイドライン**: [docs/DEVELOPMENT.md](file:///f:/DevData\projects\hell-workers\docs\DEVELOPMENT.md)
3.  **ドキュメント目次**: [docs/README.md](file:///f:/DevData\projects\hell-workers\docs\README.md)
4.  **アーキテクチャ詳細**: [docs/architecture.md](file:///f:/DevData\projects\hell-workers\docs\architecture.md)

## 技術的制約・ルール
- **エンジン**: Bevy 0.17 を使用。
- **ECS Relationships**: エンティティ間の参照には必ず Relationship を使用する。
- **検証**: 完了報告の前に必ず `cargo check` を実行し、Rust-analyzer の警告・エラーをゼロにすること。
- **画像生成**: [docs/DEVELOPMENT.md](file:///f:/DevData\projects\hell-workers\docs\DEVELOPMENT.md) に記載されたマゼンタ背景のワークフローを厳守すること。

## ドキュメントの更新
機能の追加や大規模な変更を行った際は、`docs/` 内の関連ドキュメントを最新の状態に更新すること。
