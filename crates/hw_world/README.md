# hw_world — ワールド・地形・経路探索

## 役割

ゲームワールドの地形生成・管理、座標変換ユーティリティ、A* 経路探索を提供するクレート。
`WorldMap` 本体、`WorldMapRead` / `WorldMapWrite` の `SystemParam`、固定アンカーと生成マスク、room detection の ECS 型、world 系の軽量 system を所有する。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `coords.rs` | 座標変換 (`grid_to_world`, `world_to_grid`, `snap_to_grid_*`, `idx_to_pos`) |
| `anchor.rs` | `AnchorLayout`, `GridRect`, Yard 内固定物の pure data 契約 |
| `map/` | `WorldMap` — 地形・歩行可能性・建物データの保持（access, bridges, buildings, doors, obstacles, stockpiles, tiles のサブモジュールを含む） |
| `terrain.rs` | `TerrainType` enum (Water, Sand, Rock, Grass, ...) |
| `mapgen.rs` | `generate_base_terrain_tiles()` と `generate_world_layout()` |
| `mapgen/wfc_adapter.rs` | `wfc` crate を隠蔽する adapter 骨格と制約変換 |
| `river.rs` | 固定 River 生成、seed 付き `river_mask` 生成、砂地導出 |
| `layout.rs` | ワールドレイアウト定数 (木・岩・木材の初期位置, 川の範囲) |
| `world_masks.rs` | `site_mask`, `yard_mask`, protection band, `river_mask`, `river_centerline` |
| `regrowth.rs` | 森林再生システム (`ForestZone`, 周期的な木スポーン) |
| `pathfinding/` | A* 経路探索（下記詳細参照） |
| `query.rs` | 環境クエリ (`find_nearest_river_grid`, `find_nearest_walkable_grid`) |
| `room_detection/` | Room 検出 core (`build_detection_input`, `detect_rooms`, `room_is_valid_against_input`, `RoomBounds`。core/ecs/tests サブモジュールを含む） |
| `room_systems.rs` | `detect_rooms_system`, `validate_rooms_system` |
| `door_systems.rs` | ドア自動開閉、`DoorVisualHandles`, `apply_door_state` |
| `terrain_visual.rs` | 障害物 cleanup、`TerrainVisualHandles`、`TerrainChangedEvent`（`Message`）発行 |
| `spatial.rs` | ワールド向け `SpatialGridOps` 実装 |
| `spawn.rs` | スポーンヘルパー (`find_nearby_walkable_grid`, `pick_random_walkable_grid_in_rect`) |
| `zones.rs` | `Yard`, `Site`, `PairedYard`, `PairedSite` — ゾーン系コンポーネント |
| `zone_ops.rs` | ゾーン操作の純粋アルゴリズム helper (`expand_yard_area`, `identify_removal_targets` 等) |
| `tree_planting.rs` | `DreamTreePlantingPlan` — 植林プランデータ構造 |
| `map/access.rs` | `WorldMapRead`, `WorldMapWrite` (`SystemParam`) |

## 経路探索 (`pathfinding/`)

### 主要関数

```rust
find_path(map, from, to)                          // 基本 A*
find_path_to_adjacent(map, from, target)          // 隣接タイルへの探索
find_path_to_boundary(map, from)                  // マップ境界への探索
find_path_world_waypoints(map, from, to)          // ウェイポイント付き経路探索
can_reach_target(map, from, to)                   // 到達可能性チェック
```

### PathGoalPolicy トレイト

歩行可能性の契約をカスタマイズするトレイト。用途に応じて探索条件を差し替え可能。

## 座標系

- **グリッド座標**: タイル単位の整数インデックス
- **ワールド座標**: Bevy の `Vec2` / `Vec3` (ピクセル単位)
- `coords.rs` の変換関数を通して相互変換する

## 依存クレート

- `hw_core`, `hw_jobs`, `bevy`, `rand`
- `wfc`, `direction`

---

## src/ との境界

hw_world はワールドの**所有型・SystemParam・world系ロジック**を提供する。
root 側は `GameAssets` 依存の startup / spawn / plugin wiring を担当する。

| hw_world に置くもの | src/world/ に置くもの |
|---|---|
| `WorldMap` 型と全データ構造 | `WorldMap` の `init_resource`、startup/wiring |
| `WorldMapRead` / `WorldMapWrite` (`SystemParam`) | root facade からの re-export |
| A* 経路探索関数群 | マップエンティティのスポーン (`spawn.rs`) |
| Room 検出 core + `Room` / `RoomOverlayTile` / `RoomTileLookup` / `RoomDetectionState` / `RoomValidationState` | overlay sync、dirty mark observer の root wiring |
| 座標変換関数 (`grid_to_world` 等) | 3D 地形メッシュ・マテリアル同期（`spawn.rs`、`terrain_material_sync_system` 等） |
| 地形生成関数 (`generate_base_terrain_tiles`, `generate_world_layout`) | `Commands` / `Terrain3dHandles` を使う描画スポーン |
| `obstacle_cleanup_system` / `door_auto_open_system` / `door_auto_close_system` | `GameAssets` から専用 handle Resource を注入する startup |
| `Yard`, `Site`, `PairedYard`, `PairedSite` | — |
| `AnchorLayout`, `WorldMasks`, `mapgen::wfc_adapter` | `spawn_map` の暫定 preview 接続や startup 統合 |

**判断基準**: root 固有の `GameAssets` / UI / plugin wiring が必要なら `src/` に残す。
shared crate 型と `WorldMap` access だけで閉じるなら hw_world に置く。

`WorldMapRead` / `WorldMapWrite` はシステム引数として使用する:
```rust
// システム引数でワールドマップにアクセス
fn my_system(world_map: WorldMapRead) {
    let path = world_map.find_path(from, to);
}
```
