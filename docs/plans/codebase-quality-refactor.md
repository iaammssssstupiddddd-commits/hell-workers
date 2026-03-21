# リファクタリング計画: コードベース全体の整理・品質向上

## 問題の概要

コードベース全体（660ファイル、約68,667行）のメンテナンス性向上を目的とする。
主な問題は以下の4点：
- 肥大化ファイル（459行のenumなど）
- コードの重複（pathfinding等が3箇所）
- `bevy_app` への責務の混在（215ファイル＝全体の33%）
- 命名規則の不統一

## アプローチ

独立したフェーズで段階的に実施。各フェーズ後に `cargo check` でコンパイル確認。
リスクの低い構造整理から始め、クレート境界の見直しへと進む。

---

## Phase 1: ファイル分割・構造整理（低リスク・高リターン）

### 1-A: `assigned_task.rs` の分割
- **対象**: `crates/hw_jobs/src/assigned_task.rs` (459行)
- **方針**: `hw_jobs/src/tasks/` ディレクトリを作成し、タスクグループ別にファイルを分割
- **分割案**:
  ```
  hw_jobs/src/tasks/
  ├── mod.rs          # AssignedTask enum の再エクスポート
  ├── gather.rs       # GatherData, GatherPhase
  ├── haul.rs         # HaulData, HaulToBlueprintData, HaulPhase, HaulToBpPhase
  ├── build.rs        # BuildData, BuildPhase, ReinforceFloorTileData, PourFloorTileData,
  │                   #   FrameWallTileData, CoatWallData (各Phase型)
  ├── bucket.rs       # BucketTransportData, BucketTransportPhase, BucketTransportSource,
  │                   #   BucketTransportDestination
  ├── collect.rs      # CollectSandData, CollectSandPhase, CollectBoneData, CollectBonePhase
  ├── refine.rs       # RefineData, RefinePhase, HaulToMixerData
  ├── move_plant.rs   # MovePlantData, MovePlantPhase
  └── wheelbarrow.rs  # HaulWithWheelbarrowData, HaulWithWheelbarrowPhase
  ```
- **パブリック API**: `assigned_task.rs` を `tasks/mod.rs` にリネームし、既存の import path を維持
- **検証**: `cargo check` + 既存テストの確認

### 1-B: `WorldMap` メソッドの論理グループ分割
- **対象**: `crates/hw_world/src/map/mod.rs` (453行、40+メソッド)
- **方針**: Rust の `impl` ブロックを別ファイルに分けることで可読性向上（APIは不変）
- **分割案**:
  ```
  hw_world/src/map/
  ├── mod.rs           # WorldMap 構造体定義 + use宣言 + pub use
  ├── access.rs        # (既存) WorldMapRead / WorldMapWrite
  ├── tiles.rs         # タイル読み書き (terrain, tile_entity, walkable, river)
  ├── obstacles.rs     # 障害物管理 (add/remove_obstacle, obstacle_count)
  ├── buildings.rs     # 建物管理 (set/clear_building, footprint, occupancy)
  ├── doors.rs         # ドア管理 (add/remove_door, door_state, door_entity)
  ├── stockpiles.rs    # 備蓄管理 (set/clear_stockpile, stockpile_tile)
  └── bridges.rs       # 橋管理 (add_bridged_tile, register_bridge_tile)
  ```
- **注意**: `impl WorldMap` を複数ファイルに分割する（トレイトは新設しない）
- **検証**: `cargo check`

---

## Phase 2: コード重複の解消（中リスク）

### 2-A: Pathfinding の整理
- **対象**:
  - `hw_world/src/pathfinding.rs` (421行) ← 共通実装
  - `hw_soul_ai/src/soul_ai/pathfinding/` (585行: mod.rs, fallback.rs, reuse.rs)
- **現状確認**: soul_ai pathfinding が hw_world pathfinding の何をラップしているか調査
- **方針**: soul_ai 固有の「再利用・フォールバック」ロジックを hw_world 側の共通 API に昇格できるか検討し、重複を削減
- **注意**: 性能上重要な hot path は慎重に扱う

