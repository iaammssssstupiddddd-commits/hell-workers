# bevy_app/systems/soul_ai — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このディレクトリがやること）

**ECS 接続層（アダプタ層）のみ**：`hw_soul_ai` のロジックを Bevy ECS へ接続する配線

具体的には：
- `SoulAiPlugin`：`SoulAiCorePlugin` の登録と `SoulAiSystemSet` / `FamiliarAiSystemSet` の ordering 設定
- `adapters.rs`：`DriftingEscapeStarted` / `SoulEscaped` などの root adapter Observer（`PopulationManager` 更新）
- `scheduling`：`SoulAiSystemSet` / `FamiliarAiSystemSet` の re-export
- re-export facade：移設済みシステムの `pub use` のみ含むファイル

## 禁止事項（AI がやってはいけないこと）

- **このディレクトリに Soul の行動判断・タスク実行ロジックを書かない**（`hw_soul_ai` に書く）
- **`hw_soul_ai` が登録済みのシステムを再登録しない**（二重登録でパニック）
- **`unassign_task` を Observer / アダプタ内から直接呼ぶ際は二重呼び出しに注意**（`OnTaskAbandoned` Observer 内では禁止 — I-S4）
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール

- `bevy_app` は **App Shell / Adapter**：pure ロジックはここに書かない
- `adapters.rs` は **正当な root adapter**：`PopulationManager` (hw_core) を更新する Observer は root shell の責務
- 詳細: [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md)

## ECS システムセット実行順（参照）

```
Input → Spatial → Logic → Actor → Visual → Interface
```

`SoulAiSystemSet` のサブ順序：
```
Perceive → Update → Decide → Execute
```
（`FamiliarAiSystemSet::Execute` → `ApplyDeferred` → `SoulAiSystemSet::Perceive` の接続）

## plugin / system 登録責務

- `SoulAiPlugin` が担う：
  1. `hw_soul_ai::SoulAiCorePlugin` のインストール
  2. `SoulAiSystemSet` の `configure_sets`（ordering のみ）
  3. root adapter Observer（`adapters.rs` の Observer）の `add_observer`
- leaf 側 `SoulAiCorePlugin` が登録済みのシステムはここで再登録しない

## 既存の re-export ファイル

以下は `hw_soul_ai` への移設完了済み：

- `execute/cleanup.rs` → `pub use hw_soul_ai::...`
- `execute/task_execution/mod.rs` → `pub use hw_soul_ai::...`
- `execute/task_execution/transport_common/*.rs` → `pub use hw_soul_ai::...`
- `helpers/work.rs` → `pub use hw_soul_ai::...`
- `decide/drifting.rs` → `pub use hw_soul_ai::...`

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/soul_ai.md](../../../../../docs/soul_ai.md)
- [docs/architecture.md](../../../../../docs/architecture.md)（システムセット構造変更時）
- `crates/bevy_app/src/systems/soul_ai/_rules.md`（このファイル）

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/soul_ai.md](../../../../../docs/soul_ai.md): Soul AI / task execution 仕様
- [docs/tasks.md](../../../../../docs/tasks.md): unassign_task 契約
- [docs/invariants.md](../../../../../docs/invariants.md): ゲーム不変条件（必読）
- [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md): leaf/root 境界ルール
- [docs/architecture.md](../../../../../docs/architecture.md): システムセット実行順
- [crates/hw_soul_ai/_rules.md](../../../../hw_soul_ai/_rules.md): leaf crate ルール
