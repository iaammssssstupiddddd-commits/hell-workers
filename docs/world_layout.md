# ワールドマップ仕様書 (2026-01-24)

100x100の固定レイアウトを持つワールドマップの仕様です。

## 基本設定

| 項目 | 値 | 定数名 |
|:--|:--|:--|
| マップサイズ | 100 x 100 | `MAP_WIDTH` / `MAP_HEIGHT` |
| タイルサイズ | 32.0 px | `TILE_SIZE` |

## 地形タイプ (`TerrainType`)

| 地形 | 通行 | 説明 |
|:--|:--|:--|
| **Grass** | ○ | 基本となる地面。 |
| **Dirt** | ○ | 疎らに点在する土。 |
| **Sand** | ○ | 川の両岸に広がる砂浜。 |
| **Stone** | × | 右上の岩石エリアに点在する岩の地面。 |
| **River** | × | マップを縦断する水場。将来的に水汲みが可能。 |

## 地形生成アルゴリズム

### 蛇行川 (`River`)
- **手法**: パーリンノイズ（1D）によるX軸オフセットの計算。
- **シード**: 42 (固定)
- **川幅**: 5タイル (`RIVER_WIDTH`)
- **生成ロジック**: `src/world/river.rs` に実装。

### 砂浜 (`Sand`)
- 川のタイルから左右2タイル (`SAND_WIDTH`) の範囲に自動生成。

## 初期資源配置

資源は特定のエリアに固定座標で配置されます。座標定義は `src/world/map.rs` に集約されています。

### 1. 森林エリア (左上付近)
- **対象**: 木 (`Tree`)
- **数量**: 20本
- **座標定義**: `TREE_POSITIONS`

### 2. 岩石エリア (右上付近)
- **対象**: 岩 (`Rock`)
- **数量**: 15個
- **座標定義**: `ROCK_POSITIONS`
- **地形パッチ**: このエリアには `TerrainType::Stone` が点在します。

### 3. 中央エリア
- **対象**: 木材アイテム (`ResourceItem(Wood)`)
- **数量**: 5個
- **座標定義**: `INITIAL_WOOD_POSITIONS`
- ゲーム開始時のチュートリアル用資材として配置。

## 関連ファイル
- [map.rs](file:///home/iaamm/projects/hell-workers/src/world/map.rs): マップデータ構造と生成システム
- [river.rs](file:///home/iaamm/projects/hell-workers/src/world/river.rs): 川生成アルゴリズム
- [logistics.rs](file:///home/iaamm/projects/hell-workers/src/systems/logistics.rs): 資源配置システム
- [constants.rs](file:///home/iaamm/projects/hell-workers/src/constants.rs): グローバル定数
