# world — ワールドマップアクセス層

## 役割

`hw_world` クレートが提供するワールドデータへのアクセス層。
ルートクレートからワールドマップを操作するためのシステムと、マップ座標ユーティリティをここに配置する。

## ディレクトリ構成

| ディレクトリ | 内容 |
|---|---|
| `map/` | ワールドマップの初期化・アクセス・地形描画の app shell |

## map/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `spawn.rs` | マップエンティティのスポーン。`GeneratedWorldLayoutResource` の `terrain_tiles` を 3D タイルとしてスポーン（startup 本経路） |

## 地形生成・経路探索について

地形生成アルゴリズムと A* 経路探索は `hw_world` クレートに実装されている。
詳細は `crates/hw_world/README.md` を参照。

---

## hw_world との境界

| 層 | 内容 |
|---|---|
| `hw_world` クレート | 純粋アルゴリズム（A*・地形生成・座標変換）と `WorldMap` 型 |
| `src/world/map/` | app shell facade と Bevy 統合（スポーン・タイル設定） |

### hw_world に置かれているもの

- `WorldMap` 型と全データ構造
- `find_path`, `can_reach_target` などの A* アルゴリズム
- `grid_to_world`, `world_to_grid` などの座標変換
- `generate_base_terrain_tiles`, `generate_world_layout`, `generate_fixed_river_tiles` などの地形生成
- `find_nearest_walkable_grid` などの空間クエリ
- `AnchorLayout`, `WorldMasks` などの生成中間データ

### src/ に置かれているもの（app shell / Bevy 統合層）

```rust
// src/world/map/mod.rs
// root 側の正規 public path をまとめる facade
pub use hw_world::{WorldMapRead, WorldMapWrite, TerrainType, generate_fixed_river_tiles};
```

| ファイル | hw_world ではなく src/ にある理由 |
|---|---|
| `mod.rs` | `WorldMapRead` / `WorldMapWrite` と layout 定数の root facade |
| `spawn.rs` | `Commands` と `Terrain3dHandles` を使った地形スポーン。`PostStartup` で `spawn_map` が呼ばれ、`setup()` で挿入済みの `GeneratedWorldLayoutResource` を消費する |
