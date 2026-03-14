# bevy_app/systems/jobs — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このディレクトリがやること）

**ECS 接続層（アダプタ層）のみ**：`hw_jobs` のジョブ型を Bevy ECS に接続する配線

具体的には：
- 建築系 (`building_completion`, `floor_construction`, `wall_construction`)、ドア (`door.rs`) などの Apply システム
- `Designation` / `TransportRequest` の生成アダプタ

## 禁止事項（AI がやってはいけないこと）

- **このディレクトリにジョブの型定義を書かない**（型は `hw_jobs` に置く）
- **`unassign_task` を迂回して `AssignedTask` を直接 `None` にしない**（予約リークが発生する — I-L1）
- **Haul 系 WorkType を持つ Designation を `TransportRequest` なしで生成しない**（サイレントフィルタ — I-T1）
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール

- `bevy_app` は **App Shell / Adapter**：ジョブロジックは `hw_jobs` / `hw_soul_ai` に置く
- 詳細: [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md)

## ECS システムセット実行順（参照）

```
Input → Spatial → Logic → Actor → Visual → Interface
```

建築系 Apply システムは通常 `GameSystemSet::Logic` または `GameSystemSet::Actor` に属する。

## ⚠️ 重要な注意事項

- **TransportRequest の必須添付**：Haul 系 WorkType を持つ Designation を発行する際は `TransportRequest` を同時に添付すること（詳細: [docs/invariants.md §I-T1](../../../../../docs/invariants.md)）
- **Designation 削除の影響**：`Designation` を削除するとタスクが消滅する（詳細: [docs/invariants.md §I-T4](../../../../../docs/invariants.md)）

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/tasks.md](../../../../../docs/tasks.md)
- [docs/building.md](../../../../../docs/building.md)（建築システム変更時）
- `crates/bevy_app/src/systems/jobs/_rules.md`（このファイル）

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/tasks.md](../../../../../docs/tasks.md): タスク ECS 接続マップ
- [docs/invariants.md](../../../../../docs/invariants.md): ゲーム不変条件（I-T1, I-T4）
- [docs/building.md](../../../../../docs/building.md): 建築プロセス
- [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md): leaf/root 境界ルール
- [crates/hw_jobs/_rules.md](../../../../hw_jobs/_rules.md): leaf crate ルール
