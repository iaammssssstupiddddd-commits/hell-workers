# hw_world — ワールド・地形・経路探索

## 役割

ゲームワールドの地形生成・管理、座標変換ユーティリティ、A* 経路探索を提供するクレート。
ワールドデータの読み取り（クエリ）と経路計算のみを行い、**エンティティのスポーンは行わない**。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `coords.rs` | 座標変換 (`grid_to_world`, `world_to_grid`, `snap_to_grid_*`, `idx_to_pos`) |
| `map.rs` | `WorldMap` — 地形・歩行可能性・建物データの保持 |
| `terrain.rs` | `TerrainType` enum (Water, Sand, Rock, Grass, ...) |
| `mapgen.rs` | `generate_base_terrain_tiles()` — Perlin ノイズ地形生成 |
| `river.rs` | `generate_fixed_river_tiles()` と砂地生成 |
| `borders.rs` | マップ境界仕様 (`TerrainBorderSpec`) |
| `layout.rs` | ワールドレイアウト定数 (木・岩・木材の初期位置, 川の範囲) |
| `regrowth.rs` | 森林再生システム (`ForestZone`, 周期的な木スポーン) |
| `pathfinding.rs` | A* 経路探索（下記詳細参照） |
| `query.rs` | 環境クエリ (`find_nearest_river_grid`, `find_nearest_walkable_grid`) |
| `spatial.rs` | ワールド向け `SpatialGridOps` 実装 |
| `spawn.rs` | スポーンヘルパー (`find_nearby_walkable_grid`, `pick_random_walkable_grid_in_rect`) |

## 経路探索 (pathfinding.rs)

### 主要関数

```rust
find_path(map, from, to)                          // 基本 A*
find_path_to_adjacent(map, from, target)          // 隣接タイルへの探索
find_path_to_boundary(map, from)                  // マップ境界への探索
find_path_with_policy(map, from, goal_policy)     // カスタムゴール条件
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

---

## src/ との境界

hw_world は**純粋なワールドアルゴリズム**を提供する。
Bevy との統合（`SystemParam`・エンティティスポーン）は `src/world/` に実装する。

| hw_world に置くもの | src/world/ に置くもの |
|---|---|
| `WorldMap` 型と全データ構造 | `WorldMapRead` / `WorldMapWrite` (`SystemParam` ラッパー) |
| A* 経路探索関数群 | マップエンティティのスポーン (`spawn.rs`) |
| 座標変換関数 (`grid_to_world` 等) | 地形境界タイルへのコンポーネント付与 (`terrain_border.rs`) |
| 地形生成関数 (`generate_base_terrain_tiles` 等) | — |
| `tree_regrowth_system` などの純粋システム | — |

**判断基準**: `Commands`・`Entity`・`Res<T>` など Bevy ECS API を使う必要があるなら src/ 側に置く。
`WorldMap` の読み書きだけなら hw_world 内で完結できる。

`WorldMapRead` / `WorldMapWrite` はシステム引数として使用する:
```rust
// システム引数でワールドマップにアクセス
fn my_system(world_map: WorldMapRead) {
    let path = world_map.find_path(from, to);
}
```
