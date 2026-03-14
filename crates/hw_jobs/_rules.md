# hw_jobs — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- `AssignedTask` enum の定義とライフサイクル（ECS Component として Soul に付与）
- `Designation` / `TaskSlots` / `WorkingOn` 等タスク関連 ECS Component の型定義
- タスク関連イベントの定義（`hw_jobs/src/events.rs`）
- タスク状態遷移ロジックの定義（データ型レベル）

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **`#[allow(dead_code)]` を使用しない**（使われないコードは削除する）
- **`unassign_task` をこのクレートに実装しない**（タスク中断の実行は `hw_soul_ai` の責務）
- **`hw_logistics` / `hw_world` / `hw_familiar_ai` / `hw_soul_ai` に依存しない**（これらはすべて `hw_jobs` の下流）
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：Bevy 型の利用は許可
- `bevy_app` への逆依存は **完全禁止**
- このクレートは最も基底に近い業務型クレート：多くのクレートが依存するため変更コスト高
- 型の所有権原則：`AssignedTask` や `Designation` など複数クレートから参照される型は **このクレートが所有者**
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
hw_core      ✓
bevy         ✓

# 禁止（すべて下流クレート）
bevy_app     ✗
hw_world     ✗
hw_logistics ✗
hw_spatial   ✗
hw_soul_ai   ✗
hw_familiar_ai ✗
hw_ui        ✗
hw_visual    ✗
```

## plugin / system 登録責務

- このクレートは **Plugin を持つ必要はない**（型定義・イベント定義が主目的）
- system 登録が必要な場合は `hw_soul_ai` または `bevy_app` が担う

## 重要な設計メモ

- **`unassign_task` は `hw_soul_ai` 側の契約**：このクレートに「タスク中断関数」を追加しない
- `AssignedTask::None` への変化は `OnTaskCompleted` を発火させる（Change Detection）
- `Designation` を削除するとタスクが消滅する（詳細: [docs/invariants.md §I-T4](../../docs/invariants.md)）
- `WorkingOn` Relationship は Source 側操作で Target 側（`TaskWorkers`）が自動更新される（手動書き込み禁止）

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/tasks.md](../../docs/tasks.md)
- [docs/events.md](../../docs/events.md)（イベント変更時）
- `crates/hw_jobs/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/tasks.md](../../docs/tasks.md): タスク ECS 接続マップと unassign_task 契約
- [docs/events.md](../../docs/events.md): イベントカタログ
- [docs/invariants.md](../../docs/invariants.md): ゲーム不変条件
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
