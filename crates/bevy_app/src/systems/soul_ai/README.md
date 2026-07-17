# soul_ai — Soul（魂）AI の app shell

## 役割

`DamnedSoul` の知覚・状態更新・意思決定・タスク実行の実装本体は
`hw_soul_ai::soul_ai` が所有する。このディレクトリには、Bevy app の plugin 配線、
root 固有 resource との adapter、および既存 import path を保つ薄い facade だけを置く。

## 現在の構成

| パス | 所有する責務 |
|---|---|
| `mod.rs` | `SoulAiPlugin`、system set の順序、`hw_soul_ai::SoulAiCorePlugin` の登録 |
| `adapters.rs` | drifting event を root 固有の `PopulationManager` に反映する observer |
| `execute/gathering_spawn.rs` | `GameAssets` に依存する集会 visual spawn adapter |
| `execute/task_execution/mod.rs` | task execution API の thin facade |
| `_rules.md` | この境界の実装ルール |

`perceive/`、`update/`、`decide/`、task handler の実装を root 側へ追加しない。
共有ロジックと system 登録は `hw_soul_ai` に置き、描画専用ロジックは `hw_visual`
に置く。

## task_execution facade

`execute/task_execution/mod.rs` は独自の実行ロジックを持たず、次の API を再公開する。

- `hw_soul_ai::soul_ai::execute::task_execution` の context、handler、types、helper
- `hw_familiar_ai` の `FamiliarTaskAssignmentQueries`
- `task_execution_system` と `apply_task_assignment_requests_system`

`AssignedTask` の正本は `crates/hw_jobs/src/tasks/mod.rs` にある。各 variant は
`Variant(VariantData)` の payload 付き tuple variant とし、payload 型は
`crates/hw_jobs/src/tasks/` の機能別ファイルに置く。

## 新しいタスクを追加する場合

1. `crates/hw_jobs/src/tasks/<feature>.rs` に payload と phase を定義し、
   `crates/hw_jobs/src/tasks/mod.rs` の `AssignedTask` に `Variant(VariantData)` を追加する。
2. 必要な ECS query を
   `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs` の
   `TaskQueries` に集約する。
3. `crates/hw_soul_ai/src/soul_ai/execute/task_execution/` に handler を実装し、
   dispatcher と module 宣言へ接続する。
4. task lifecycle、失敗時の disposition、save/load への影響を `docs/tasks.md` と
   関連する恒久ドキュメントへ反映する。

## root に実装を残せる条件

次のいずれかを満たす場合だけ root 側に実装を置く。

- `PopulationManager` や `GameAssets` など、app shell 固有の型へアクセスする adapter
- plugin wiring や system set の境界
- 既存 import path を維持する re-export のみの thin facade

共有 model、task lifecycle、純粋な AI 判断、navigation、spatial resource だけで閉じる
処理は、それぞれ `hw_core`、`hw_jobs`、`hw_soul_ai`、`hw_spatial` に置く。

## system 登録

`SoulAiPlugin` は `hw_soul_ai::SoulAiCorePlugin` を登録し、
`FamiliarAiSystemSet::Execute` の後に Soul AI の
`Perceive -> Update -> Decide -> Execute` を順序付ける。root 側で同じ core system を
重複登録してはいけない。

変更後は少なくとも次を実行する。

```bash
python3 scripts/dev.py check
python3 scripts/dev.py verify
```
