# ワールドマップ仕様書

100x100 の固定グリッドを持つワールドマップの仕様です。`Site/Yard` は固定アンカーのまま、**本番のマップ生成経路**（`generate_world_layout`）では **WFC ソルバー**（gridbugs `wfc`）で `terrain_tiles` を生成し、**各試行ごとに** `mapgen::validate::lightweight_validate()` で invariant と必須資源到達を検証する（MS-WFC-2a/2b/2c/2d/2e/2.5 完了）。`debug` / テストビルドでは `debug_validate()` が追加診断を `eprintln!` する。生成パイプライン自体の詳細は [`map_generation.md`](map_generation.md) を参照。

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
  - 固定アンカー、川、砂浜、地形ゾーン、岩場を seed から確定した上で WFC で `terrain_tiles` を生成する
  - validate と資源配置の両方を通ったレイアウトだけを採用する
  - startup の地形描画・初期木/岩・初期木材・猫車置き場・regrowth 初期化は同じ `GeneratedWorldLayout` を共有する
  - retry / fallback を含む処理順と詳細契約は [`map_generation.md`](map_generation.md) を参照

### 川 (`River`)

- **方向**: マップを西から東へ横断する
- **制約**:
  - `Site/Yard` のアンカーセルには進入しない
  - `river_protection_band` を侵さない
- **アンカーとの縦関係（本番）**: Site/Yard の縦位置は川の南端基準で seed ごとに調整される。具体的な配置契約は下記「固定アンカー」と [`map_generation.md`](map_generation.md) を参照。
- **seed**:
  - 同一 seed では同一形状
  - ゲーム起動時は `HELL_WORKERS_WORLDGEN_SEED=<u64>` で `generate_world_layout` の master seed を指定できる（`GeneratedWorldLayoutResource` 経由で地形・初期スポーン・regrowth が共有される）
  - 未指定時は起動ごとにランダム seed を採用する

### 砂浜 (`Sand`)

- **河岸の砂浜**: River 沿いに連続した `Sand` 帯を作る。最終的な合法領域は `final_sand_mask` で表現される
- **内陸砂 (`inland_sand_mask`)**: MS-WFC-2.5 以降、草地寄りの領域内に小さな孤立 Sand パッチを作れる。河岸の砂浜とは別経路で、`final_sand_mask` とは排他関係を保つ
- レガシー経路 `generate_base_terrain_tiles` は従来どおり `generate_sand_tiles` で River 帯から導出
- 正確な生成アルゴリズムは [`map_generation.md`](map_generation.md) を参照

## 地形ゾーン（MS-WFC-2.5）

`generate_world_layout` では、川と砂浜の確定後に terrain zone を導入して `Grass` / `Dirt` の偏りと内陸砂の配置を調整する。

### grass_zone_mask / dirt_zone_mask

アンカーと川から十分離れた領域に、seed 依存の Grass / Dirt バイアスを導入する。

| マスク | 役割 |
|:--|:--|
| `grass_zone_mask` | 地形を Grass 寄りに寄せる領域 |
| `dirt_zone_mask` | 地形を Dirt 寄りに寄せる領域 |

- 両ゾーンは重ならず、アンカー・River・砂浜帯を侵さない
- zone 内だけでなく、境界付近と完全中立領域にも弱いバイアスをかけてパッチの断絶感を減らす
- ゾーン生成の距離場や確率パラメータは [`map_generation.md`](map_generation.md) を参照

### inland_sand_mask

- `grass_zone_mask` 内にだけ存在できる小さな Sand パッチ
- 河岸の砂浜とは別物で、地形の単調さを崩すために使う
- `final_sand_mask` とは排他で、validator は `final_sand_mask || inland_sand_mask` を合法 Sand 領域として扱う

### rock_field_mask（MS-WFC-3b）

- east-side に寄せた岩場候補領域
- 川・砂・内陸砂・アンカー帯を避けて生成される
- `rock_field_mask` 上の terrain は Dirt に揃えられ、初期岩配置の母集団にも使われる

## 資源配置と再生システム

現行のマップは「固定アンカー + Yard 内固定物 + seed 由来の木/岩自動生成」で構成される。

### 1. 固定アンカー

- `Site` / `Yard` は `crates/hw_world/src/anchor.rs` の `AnchorLayout` で決定する
- **本番の地形生成**では `AnchorLayout::aligned_to_worldgen_seed(master_seed)` が使われ、Y 位置は川の南端に合わせて seed ごとに調整される。詳細な決定手順は [`map_generation.md`](map_generation.md) を参照
- `Site` はマップ中央付近、`Yard` はその東隣に配置される
- `Site/Yard` 内の地形は `Grass` または `Dirt` のみを許可する設計

### 2. Yard 内固定物

