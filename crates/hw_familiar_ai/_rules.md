# hw_familiar_ai — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- Familiar（使い魔）の状態遷移・タスク探索・Squad 管理・リクルート・激励（encouragement）のビジネスロジック
- Familiar AI のフェーズごとの pure 処理：Perceive / Decide / Execute
- `FamiliarAiCorePlugin`：このクレート内で完結するシステムの唯一の登録者

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約。コンパイルエラーになる）
- **Soul の `WorkingOn` Relationship を直接操作しない**（タスク割当は `hw_soul_ai` 側の `apply_task_assignment_requests` が担う）
- **`GameAssets` や `UiNodeRegistry` を引数に取るシステムをこのクレートに書かない**（root shell 固有型）
- **`#[allow(dead_code)]` を使用しない**（使われないコードは削除する）
- **Bevy 0.14 以前の API を推測で使わない**（0.18 の変更点が多い。既存コードまたは docs.rs/bevy/0.18.0 で確認する）

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- このクレートは **leaf crate**：Bevy 型（`Entity`, `Query`, `Res`, `Commands` 等）の利用は許可・推奨
- `bevy_app` への逆依存は **完全禁止**
- Decision フェーズは副作用を持たない純粋ロジックへの切り出しを優先する
- Execute / Apply フェーズは leaf crate の system / observer として直接実装してよい
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
hw_core      ✓
hw_jobs      ✓
hw_logistics ✓
hw_soul_ai   ✓  (タスク割当クエリ・unassign_task を利用)
hw_world     ✓
hw_spatial   ✓
bevy         ✓

# 禁止
bevy_app     ✗  (逆依存禁止)
hw_ui        ✗  (UI 層は bevy_app が保持する)
hw_visual    ✗  (視覚演出は hw_visual 側から hw_core events を購読する)
```

## plugin / system 登録責務

- **`FamiliarAiCorePlugin`** がこのクレート内で完結するシステムの唯一の登録者
- `bevy_app` 側は ordering 参照・root-only adapter の追加のみを行う
- `bevy_app` が `FamiliarAiCorePlugin` を重複登録してはならない

## 主要な不変条件

- **I-F1**: Familiar は直接作業しない（`WorkingOn` を自身に付けてはならない）
- **I-F2**: リクルート閾値 < リリース閾値（大小関係を逆転させない）
- 詳細: [docs/invariants.md](../../docs/invariants.md) §2

## 既知のサイレント失敗トラップ

- `blueprint_auto_gather_system` が自動採取リクエストを出す際、対象タイルに `Designation` が存在しないとサイレントスキップ
- `FamiliarIdleVisualRequest` は `bevy_app` の `MessagesPlugin` で `add_message` 登録が必要（未登録だと `MessageReader` が空になる）

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/familiar_ai.md](../../docs/familiar_ai.md)
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（Cargo.toml 変更時）
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md)（境界ルール変更時）
- `crates/hw_familiar_ai/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/familiar_ai.md](../../docs/familiar_ai.md): Familiar AI 仕様
- [docs/tasks.md](../../docs/tasks.md): タスク ECS 接続マップと unassign_task 契約
- [docs/invariants.md](../../docs/invariants.md): ゲーム不変条件
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
