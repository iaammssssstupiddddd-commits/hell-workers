# hw_soul_ai — Soul AI ロジック

## 役割

Soul（Damned Soul）の AI を実装するクレート。
Perceive / Decide / Execute / Update の各フェーズでの行動選択・タスク実行・バイタル更新を担う。
タスク中断時の最終防衛線 `unassign_task` もここに置く。

## 主要モジュール

| ディレクトリ/ファイル | 内容 |
|---|---|
| `soul_ai/` | Soul AI のトップレベル Plugin |
| `soul_ai/perceive/` | 周囲のリソース・タスク・施設情報の収集 |
| `soul_ai/decide/` | 行動選択（タスク割当・アイドル行動・脱走判定） |
| `soul_ai/execute/` | タスクフェーズステートマシンの実行（`task_execution_system`） |
| `soul_ai/execute/task_execution/` | 各タスク種別の具体的な実行ロジック |
| `soul_ai/update/` | バイタル（疲労・ストレス・dream）の更新 |
| `soul_ai/helpers/work/` | `unassign_task` — タスク中断の最終防衛線 |

## plugin 登録

- `SoulAiCorePlugin`（`src/lib.rs`）がシステムを登録する唯一の登録元
- `bevy_app/plugins/logic.rs` から `add_plugins(SoulAiPlugin)` で組み込まれる

## ⚠️ unassign_task の契約

タスクを中断・放棄・完了する**全経路**で `soul_ai::helpers::work::unassign_task` を呼ぶこと。
内部では `SharedResourceCache` 予約解放・パスクリア・`AssignedTask = None` を行う。
`CommandedBy` の削除は呼び出し元（Observer 等）の責務。
詳細: [docs/invariants.md](../../docs/invariants.md)

## 仕様ドキュメント

- [docs/soul_ai.md](../../docs/soul_ai.md)
- [docs/tasks.md](../../docs/tasks.md)
- [docs/invariants.md](../../docs/invariants.md)