- 初期木材は `AnchorLayout::initial_wood_positions` に固定配置
- 猫車置き場は `AnchorLayout::wheelbarrow_parking` の 2x2 footprint に固定配置

### 3. 木・岩・再生

- 木は `grass_zone_mask` ベースの `forest_regrowth_zones` から procedural に生成される
- 岩は `rock_field_mask` ベースで procedural に生成される
- root app startup は `GeneratedWorldLayout` を resource 化し、地形描画・初期木・初期岩・初期木材・猫車置き場・regrowth 初期化が同じ layout を共有する
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

## 論理タイルデータの真実源（Truth Source）

地形タイル情報は以下の配列データが唯一の真実源である。ECS entity は描画・ロジック anchor を担うに過ぎない。

| データ | 型 | 更新タイミング | 備考 |
|:--|:--|:--|:--|
| `WorldMap.tiles` | `Vec<TerrainType>` | startup + obstacle 除去時 | 論理地形の truth source |
| `WorldMap.tile_entities` | `Vec<Option<Entity>>` | startup のみ | 論理 anchor の lookup 層。描画 entity ではない |
| `GeneratedWorldLayout.terrain_tiles` | `&[TerrainType]` | startup のみ（snapshot） | worldgen 結果。`WorldMap.tiles` の初期値として使用 |

**`WorldMap.tile_entities` / `tile_entity_at_idx()` について**:
- `tile_entities` に登録される `Tile` entity は描画コンポーネント（`Mesh3d` / `MeshMaterial3d`）を持たない論理 anchor である。
- ランタイムで `tile_entity_at_idx()` を呼ぶ箇所は `crates/hw_familiar_ai/.../haul/direct_collect.rs:147` の **1 箇所のみ**。用途は tile entity 上の `Designation` + `TaskWorkers` コンポーネントの Query（収集可否判定）。
- 地形描画は chunk entity（`TerrainChunk`）が担う。`tile_entities` は将来の別フェーズで廃止検討。

**将来の BiomeType 追加方針**:
- biome タイプは `WorldMap.tiles` と並列に `Vec<BiomeType>` を `WorldMap` に追加する形で格納する。
- 描画用には `TerrainFeatureMap` と同様の独立した `BiomeIdMap` texture（`R8Unorm`）を startup で生成し、`TerrainSurfaceMaterial` の uniform に追加するだけで chunk entity 自体の変更は不要。

