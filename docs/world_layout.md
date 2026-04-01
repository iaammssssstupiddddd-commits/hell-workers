# ワールドマップ仕様書

100x100の固定レイアウトを持つワールドマップの仕様です。地形と資源の配置は、ゲームの拠点としての機能を果たすよう設計されています。

## 基本設定

| 項目 | 値 | 定数名 |
|:--|:--|:--|
| マップサイズ | 100 x 100 | `MAP_WIDTH` / `MAP_HEIGHT` |
| タイルサイズ | 32.0 px | `TILE_SIZE` |

## 地形タイプ (`TerrainType`)

| 地形 | 通行 | 説明 |
|:--|:--|:--|
| **Grass** | ○ | 基本となる地面。 |
| **Dirt** | ○ | 疎らに点在する土。岩を破壊した跡地もこの地形になります。 |
| **Sand** | ○ | 川の両岸に広がる砂浜。 |
| **River** | × | マップを**横断**する水場。物理的な障害物として機能します。 |

## 地形生成アルゴリズム

### 蛇行川 (`River`)
- **手法**: パーリンノイズ（1D）によるY軸オフセットの計算。
- **方向**: マップを西から東へ**横断**します。
- **シード**: 42 (固定)
- **川幅**: 5タイル (`RIVER_WIDTH`)
- **生成ロジック**: 純粋な生成処理は `crates/hw_world/src/river.rs` にあり、`crates/bevy_app/src/world/mod.rs` の inline module `pub mod river { pub use hw_world::river::...; }` として re-export されています。

### 砂浜 (`Sand`)
- 川のタイルから上下2タイル (`SAND_WIDTH`) の範囲に自動生成。

## 資源配置と再生システム

資源は特定のエリアに集中して配置され、一部は時間経過とともに再生します。

### 1. 森林エリア (ForestZone)
- **対象**: 木 (`Tree`)
- **配置**: マップ北西（左上）に広がる森林地。
- **再生ロジック**: `RegrowthManager` によって管理。伐採された箇所に一定時間（デフォルト60秒）経過後、新しい木が再生します。
- **物理特性**: 木は物理的な障害物であり、魂（地上ユニット）は通り抜けることができません。

### 2. 岩石エリア (RockArea)
- **対象**: 岩 (`Rock`)
- **配置**: マップ南東（右下）の険しいエリア。
- **特性**: 岩は強固な障害物です。破壊（採掘）には時間がかかりますが、跡地は `TerrainType::Dirt` に変化し、通行可能になります。

### 3. 初期資源
- ゲーム開始時、中央の拠点付近に少量の木材アイテム (`ResourceItem(Wood)`) が配置されます。

## 座標変換 (Coordinate System)

| 関数 | 用途 |
|:--|:--|
| `hw_world::world_to_grid(Vec2) -> (i32, i32)` | ワールド座標 → グリッド座標 |
| `hw_world::grid_to_world(i32, i32) -> Vec2` | グリッド座標 → ワールド座標 |
| `WorldMap::pos_to_idx(i32, i32) -> Option<usize>` | グリッド → フラット配列インデックス |
| `hw_world::snap_to_grid_center(Vec2) -> Vec2` | タイル中心にスナップ |
| `hw_world::snap_to_grid_edge(Vec2) -> Vec2` | タイル境界線にスナップ（ゾーン配置等） |

- **原点**: グリッド (0, 0) = マップ中心 = ワールド座標 (0.0, 0.0)
- **タイル中心**: ワールド整数座標がタイル中心と一致（1px の狂いなし）
- **フラット配列インデックス**: `y * MAP_WIDTH + x`（行優先）
- **境界到達パス**: 2x2以上の建築物など占有領域への隣接パスは `find_path_to_boundary` を使用

実装詳細:
- 純粋な座標変換は `crates/hw_world/src/coords.rs`
- root 側の `WorldMap::{world_to_grid, grid_to_world, snap_*}` は互換 wrapper

## 物理衝突と通行制御

- **通行不可オブジェクト**: 木、岩、川は物理的な障害物です。
- **スライディング衝突**: 魂が障害物に斜めにぶつかった際、壁に沿って滑るように移動する物理解決が実装されています。
- **8方向パス検索と高度な制御**:
    - **8方向移動**: 上下左右に加え、斜め方向への移動が可能です。
    - **角抜け防止**: 斜め移動時、壁の角をすり抜けないよう判定を行っています。
    - **探索共通核**: `find_path_with_policy` を内部共通核として、通常探索・隣接探索・境界探索の差分をポリシー関数で切り替えています。
    - **経路平滑化**: 現在は無効化されており、グリッド経路をそのまま使用しています。
    - **重複エントリの抑制**: A* の `BinaryHeap` で古いエントリを pop 時にスキップし、不要な展開を削減しています。
    - **再計算抑制**: Soul 側では失敗時クールダウンとフレーム当たり探索件数制限により、再探索スパイクを抑制しています。
    - **境界到達 (Boundary Reaching)**: 2x2以上の建築物など、ターゲット領域に入り込まずにその境界（隣接マス）で停止する高度なパス探索ロジック（`find_path_to_boundary`）を実装しています。対象占有領域は集合 membership として扱い、開始地点が対象内にある場合は最寄りの外側歩行マスへの短い脱出パスを返します。通常時は目標領域へ入る最初の1歩手前で経路を切り詰めます。