### 2-B: Reservation Ops の統合
- **対象**:
  - `hw_familiar_ai/.../policy/haul/wheelbarrow.rs`
  - `hw_soul_ai/.../transport_common/wheelbarrow.rs`
  - `hw_logistics/src/resource_cache.rs`
- **方針**: 共通ロジックを `hw_logistics::reservation_ops` module に集約
- **前提条件**: Phase 1 完了後

---

## Phase 3: 命名規則の統一（低リスク・広範囲）

### 3-A: システム関数命名の統一
- **現状の不統一**:
  | パターン | 例 |
  |---------|-----|
  | `update_*_system` | `update_grid_system` |
  | `sync_*_system` | `sync_reservations_system` |
  | `apply_*_system` | `apply_designation_requests_system` |
  | `execute_*_system` | `execute_task_system` |
  | `handle_*_system` | `handle_..._system` |
- **方針**: `docs/DEVELOPMENT.md` に命名規則を明文化し、新規コードから適用
  - Observer: `on_{event}` (例: `on_task_completed`)
  - 決定フェーズ: `decide_{topic}_system`
  - 実行フェーズ: `execute_{topic}_system`
  - 更新フェーズ: `update_{topic}_system`
  - 同期フェーズ: `sync_{topic}_system`
- **既存コード**: 一括リネームは副作用が大きいため、次回変更時に対象ファイルを個別対応

### 3-B: 概念の命名統一
- **問題**: 同じ概念に複数の名前が存在

  | 概念 | 現状の名前 | 統一後 |
  |------|----------|--------|
  | ソウルのタスク | `WorkingOn`, `AssignedTask`, `TaskWorkers` | `AssignedTask` (既存を維持) |
  | グリッド座標型 | `(i32, i32)`, `grid`, `pos` 等が混在 | 型エイリアス `GridPos = (i32, i32)` を hw_core に追加検討 |

- **方針**: `hw_core` に type alias を追加し、段階的に置き換え

---

## Phase 4: `bevy_app` のスリム化（高リスク・長期）

### 4-A: `entities/` の整理
- **対象**: `bevy_app/src/entities/` (ソウル・使い魔のスポーン処理)
- **方針**: `crate-boundaries.md` に従い、Spawn ロジックを対応する leaf crate へ移動
- **前提条件**: Phase 1, 2 完了後に設計検討

### 4-B: `systems/` の domain crate 移動
- **対象**: `bevy_app/src/systems/` のうち純粋ドメインロジック
- **方針**: `GameAssets` 等の bevy_app 固有型に依存しない system を leaf crate に移動
- **注意**: `crate-boundaries.md` のルールを厳守

---

## 優先順位と進め方

```
Phase 1-A → Phase 1-B → Phase 2-A → Phase 2-B → Phase 3-A → Phase 3-B → Phase 4
  (即実施可)    (即実施可)   (調査後)    (調査後)    (ドキュメント)   (段階的)    (長期)
```

## 検証方法

各フェーズ完了後:
```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

---

## ファイル変更一覧（Phase 1）

### Phase 1-A
- 新規: `crates/hw_jobs/src/tasks/mod.rs`, `gather.rs`, `haul.rs`, `build.rs`, `bucket.rs`, `collect.rs`, `refine.rs`, `move_plant.rs`, `wheelbarrow.rs`
- 変更: `crates/hw_jobs/src/lib.rs` (モジュール参照を `tasks` に変更)
- 削除: `crates/hw_jobs/src/assigned_task.rs` (内容を tasks/ に移動)

### Phase 1-B
- 新規: `crates/hw_world/src/map/tiles.rs`, `obstacles.rs`, `buildings.rs`, `doors.rs`, `stockpiles.rs`, `bridges.rs`
- 変更: `crates/hw_world/src/map/mod.rs` (impl を分割後、構造体定義のみ残す)
