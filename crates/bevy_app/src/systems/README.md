# systems — ゲームロジック実装

## 役割

ゲームの全ロジック（AI・タスク・ロジスティクス・建設・視覚・空間）を実装するディレクトリ。
`plugins/logic.rs`・`plugins/spatial.rs`・`plugins/visual.rs` から登録される。

## ディレクトリ構成

| ディレクトリ / ファイル | フェーズ | 内容 |
|---|---|---|
| `soul_ai/` | Logic | Soul（魂）の意思決定・タスク実行 |
| `familiar_ai/` | Logic | Familiar（使い魔）の意思決定・タスク委譲 |
| `command/` | Logic | プレイヤーのコマンド処理（タスクエリア・ゾーン配置） |
| `jobs/` | Logic | 建設フェーズ遷移・建物完成・ドア管理 |
| `logistics/` | Logic | リソース管理・輸送要求・ゾーン・地上アイテム |
| `visual/` | Visual | 視覚フィードバック・アニメーション同期 |
| `dream_tree_planting.rs` | Logic | ドリームツリーの植林システム |
| `time.rs` | Logic | ゲーム内時間管理 |

## 各システムの関係

```
[プレイヤー入力]
    ↓
command/ → Designation / TransportRequest エンティティ生成
    ↓
plugins/spatial.rs → 空間グリッド更新（毎フレーム）
    ↓
familiar_ai/ → タスク発見・割り当て要求生成
    ↓
soul_ai/ → タスク実行・バイタル更新
    ↓
jobs/ / logistics/ → 建設進行・リソース状態変化
    ↓
visual/ → 視覚フィードバック同期
```

## 重要な設計原則

- **Decide フェーズ**: リクエストメッセージを生成するのみ（ECS 変更禁止）
- **Execute フェーズ**: メッセージを消費して ECS を変更する
- タスククエリは `TaskQueries` / `TaskAssignmentQueries` に集約する（system 引数の散在を避ける）