## 関連ファイル
- `crates/bevy_app/src/world/map/`: マップデータ構造（mod）・レイアウト定数（layout）・生成システム（spawn）
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`: `Terrain3dHandles` リソース（タイルメッシュ・4種 SectionMaterial ハンドル）
- `crates/bevy_app/src/systems/visual/terrain_material.rs`: 障害物除去後のテレインマテリアル差し替えシステム
- [`../crates/hw_world/src/river.rs`](../crates/hw_world/src/river.rs): 川生成アルゴリズム
- [`../crates/hw_world/src/coords.rs`](../crates/hw_world/src/coords.rs): 座標変換
- [`../crates/bevy_app/src/world/regrowth.rs`](../crates/bevy_app/src/world/regrowth.rs): 木の再生システムの app shell
- `crates/bevy_app/src/world/mod.rs` (inline `pub mod pathfinding`): 通行制御を伴うパス検索の互換層（`hw_world::pathfinding` への re-export）

## 地形レンダリング（MS-3-4 完了・2026-03-29）

地形タイルは **Camera3d → RtT** パイプラインのみで描画される。`Camera2d` 側のゲーム内地形描画は完全に除去済み。

- **タイルメッシュ**: `Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)` を全タイルで共有。
- **マテリアル**: `Terrain3dHandles`（`SectionMaterial` × 4種）を `TerrainType` に応じて割り当て。地形用は `make_terrain_section_material`（`crates/hw_visual/src/material/section_material.rs`）で生成し、`albedo_uv_mode = 1.0` によりフラグメントで **ワールド XZ ベースのアルベド UV**（タイル境界で連続）を使う。建物・壁は `albedo_uv_mode = 0.0` のままメッシュ UV。
- **テクスチャサンプラ**: 地形 4 枚（`grass` / `dirt` / `sand_terrain` / `river`）は `asset_catalog.rs` で `AddressMode::Repeat` 付きロード。ワールド UV が 0〜1 を超える前提。
- **川**: `uv_scroll_speed` のみ非ゼロ（U 方向・時間でオフセット）。見た目は画面上 **左→右**の流れ（符号はシェーダ側で調整済み）。
- **草のみ A3（任意の単調さ緩和）**: `uv_distort_strength`（UV 空間の低周波歪み、`TERRAIN_GRASS_UV_DISTORT_STRENGTH`）と `brightness_variation_strength`（`base_color.rgb` への低周波乗算、`TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH`）。土・砂・川は両方 `0.0`。
- **uniform レイアウト**: `SectionMaterialUniform` にパディング用の `f32` を並べる場合、`[f32; N]` 配列は encase の uniform 制約で使えない（ストライド 16 必須）。**個別の `f32` フィールド**で並べる（`section_material.rs` 参照）。
- **レイヤー**: `building_3d_render_layers()`（`LAYER_3D` + `LAYER_3D_SHADOW_RECEIVER`）で他の 3D エンティティと同レイヤー。
- **Transform**: `from_xyz(x, 0.0, -y)`（Y=0 が地面平面）。
- **障害物除去後の差し替え**: `hw_world::obstacle_cleanup_system` が `TerrainChangedEvent`（`Message`）を発行 → `bevy_app::terrain_material_sync_system` が受信してマテリアルを Dirt に更新。
- **廃止**: `TerrainBorder` / `terrain_border.rs` / `hw_world::borders` は MS-3-4 で除去済み。`TerrainType::z_layer()` も同様に除去済み。

### 2D 前景カメラ（composite より手前の `LAYER_2D`）

RtT composite が全画面を覆うため、`startup_systems::setup` で **`WorldForeground2dCamera`**（`Camera2d`、`order=2`、`LAYER_2D`、クリアなし）が同レイヤーを再描画する。`PanCamera` は `MainCamera` のみ更新するため、`sync_world_foreground_2d_camera_system`（`camera_sync.rs`）が **毎フレーム `MainCamera` と同一の `Transform` / `Camera::is_active`** を前景カメラへコピーし、パン・ズームと連動させる。
