# world — ワールドマップアクセス層

## 役割

`hw_world` クレートが提供するワールドデータへのアクセス層。
ルートクレートからワールドマップを操作するためのシステムと、マップ座標ユーティリティをここに配置する。

## ディレクトリ構成

| ディレクトリ | 内容 |
|---|---|
| `map/` | ワールドマップの初期化・アクセス・地形境界 |

## map/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `access.rs` | `WorldMap` リソースへのアクセスヘルパー |
| `layout.rs` | マップレイアウト定数（re-export） |
| `spawn.rs` | マップエンティティのスポーン処理 |
| `terrain_border.rs` | 地形境界タイルの設定 |

## 地形生成・経路探索について

地形生成アルゴリズム（Perlin ノイズ・川生成）と A* 経路探索は `hw_world` クレートに実装されている。
詳細は `crates/hw_world/README.md` を参照。

---

## hw_world との境界

| 層 | 内容 |
|---|---|
| `hw_world` クレート | 純粋アルゴリズム（A*・地形生成・座標変換）と `WorldMap` 型 |
| `src/world/map/` | Bevy 統合（`SystemParam` ラッパー・スポーン・タイル設定） |

### hw_world に置かれているもの

- `WorldMap` 型と全データ構造
- `find_path`, `can_reach_target` などの A* アルゴリズム
- `grid_to_world`, `world_to_grid` などの座標変換
- `generate_base_terrain_tiles`, `generate_fixed_river_tiles` などの地形生成
- `find_nearest_walkable_grid` などの空間クエリ

### src/ に置かれているもの（Bevy 統合層）

```rust
// src/world/map/access.rs
// WorldMap を SystemParam として扱うラッパー
#[derive(SystemParam)]
pub struct WorldMapRead<'w> {
    world_map: Res<'w, WorldMap>,
}
```

| ファイル | hw_world ではなく src/ にある理由 |
|---|---|
| `access.rs` | `WorldMapRead` / `WorldMapWrite` は Bevy `SystemParam` — エンジン固有 |
| `spawn.rs` | `Commands` によるエンティティスポーンが必要 |
| `terrain_border.rs` | タイルエンティティへのコンポーネント付与が必要 |
| `layout.rs` | hw_world 定数の re-export のみ |
