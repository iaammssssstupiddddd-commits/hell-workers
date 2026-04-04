# ワールドマップ仕様書

100x100 の固定グリッドを持つワールドマップの仕様です。`Site/Yard` は固定アンカーのまま、**本番のマップ生成経路**（`generate_world_layout`）では **WFC ソルバー**（gridbugs `wfc`）で `terrain_tiles` を生成し、**各試行ごとに** `mapgen::validate::lightweight_validate()` で invariant と必須資源到達を検証する（MS-WFC-2a/2b/2c/2d/2e 完了）。`debug` / テストビルドでは `debug_validate()` が追加診断を `eprintln!` する。

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

## 地形生成の現状

### 現行の生成経路

- legacy 純粋関数: `hw_world::generate_base_terrain_tiles()`
  - 固定 River 矩形と簡易 Dirt/Grass パターンを返す旧経路（`GeneratedWorldLayout::stub` や visual 用のレガシー経路で使用）
- **本番相当の経路**: `hw_world::generate_world_layout(master_seed)`
  - `AnchorLayout::fixed()` と `WorldMasks::from_anchor` → `fill_river_from_seed` → `fill_sand_from_river_seed` でアンカー・川・砂マスクを確定
  - `mapgen::wfc_adapter::run_wfc`（`RunOwn::new_wrap_forbid` + `collapse`）で地形を生成し、`post_process_tiles` が `final_sand_mask` と一致するよう Sand を補正
  - 各試行で `mapgen::validate::lightweight_validate()` に通過したレイアウトのみ採用。通過時は到達確認済み `ResourceSpawnCandidates` を `GeneratedWorldLayout.resource_spawn_candidates` に格納する
  - WFC 成功でも validate 失敗なら次 attempt。全試行で通過できなければ `fallback_terrain`（River と `final_sand_mask` を維持し、他は Grass）。fallback レイアウトは lightweight を通さない（`used_fallback == true`）
  - `crates/bevy_app/src/world/map/spawn.rs` は、MS-WFC-4 の本統合前の暫定措置としてこの結果の `terrain_tiles` を描画する

### 川 (`River`)

- **方向**: マップを西から東へ横断する
- **生成ロジック**: `crates/hw_world/src/river.rs` の `generate_river_mask(seed, anchor_mask, river_protection_band)`
- **出力**:
  - `WorldMasks::river_mask`
  - `WorldMasks::river_centerline`
- **制約**:
  - `Site/Yard` のアンカーセルには進入しない
  - `river_protection_band` を侵さない
- **seed**:
  - 同一 seed では同一形状
  - `crates/bevy_app/src/world/map/spawn.rs` の暫定プレビューでは `HELL_WORKERS_WORLDGEN_SEED=<u64>` を使う
  - 未指定時は起動ごとにランダム seed を採用する

### 砂浜 (`Sand`)

- **`generate_world_layout` 経路**: `WorldMasks::fill_sand_from_river_seed()` が `river_mask` から **river distance field** を計算し、River の **8 近傍 shoreline shell を dist=1** として扱う。そこから `dist 1..=2` の **base shoreline** と `dist==1` frontier からの **bounded growth** を合成して `sand_candidate_mask` を作る。seed 由来の non-sand carve を差し引いた `final_sand_mask` を決める。`wfc_adapter::post_process_tiles` と `fallback_terrain` はこの `final_sand_mask` を最終 `Sand` 分布として反映する
- レガシー経路 `generate_base_terrain_tiles` は従来どおり `generate_sand_tiles` で River 帯から導出

## 資源配置と再生システム

現状は「固定アンカー + 一部固定物 + その他自動生成」へ移行中です。

### 1. 固定アンカー

- `Site` / `Yard` は `crates/hw_world/src/anchor.rs` の `AnchorLayout::fixed()` で決定する
- `Site` は中央、`Yard` はその東隣に固定配置される
- `Site/Yard` 内の地形は `Grass` または `Dirt` のみを許可する設計

### 2. Yard 内固定物

- 初期木材は `AnchorLayout::initial_wood_positions` に固定配置
- 猫車置き場は `AnchorLayout::wheelbarrow_parking` の 2x2 footprint に固定配置

### 3. 木・岩・再生

- 木・岩の完全自動生成と `forest_regrowth_zones` の本統合は WFC 系マイルストーンで段階導入中
- 現在の画面描画で確認できるのは主に地形プレビューであり、初期木・岩・regrowth はまだ旧 startup 経路が残る
- 木や岩は物理的な障害物として機能し、岩の跡地は `TerrainType::Dirt` に変化する

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
- `crates/bevy_app/src/world/map/`: root 側の app shell。`spawn.rs` は暫定的に `generate_world_layout()` の地形を描画する
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`: `Terrain3dHandles` リソース（タイルメッシュ・4種 SectionMaterial ハンドル）
- `crates/bevy_app/src/systems/visual/terrain_material.rs`: 障害物除去後のテレインマテリアル差し替えシステム
- [`../crates/hw_world/src/anchor.rs`](../crates/hw_world/src/anchor.rs): `Site/Yard` 固定アンカー定義
- [`../crates/hw_world/src/world_masks.rs`](../crates/hw_world/src/world_masks.rs): anchor/protection-band/river/sand の各マスク
- [`../crates/hw_world/src/mapgen.rs`](../crates/hw_world/src/mapgen.rs): `generate_base_terrain_tiles()` と `generate_world_layout()`
- [`../crates/hw_world/src/mapgen/validate.rs`](../crates/hw_world/src/mapgen/validate.rs): 生成後バリデータ（`lightweight_validate`, `debug_validate`）
- [`../crates/hw_world/src/mapgen/wfc_adapter.rs`](../crates/hw_world/src/mapgen/wfc_adapter.rs): WFC ソルバー統合（`run_wfc`, `post_process_tiles`, `fallback_terrain`）
- [`../crates/hw_world/src/river.rs`](../crates/hw_world/src/river.rs): seed 付き川マスク生成と砂地導出
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
