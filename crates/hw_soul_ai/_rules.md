# hw_soul_ai — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- Soul（Damned Soul）の AI システム：Perceive / Decide / Execute / Update フェーズ実装
- `task_execution_system`：`AssignedTask` フェーズステートマシンの実行
- `unassign_task`：タスク中断・放棄の最終防衛線（予約解放・パスクリア・タスク解除）
- `cleanup_commanded_souls_system`：Soul のクリーンアップ
- `drifting_decision_system`：未管理 Soul の自然脱走遷移
- `SoulAiCorePlugin`：このクレート内で完結するシステムの唯一の登録者

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **`GameAssets` / `UiNodeRegistry` / `GameAssets` を引数に取るシステムをこのクレートに書かない**
- **`unassign_task` を呼ばずに `AssignedTask` を直接 `None` にしない**（予約リークが発生する）
- **Observer 内で `unassign_task` を二重呼び出しない**（I-S4: `OnTaskAbandoned` Observer はクリーンアップ不要）
- **`CommandedBy` の削除を `unassign_task` 内で行わない**（削除責務は `OnExhausted` / `OnStressBreakdown` Observer）
- **`#[allow(dead_code)]` を使用しない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：Bevy 型の利用は許可
- `bevy_app` への逆依存は **完全禁止**
- Decision フェーズは副作用を持たない純粋ロジックへの切り出しを優先
- Execute / Apply フェーズは leaf crate system / observer として直接実装してよい
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
hw_core      ✓
hw_jobs      ✓
hw_logistics ✓
hw_world     ✓
hw_spatial   ✓
bevy         ✓
rand         ✓

# 禁止
bevy_app     ✗  (逆依存禁止)
hw_familiar_ai ✗  (双方向依存禁止)
hw_ui        ✗
hw_visual    ✗
```

## plugin / system 登録責務

- **`SoulAiCorePlugin`** がこのクレート内で完結するシステムの唯一の登録者
- `bevy_app` 側は ordering 参照と root-only adapter の追加のみ
- 以下はこのクレートが唯一の登録元：
  - `apply_task_assignment_requests_system`（タスク割当実行）
  - `task_execution_system`（タスクフェーズ進行）
  - `cleanup_commanded_souls_system`（Soul クリーンアップ）
  - `drifting_decision_system`（漂流意思決定）

## 重要な契約（不変条件）

### unassign_task の契約
- `SharedResourceCache` 予約解放の最終防衛線
- タスクを中断・放棄・完了する**全経路**で呼ぶこと
- 内部で `CommandedBy` を削除しない（呼び出し元が必要に応じて削除する）
- `emit_abandoned_event = true` の場合のみ `OnTaskAbandoned` を発火
- 詳細: [docs/invariants.md §I-L1](../../docs/invariants.md)

### CommandedBy 削除責務
- **Observer 側**（`OnExhausted` / `OnStressBreakdown`）が削除する
- `unassign_task` 内では削除しない
- 詳細: [docs/invariants.md §I-S2](../../docs/invariants.md)

### AssignedTask の変更
- `None` への変化時に `OnTaskCompleted` が発火する（Change Detection）
- `None` 状態のまま直接書き換えてはならない
- 詳細: [docs/invariants.md §I-S3](../../docs/invariants.md)

## 既知のサイレント失敗トラップ

- Haul 系 `WorkType`（`Haul` / `HaulToMixer` / `GatherWater` / `WheelbarrowHaul`）は `TransportRequest` がないとタスク検索で無音スキップ（詳細: [docs/invariants.md §I-T1](../../docs/invariants.md)）
- `WorkingOn` Relationship の Target 側（`TaskWorkers`）を手動操作すると整合性崩壊（詳細: [docs/invariants.md §I-T2](../../docs/invariants.md)）

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/soul_ai.md](../../docs/soul_ai.md)
- [docs/tasks.md](../../docs/tasks.md)
- [docs/invariants.md](../../docs/invariants.md)（不変条件に変化があった場合）
- [docs/events.md](../../docs/events.md)（イベント変更時）
- `crates/hw_soul_ai/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/soul_ai.md](../../docs/soul_ai.md): Soul AI / task execution 仕様
- [docs/tasks.md](../../docs/tasks.md): タスク ECS 接続マップと unassign_task 契約
- [docs/invariants.md](../../docs/invariants.md): ゲーム不変条件（必読）
- [docs/events.md](../../docs/events.md): イベントカタログ
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
