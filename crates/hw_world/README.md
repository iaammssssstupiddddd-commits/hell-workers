# hw_world — ワールド・地形・経路探索

## 役割

ゲームワールドの地形生成・管理、座標変換ユーティリティ、A* 経路探索を提供するクレート。
`WorldMap` 本体、`WorldMapRead` / `WorldMapWrite` の `SystemParam`、固定アンカーと生成マスク、room detection の ECS 型、world 系の軽量 system を所有する。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `coords.rs` | 座標変換 (`grid_to_world`, `world_to_grid`, `snap_to_grid_*`, `idx_to_pos`) |
| `anchor.rs` | `AnchorLayout`（本番は `aligned_to_worldgen_seed` で川南端基準に縦シフト）, `GridRect`, Yard 内固定物の pure data 契約 |
| `map/` | `WorldMap` — 地形・歩行可能性・建物データの保持（access, bridges, buildings, doors, obstacles, stockpiles, tiles のサブモジュールを含む） |
| `terrain.rs` | `TerrainType` enum (Water, Sand, Rock, Grass, ...) |
| `mapgen/mod.rs` | `mapgen` のモジュールルート。`generate_base_terrain_tiles()` と `generate_world_layout()` の公開面を持つ薄い shell / re-export |
| `mapgen/pipeline.rs` | `generate_world_layout()` の実装本体（WFC + validate + resource 配置 + retry/fallback + river/sand/rock-field 派生マスク） |
| `mapgen/resources.rs` | 木・岩・`forest_regrowth_zones` の procedural 配置。木は `grass_zone_mask`、岩は `rock_field_mask` を使う |
| `mapgen/validate.rs` | 生成後バリデータ（`lightweight_validate`, `validate_post_resource`, `debug_validate`, `ValidatorPathWorld`） |
| `mapgen/wfc_adapter.rs` | gridbugs `wfc` の adapter（`run_wfc`, `post_process_tiles`, `apply_zone_post_process`, `fallback_terrain`, `WorldConstraints`）。`final_sand_mask`・ゾーンバイアス・`rock_field_mask`・inland_sand を最終地形へ反映 |
| `test_seeds.rs` (`#[cfg(test)]`) | WFC 周辺テストの代表 seed 群。`mapgen` / `rock_fields` / `terrain_zones` が `crate::test_seeds::*` を共有参照する |
| `terrain_zones.rs` | MS-WFC-2.5: アンカー距離場→seed 選択→flood fill で `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` を生成。`compute_zone_distance_field` でゾーン境界距離場を提供 |
| `rock_fields.rs` | MS-WFC-3b: 川・砂・内陸砂・アンカー帯を避けた east-side の `rock_field_mask` を deterministic に生成 |
| `river.rs` | 固定 River 生成、seed 付き `river_mask` 生成、`preview_river_min_y`（プレビュー川の南端 y）、river distance field + base shoreline + bounded growth による `sand_candidate_mask` / carve / `final_sand_mask` の導出 |
| `layout.rs` | レガシー固定川の範囲 (`RIVER_*`) と `SAND_WIDTH`（`generate_base_terrain_tiles` / 建物配置ヒント等） |
| `world_masks.rs` | `site_mask`, `yard_mask`, protection band, `river_mask`, `river_centerline`, `sand_candidate_mask`, `sand_carve_mask`, `final_sand_mask`, `grass_zone_mask`, `dirt_zone_mask`, `inland_sand_mask`, `rock_field_mask`, `dirt_zone_distance_field`, `grass_zone_distance_field` |
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
| `AnchorLayout`, `WorldMasks`, `mapgen::pipeline`, `mapgen::wfc_adapter`, `mapgen::resources`, `rock_fields` | `GeneratedWorldLayout` を root Resource に包んで startup / map spawn / regrowth 初期化へ接続する app shell |

**判断基準**: root 固有の `GameAssets` / UI / plugin wiring が必要なら `src/` に残す。
shared crate 型と `WorldMap` access だけで閉じるなら hw_world に置く。

`WorldMapRead` / `WorldMapWrite` はシステム引数として使用する:
```rust
// システム引数でワールドマップにアクセス
fn my_system(world_map: WorldMapRead) {
    let path = world_map.find_path(from, to);
}
```