## 関連ファイル
- `crates/bevy_app/src/world/map/`: root 側の app shell。`spawn.rs` は `GeneratedWorldLayout` の `terrain_tiles` から地形論理タイル anchor をスポーンし、chunk render は `spawn_terrain_chunks` が担う（`prepare_generated_world_layout_resource` と同一 layout）
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`: `Terrain3dHandles` リソース（共有 `TerrainSurfaceMaterial` ハンドル）
- `crates/bevy_app/src/systems/visual/terrain_material.rs`: 障害物除去後のテレインマテリアル差し替えシステム
- [`../crates/hw_world/src/anchor.rs`](../crates/hw_world/src/anchor.rs): `Site/Yard` 固定アンカー定義
- [`../crates/hw_world/src/world_masks.rs`](../crates/hw_world/src/world_masks.rs): anchor/protection-band/river/sand/terrain-zone の各マスク
- [`../crates/hw_world/src/terrain_zones.rs`](../crates/hw_world/src/terrain_zones.rs): terrain zone mask と inland sand mask の生成（MS-WFC-2.5）
- [`../crates/hw_world/src/rock_fields.rs`](../crates/hw_world/src/rock_fields.rs): 岩場マスクの deterministic 生成（MS-WFC-3b）
- [`../crates/hw_world/src/mapgen/mod.rs`](../crates/hw_world/src/mapgen/mod.rs): モジュールルート（`generate_base_terrain_tiles`、`generate_world_layout` の公開）
- [`../crates/hw_world/src/mapgen/pipeline.rs`](../crates/hw_world/src/mapgen/pipeline.rs): `generate_world_layout()` のオーケストレーション本体
- [`../crates/hw_world/src/mapgen/validate/mod.rs`](../crates/hw_world/src/mapgen/validate/mod.rs): validate 公開面（`lightweight_validate`, `debug_validate`）
- [`../crates/hw_world/src/mapgen/validate/terrain.rs`](../crates/hw_world/src/mapgen/validate/terrain.rs): 地形フェーズ validate（Site/Yard・必須資源到達）
- [`../crates/hw_world/src/mapgen/validate/post_resource.rs`](../crates/hw_world/src/mapgen/validate/post_resource.rs): 資源配置後 validate（木・岩を障害物として重ねた導線再確認）
- [`../crates/hw_world/src/mapgen/validate/debug.rs`](../crates/hw_world/src/mapgen/validate/debug.rs): debug / test ビルド専用診断
- [`../crates/hw_world/src/mapgen/wfc_adapter.rs`](../crates/hw_world/src/mapgen/wfc_adapter.rs): WFC ソルバー統合（`run_wfc`, `post_process_tiles`, `fallback_terrain`）
- [`../crates/hw_world/src/river.rs`](../crates/hw_world/src/river.rs): seed 付き川マスク生成と砂地導出
- [`../crates/hw_world/src/coords.rs`](../crates/hw_world/src/coords.rs): 座標変換
- [`../crates/bevy_app/src/world/regrowth.rs`](../crates/bevy_app/src/world/regrowth.rs): 木の再生システムの app shell
- `crates/bevy_app/src/world/mod.rs` (inline `pub mod pathfinding`): 通行制御を伴うパス検索の互換層（`hw_world::pathfinding` への re-export）

## 地形レンダリング（chunk renderer 導入済み）

地形タイルは **Camera3d → RtT** パイプラインのみで描画される。`Camera2d` 側のゲーム内地形描画は完全に除去済み。

地形描画は **chunk 単位の `TerrainChunk` entity** で行う（per-tile render entity は廃止）。

- **Chunk 構成**: `CHUNK_TILES = 16`（16×16 タイル/chunk）。100×100 マップ → 7×7 = **49 chunk entity**。辺端は 4 tile 幅の端数 chunk が生じる。
- **Chunk entity**: `TerrainChunk { cx, cy }` + `Mesh3d` + `Transform` + `building_3d_render_layers()` を持ち、地形 material は LOD に応じて `MeshMaterial3d<TerrainSurfaceMaterial>` / `MeshMaterial3d<TerrainSurfaceMaterialLod1Lite>` / `MeshMaterial3d<TerrainSurfaceMaterialLod2>` のいずれか一方が付く。chunk の中心ワールド座標に配置。
- **Chunk mesh**: `Plane3d::default().mesh().size(w * TILE_SIZE, h * TILE_SIZE)`。フルチャンク（512×512wu）、端数チャンク（128×512wu 等）。
- **Tile anchor entity**: 10,000 個の `Tile` entity（`Tile` component + `Transform`）は描画コンポーネントなしで存続。`WorldMap.tile_entities` に登録され、Familiar AI の収集可否判定（`direct_collect.rs`）から `Designation` / `TaskWorkers` を取得する論理 anchor として機能する。
- **マテリアル**: `Terrain3dHandles` は `lod1: Handle<TerrainSurfaceMaterial>`、`lod1_lite: Handle<TerrainSurfaceMaterialLod1Lite>`、`lod2: Handle<TerrainSurfaceMaterialLod2>` を保持する。`terrain_lod_switch_system` が 49 chunk の `MeshMaterial3d` component を差し替え、現 runtime は `Lod1 / Lod1Lite / Lod2` を使う。`Lod0` は将来のリッチビジュアル用に予約で未使用。LOD1 shader は現行フル品質、LOD1-lite shader は **曲線境界と 4-corner bilinear を維持しつつ**、macro noise / domain warp / river scroll を落とした中景向け簡略版で、`boundary_proximity_mask` により境界外画素を early-out する。砂浜の shoreline tone は LOD1 と揃え、`shoreline_detail` は Sand 経路だけ保持する。LOD2 shader は **`boundary_mask` の nearest region を正本にして曲線境界を維持しつつ**、4-corner bilinear・domain warp・river scroll・shoreline detail を落とし、albedo UV を量子化して低解像度 texture 相当の見た目へ簡略化する。chunk 境界での継ぎ目は全 LOD とも world-space 参照のため発生しない。建物・壁は引き続き `SectionMaterial`。
- **terrain id map**: startup の `build_terrain_id_map` が `GeneratedWorldLayout.terrain_tiles` から `R8Unorm` の `TerrainIdMap` を生成する。0 / 85 / 170 / 255 を grass / dirt / sand / river として encode し、shader 側では `round(raw * 3.0)` で terrain id に戻す。`ClampToEdge + Nearest`。
- **テクスチャサンプラ**: 地形 4 枚（`grass` / `dirt` / `sand_terrain` / `river`）は `asset_catalog.rs` で `AddressMode::Repeat` 付きロード。ワールド UV が 0〜1 を超える前提。
- **feature map**: startup の `build_terrain_feature_map` が `GeneratedWorldLayout.masks` から `Rgba8Unorm` の `TerrainFeatureMap` を生成する。R=`shore sand`、G=`inland sand`、B=`rock field`、A=`zone bias`（grass zone / neutral / dirt zone）。こちらは worldgen snapshot を表す static bake で、runtime では更新しない。
- **macro noise / overlay**: `terrain_macro_noise.png` と terrain 種別ごとの `*_macro_overlay.png` を読み、`domain warp` と明度ムラに使う。Grass / Dirt / Sand は低周波の面変化を加え、共有 material 化後も world-space UV ベースの反復感崩しを維持する。
- **川**: `river_flow_noise.png` と `river_normal_like.png` を使い、river セルだけ左→右スクロールと V 軸ゆらぎを加える。境界ブレンドは river を含む全ペアには掛けず、**`river↔sand` の組み合わせだけ**を対象にする。
- **feature tint / roughness**: `terrain_feature_lut.png` は `shore sand` / `inland sand` / `rock field dirt` の色差と roughness に使う。startup の `init_visual_handles` は LUT ハンドルを `TerrainSurfaceLutImageHandle` resource として挿入し、`sync_terrain_feature_lut_uniforms_system` がロード完了後に `TerrainSurfaceUniform.lut_shore/inland/rock` へ one-shot で焼き込む。シェーダーは `feature_lut_constants_ready` が立つと LUT texture sample の代わりに uniform 定数を参照する。Sand / Dirt は neutral=0.5 の signed tint を `base texture` に対する color grade（乗算 + 加算）へ変換して適用する。一方 zone 差は LUT ではなく shader 内の専用 palette bias で扱い、`grass_zone` は Grass 表現に、`dirt_zone` は Dirt 表現にだけ適用する。`shore sand` には `shoreline_detail.png` を掛けた shoreline tone を追加し、この経路は LOD1 と LOD1-lite で維持する。
- **境界ブレンド**: `terrain_blend_mask_soft.png` をセル内 fraction に対して引き、center + cardinal 近傍の寄与を重み付き和で合成する。逐次 `mix` で上書きせず、正規化した重みで順序依存を避ける。ブレンド帯はセル端の狭い範囲に限定し、広い面までにじませない。加えて startup の `spawn_boundary_meshes` が `boundary_mask` と別に `boundary_proximity_mask`（dilated edge mask）をベイクし、LOD1 / LOD1-lite はこのマスクが 0 の画素で boundary bilinear を丸ごと省略する。
- **uniform レイアウト**: `TerrainSurfaceUniform` も encase 制約に合わせ、パディング目的の配列ではなく個別の `f32` フィールドで並べる。
- **レイヤー**: `building_3d_render_layers()`（`LAYER_3D` + `LAYER_3D_SHADOW_RECEIVER`）で他の 3D エンティティと同レイヤー。
- **Transform**: chunk は `from_xyz(cx_world, 0.0, -cy_world)`（chunk 中心）。Y=0 が地面平面。
- **障害物除去後の更新**: `hw_world::obstacle_sync_system` が source-aware な差分同期を行い、自然物由来 blocker の最後の削除でのみ `TerrainChangedEvent`（`Message`）を発行する。`bevy_app::terrain_id_map_sync_system` は受信して `TerrainIdMap` の該当ピクセルを書き換える。**chunk entity の再生成は不要**（shader が world-space で texture を参照するため、texture 1 ピクセル更新だけで全 chunk の見た目が更新される）。
- **M1/M2 LOD 観測基盤**: `bevy_app::systems::visual::terrain_lod::update_terrain_lod_metrics_system` が `Camera3dRtt` の `world_to_viewport` から `tile_rtt_px`（RtT 上の 1 タイル見かけサイズ）を算出し、`composite_logical_size(window)` と `RttRuntime.viewport` から `tile_screen_px`（スクリーン表示上の補助値）を導出する。LOD 判定の正本は `TerrainLodMetrics.tile_rtt_px` であり、`tile_screen_px` はデバッグ表示専用。runtime の hysteresis は **`Lod1 → Lod1Lite` が 22px 未満、`Lod1Lite → Lod1` が 25px 超、`Lod1Lite → Lod2` が 14px 未満、`Lod2 → Lod1Lite` が 16px 超**で、`Lod0` は予約スロットのため遷移先に含めない。
- **廃止**: `TerrainBorder` / `terrain_border.rs` / `hw_world::borders` は MS-3-4 で除去済み。`TerrainType::z_layer()` も同様に除去済み。per-tile の `Mesh3d` render entity は chunk renderer 導入時に廃止。`Terrain3dHandles.tile_mesh` フィールドも廃止済み。

### 2D 前景カメラ（composite より手前の `LAYER_2D`）

RtT composite が全画面を覆うため、`startup_systems::setup` で **`WorldForeground2dCamera`**（`Camera2d`、`order=2`、`LAYER_2D`、クリアなし）が同レイヤーを再描画する。`PanCamera` は `MainCamera` のみ更新するため、`sync_world_foreground_2d_camera_system`（`camera_sync.rs`）が **毎フレーム `MainCamera` と同一の `Transform` / `Camera::is_active`** を前景カメラへコピーし、パン・ズームと連動させる。
