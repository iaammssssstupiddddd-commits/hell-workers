# bevy_app/systems/familiar_ai — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このディレクトリがやること）

**ECS 接続層（アダプタ層）のみ**：`hw_familiar_ai` のロジックを Bevy ECS へ接続する配線

具体的には：
- `FamiliarAiPlugin`：`FamiliarAiCorePlugin` の登録と `FamiliarAiSystemSet` の ordering 設定
- root-only adapter：`GameAssets` / `UiNodeRegistry` 等 root 固有型が必要な処理
- re-export facade：`hw_familiar_ai` から移設済みシステムの `pub use` のみ含むファイル

## 禁止事項（AI がやってはいけないこと）

- **このディレクトリにビジネスロジックを書かない**（Familiar の行動判断・タスク探索・Squad 管理のロジックは `hw_familiar_ai` に書く）
- **`hw_familiar_ai` が登録済みのシステムを bevy_app 側で再登録しない**（二重登録でパニック）
- **Bevy 0.14 以前の API を推測で使わない**（0.18 の変更点が多い。`docs.rs/bevy/0.18.0` または既存コードを参照）

## crate 境界ルール

- `bevy_app` は **App Shell / Adapter**：純粋なビジネスロジックをここに書かない
- leaf crate の system を bevy_app 側に pull back するのは、root 固有型（`GameAssets` 等）が必要な場合のみ
- 詳細: [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md)

## ECS システムセット実行順（参照）

```
Input → Spatial → Logic → Actor → Visual → Interface
```

`FamiliarAiSystemSet` のサブ順序：
```
Perceive → (ApplyDeferred) → Update → (ApplyDeferred) → Decide → Execute
```

（`GameSystemSet::Logic` 内に位置する）

## plugin / system 登録責務

- `FamiliarAiPlugin` が担う：
  1. `hw_familiar_ai::FamiliarAiCorePlugin` のインストール
  2. `FamiliarAiSystemSet` の `configure_sets`（ordering のみ）
  3. root-only resource の `init_resource`（`SharedResourceCache`, `ReservationSyncTimer`）
  4. `sync_reservations_system`（Perceive フェーズ）および ApplyDeferred の登録
- leaf 側 `FamiliarAiCorePlugin` / `HwVisualPlugin` が登録済みのシステムはここで再登録しない

## re-export facade ファイル（残存）

以下のファイルは `hw_familiar_ai` / `hw_visual` への移設完了済みの facade。実装はすべて leaf crate 側にある（これらのファイルの編集は原則不要）：

- `decide/encouragement.rs` → `pub use hw_familiar_ai::...`
- `decide/auto_gather_for_blueprint.rs` → `pub use hw_familiar_ai::...`
- `execute/encouragement_apply.rs` → `pub use hw_familiar_ai::...`
- `execute/idle_visual_apply.rs` → `pub use hw_visual::familiar_idle_visual_apply_system`
- `execute/max_soul_apply.rs` → logic: `hw_familiar_ai::max_soul_logic_system`, visual: `hw_visual::max_soul_visual_system`
- `execute/squad_apply.rs` → logic: `hw_familiar_ai::squad_logic_system`, visual: `hw_visual::squad_visual_system`

これら facade 群の削除は別タスクとして管理する。

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/familiar_ai.md](../../../../../docs/familiar_ai.md)
- [docs/architecture.md](../../../../../docs/architecture.md)（システムセット構造変更時）
- `crates/bevy_app/src/systems/familiar_ai/_rules.md`（このファイル）

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/familiar_ai.md](../../../../../docs/familiar_ai.md): Familiar AI 仕様
- [docs/crate-boundaries.md](../../../../../docs/crate-boundaries.md): leaf/root 境界ルール
- [docs/architecture.md](../../../../../docs/architecture.md): システムセット実行順
- [crates/hw_familiar_ai/_rules.md](../../../../hw_familiar_ai/_rules.md): leaf crate ルール
