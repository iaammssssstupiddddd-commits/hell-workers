# hw_familiar_ai — Familiar AI ロジック

## 役割

Familiar（使い魔）の AI を実装するクレート。
Perceive / Decide / Execute の各フェーズでの状態遷移・タスク探索・Soul 管理を担う。
**ゲームエンティティへの ECS クエリを含む**が、UI や root 固有型（GameAssets 等）には依存しない。

## 主要モジュール

| ディレクトリ/ファイル | 内容 |
|---|---|
| `familiar_ai/` | Familiar AI のトップレベル Plugin |
| `familiar_ai/perceive/` | 担当エリア内の Soul・タスク・リソース情報の収集 |
| `familiar_ai/decide/` | 行動方針の決定（タスク探索・Soul リクルート・Squad 管理） |
| `familiar_ai/decide/task_management/policy/` | タスク種別ごとのアサイン戦略（basic, haul, soul_spa 等） |
| `familiar_ai/execute/` | 決定結果の ECS への反映 |

## plugin 登録

- `FamiliarAiCorePlugin`（`src/lib.rs`）がシステムを登録する唯一の登録元
- `bevy_app/plugins/logic.rs` から `add_plugins(FamiliarAiPlugin)` で組み込まれる

## 仕様ドキュメント

- [docs/familiar_ai.md](../../docs/familiar_ai.md)
- [docs/tasks.md](../../docs/tasks.md)
