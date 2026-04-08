# マップ生成仕様書

`hw_world::generate_world_layout(master_seed)` を中心にした、現行のマップ生成パイプラインの仕様です。
この文書は **生成経路そのもの** を対象とし、地形タイプの見た目・座標変換・物理衝突・レンダリング詳細は [`world_layout.md`](world_layout.md) に委ねます。

## スコープ

この文書が扱うのは次の範囲です。

- seed を入力にした deterministic な地形生成
- `Site` / `Yard` 固定アンカーと各種生成マスクの確定
- WFC ソルバー実行、validate、資源配置、retry / fallback
- `GeneratedWorldLayout` を `bevy_app` 側 startup へ引き渡す契約

この文書が主対象にしないもの:

- 地形の視覚表現、`SectionMaterial`、RtT
- `world_to_grid` / `grid_to_world` などの座標変換
- `WorldMap` への最終反映後のゲームロジック

## エントリポイント

### 本番経路

- `hw_world::generate_world_layout(master_seed)`
  - 起動時に 1 回だけ呼ばれる本番経路
  - `GeneratedWorldLayout` を返し、地形・固定物・初期資源・regrowth 初期化の共通入力になる

### レガシー経路

- `hw_world::generate_base_terrain_tiles(map_width, map_height, sand_width)`
  - 固定 River + 単純 Dirt/Grass を返す旧経路
  - `GeneratedWorldLayout::stub` や visual 用の限定用途に残っている
  - 本番 startup の正規経路ではない

## seed 契約

- `master_seed: u64` が唯一の外部入力である
- 同じ `master_seed` に対して、`generate_world_layout` は同じ `GeneratedWorldLayout` を返す
- retry が発生しても、各試行は `master_seed` から導出した deterministic な sub-seed を使う
- `bevy_app` 側では `HELL_WORKERS_WORLDGEN_SEED=<u64>` を指定するとその seed を使い、未指定時は起動ごとにランダム seed を使う

## 生成パイプライン

`generate_world_layout(master_seed)` は次の順で処理する。

### 1. 固定アンカー確定

- `AnchorLayout::aligned_to_worldgen_seed(master_seed)` を使って `Site` / `Yard` を決める
- X 方向は従来どおり中央付近を基準にする
- Y 方向は `river::preview_river_min_y(master_seed)` を参照し、Site 北辺が川南端より 4 タイル南に来るよう縦シフトする
- Yard 内固定物もこのアンカーに従って同時に移動する
  - 初期木材
  - 猫車置き場

### 2. 中間マスク確定

- `WorldMasks::from_anchor(&anchors)` でアンカー由来マスクと protection band を作る
- その後、次の順で seed 由来マスクを埋める
  1. `fill_river_from_seed(master_seed)`
     - 内部では `river::generate_river_mask` が 3 フェーズで動作する:
       1. RNG で列ごとの `center_y`（蛇行ステップ ±1）と `width`（[2, 4]）を生配列として生成
       2. `smooth_1d_f32`（ミラー端点、`RIVER_SMOOTH_PASSES = 3` パス）で `center_y` のみ移動平均をかけ、列間の急変を抑制する。`width` は有機的な変化のためランダム性を維持する
       3. 平滑化後の `center_y` と元の `width` から `river_mask` と `centerline` を構築し、保護帯フィルタ（`anchor_mask` / `river_protection_band`）を維持する
     - width の平滑化は行わない: seed=0 で River/Sand の 2×2 チェッカーボードが生じ `enforce_no_visual_cross_2x2` で修復不能な視覚クロスが発生するため
  2. `fill_sand_from_river_seed(master_seed)`
  3. `fill_terrain_zones_from_seed(master_seed)`
  4. `fill_rock_fields_from_seed(master_seed)`

この段階で、少なくとも次の情報が確定する。

- `river_mask`
- `river_centerline`
- `sand_candidate_mask`
- `sand_carve_mask`
- `final_sand_mask`
- `grass_zone_mask`
- `dirt_zone_mask`
- `inland_sand_mask`
- `rock_field_mask`

#### terrain zone mask の幾何契約

- `fill_terrain_zones_from_seed(master_seed)` の zone 生成は、アンカー由来の許可領域を前提にしつつ、**Chamfer 3-4 距離**を使って形状を決める。
- アンカー距離場と zone 距離場（`dirt_zone_distance_field` / `grass_zone_distance_field`）は、**直交コスト 3・斜めコスト 4** の 8 近傍ダイクストラで計算する。
- Dirt/Grass のパッチ成長と zone 間離隔用の buffer も同じ距離系を使う。これにより、旧来の 4 近傍 BFS ベースより等距離帯が円形に近くなり、ひし形の外周が出にくい。
- `ZONE_MIN_SEPARATION` と `ZONE_GRADIENT_WIDTH` は「マス数」ではなく、この **Chamfer コスト単位**で解釈される。
- ただし `WorldMasks::from_anchor()` が作る protection band（`river_protection_band` など）は、引き続き **アンカー外周からの 4 近傍距離**で定義される。zone 生成の主因は Chamfer 化されたが、アンカー近傍の許可領域そのものは protection band の形に制約される。

### 3. WFC 地形生成

- `mapgen::wfc_adapter::run_wfc(&masks, sub_seed, attempt)` を呼ぶ
- hard constraint と post-process の責務は `wfc_adapter` に閉じ込める
- WFC の素の出力をそのまま採用せず、後段で `final_sand_mask` と terrain zone バイアスを反映した最終 `terrain_tiles` を作る

WFC 実行後の最終地形では、少なくとも次が成立している必要がある。

- `river_mask` 上は `TerrainType::River`
- `final_sand_mask` 上は `TerrainType::Sand`
- `rock_field_mask` 上は `TerrainType::Dirt`
- `inland_sand_mask` は zone post-process の条件を満たすセルだけ `Sand` にできる
- zone post-process の C グラデーションは、`dirt_zone_distance_field` / `grass_zone_distance_field` の **Chamfer 3-4 コスト**に基づいて評価される
- **視覚十字がない**: 任意の 2×2 ブロックで「視覚キー（terrain priority × zone class）」が 4 角とも異なる十字パターンが現れない。この保証は 2 段階で実現する。
  - **前処理**: WFC 実行前に `fix_zone_mask_crosses` がゾーンマスク（`grass_zone_mask` / `dirt_zone_mask`）の対角パターンを除去し、距離フィールドを再計算する
  - **後処理**: `post_process_tiles` 末尾の `enforce_no_visual_cross_2x2` が 3 フェーズ＋2 ステージで残存十字を局所修復する（地形変更・ゾーンクラス変更の組み合わせ、保護セットのリセット再試行、River 多隣接セルの 2 セル同時変更）
- `TerrainType` は flat enum（`Grass` / `Dirt` / `Sand` / `River`）の 4 unit variant である。亜種区別はなく、視覚的な色差はゾーンマスク（grass / neutral / dirt zone）とシェーダ側 `terrain_id_map` で表現する

### 4. 地形フェーズ validate

- WFC 1 試行ごとに `mapgen::validate::lightweight_validate(&candidate)` を実行する
- ここで落ちた試行は不採用で、次 attempt へ進む
- validate 成功時だけ、到達確認済みの `ResourceSpawnCandidates` を受け取る

`lightweight_validate` の責務は、起動時に必須な成立条件だけを早く確認することにある。
代表例:

- `Site` / `Yard` が River / Sand に侵食されていない
- `Site` / `Yard` の導線が成立している
- 必須資源候補への到達可能性がある
- Yard 内固定アンカーが欠落していない

### 5. 資源配置

- `mapgen::resources::generate_resource_layout(&candidate, sub_seed)` を実行する
- 木は `grass_zone_mask` ベースで配置する
- 岩は `rock_field_mask` ベースで配置する
- regrowth 初期化に必要な `forest_regrowth_zones` もここで組み立てる

ここで決まるのは主に次の pure data である。

- `initial_tree_positions`
- `initial_rock_positions`
- `forest_regrowth_zones`
- `rock_candidates` の最終採用結果

### 6. 資源配置後 validate

- `mapgen::validate::validate_post_resource(&candidate, &resource_layout)` を実行する
- 地形だけでは成立していた経路が、木・岩配置後にも壊れていないことを確認する
- この validate に失敗した試行も不採用で、次 attempt へ進む

### 7. 採用

- 地形 validate と資源 validate の両方を通った試行だけを採用する
- 返却する `GeneratedWorldLayout` には、採用済みの地形・マスク・資源配置・メタ情報をまとめて入れる

## retry と fallback

- WFC 試行は `MAX_WFC_RETRIES` まで deterministic retry する
- すべて失敗した場合だけ `fallback_terrain(&masks, master_seed)` に入る
- fallback は単なる「とりあえず Grass」ではなく、次を維持した上で組み立てる
  - `river_mask`
  - `final_sand_mask`
  - terrain zone バイアス
  - `rock_field_mask`
  - `inland_sand_mask`
- fallback でも `lightweight_validate` と資源配置後 validate を通す
- fallback で返った場合は `GeneratedWorldLayout.used_fallback == true` になる

## 出力契約

`GeneratedWorldLayout` は `hw_world` から `bevy_app` へ渡す pure data 契約である。
重要なのはフィールド名そのものではなく、次の責務分担である。

| 区分 | 内容 | 主な消費先 |
| --- | --- | --- |
| 地形 | `terrain_tiles` | 地形スポーン、`WorldMap` 初期 terrain 反映 |
| 固定アンカー | `anchors` | Site/Yard、初期木材、猫車置き場 |
| 中間マスク | `masks` | debug、検証、地形/資源の意味づけ |
| 到達確認済み候補 | `resource_spawn_candidates` | 水・砂・岩候補の共有 |
| 初期資源 | 木・岩の初期座標 | startup 初期スポーン |
| regrowth 入力 | `forest_regrowth_zones` | 木の再生成初期化 |
| メタ | `master_seed`, `generation_attempt`, `used_fallback` | ログ、debug、再現調査 |

## debug 契約

- `#[cfg(any(test, debug_assertions))]` では `debug_validate(&layout)` を追加実行する
- warning は `[WFC debug] ...` として `eprintln!` する
- validate 失敗で retry へ進む段階でも `[WFC validate] ...` のログが出る
- `bevy_app` 側 startup では、採用された layout の `seed` / `attempt` / `fallback` をログに出す

## app shell への受け渡し

`bevy_app` 側は `GeneratedWorldLayout` を直接その場で再生成せず、startup の先頭で 1 回だけ resource 化して共有する。

1. `resolve_worldgen_seed()` が `HELL_WORKERS_WORLDGEN_SEED` を解決する
2. `prepare_generated_world_layout_resource()` が `generate_world_layout(master_seed)` を呼ぶ
3. `GeneratedWorldLayoutResource` として root world に挿入する
4. `PostStartup` の地形スポーンと初期資源スポーンが同じ layout を消費する
5. regrowth 初期化も同じ layout を参照する

この共有により、地形だけ別 seed、初期木だけ別 layout、という不整合を避ける。

### 境界曲線メッシュ（`bevy_app`・純粋ビジュアル）

`terrain_tiles` の**隣接エッジ**と `WorldMasks` から、PostStartup で曲線状の境界ストリップメッシュを追加スポーンする処理がある（`crates/bevy_app/src/world/map/boundary.rs` の `spawn_boundary_meshes`）。`BoundaryKind::from_pair` が `TerrainType::priority()` を用いて地形種別の変わる境を検出し、**両方とも草／両方とも土**で隣接セルのゾーンクラス（`grass_zone_mask` / `dirt_zone_mask` から導出した 0=草ゾーン, 128=中立, 255=土ゾーン）が変わる境にも線を付ける（`BoundaryKind::GrassZoneTone` / `BoundaryKind::DirtZoneTone`）。`spawn_map_timed` の直後に同一 `PostStartup` チェーンで実行される。

**マテリアル**: 各リボンメッシュに `BoundarySurfaceMaterial`（`hw_visual` の `ExtendedMaterial<StandardMaterial, BoundarySurfaceMaterialExt>`）を使用する。リボン幅方向の UV（u=0=左端、u=1=右端）を介して left/right terrain のアルベドテクスチャを world-space UV でサンプルし、中央でブレンドする。端部はアルファフェードで透過する（フェードゾーン u=[0, 0.30] / u=[0.70, 1.0]、中心帯 u=[0.30, 0.70] が最大不透明）。リボン幅は 48wu（NOISE_AMPLITUDE × 4）。最大ノイズ変位 12wu 時でも u=0.25 がフェード開始点と一致し、リボン端がタイルグリッドエッジを確実にカバーする。`BoundaryKind` ごとに 1 マテリアルインスタンスを作成・キャッシュする。

**カラーグレーディング**: `terrain_feature_map`（binding 117/118：r=shore_sand, g=inland_sand, a=zone_bias）を参照し、草ゾーン・土ゾーンに応じたパレットバイアスと、砂タイルの shore/inland 色補正を地形シェーダーと同一ロジックで行う。砂の shore/inland 判定は、リボンが境界の非砂側に乗る場合でも正しい値を得るため、フラグメント位置を中心とした 3×3 グリッド（9 タイル・コーナーを含む全 8 近傍）から砂タイルの feature を探索する（`find_sand_feature`）。砂の色補正には `terrain_feature_lut`（binding 119/120）と `shoreline_detail`（binding 121/122）も使用する。

**terrain_surface_material.wgsl との連携**: 異カテゴリ境界（草↔土など）の `should_blend_pair` ブレンドはリボンが担当するため、シェーダ側のクロスカテゴリブレンドは除去済み。ゾーントーン差（草ゾーン↔中立↔土ゾーン）はゾーン境界ポリラインも `rasterize_terrain_regions` BFS で `boundary_mask` に書かれるため、シェーダ側は `boundary_mask` のバイリニアブレンドでゾーントーン遷移を処理する。

**`rasterize_terrain_regions` BFS と `boundary_mask` エンコーディング**:

全ポリライン（異カテゴリ・ゾーントーン含む全 `BoundaryKind`）を 1024×1024 の `boundary_mask` テクスチャに焼き込む。エンコーディング:

| raw byte | 意味 |
|:---|:---|
| 0 / 1 / 2 | Grass（ゾーントーン 0=中立→草, 1=中間, 2=草ゾーン） |
| 85 / 86 / 87 | Dirt（ゾーントーン 0=中立→土, 1=中間, 2=土ゾーン） |
| 170 / 171 / 172 | Sand（shore / midzone / inland） |
| 255 | River |
| 254 | Sentinel（カーブ曲線ピクセル = 境界線本体） |
| 253 | Unassigned（BFS 未到達・初期値） |

BFS はタイルセンター座標を多発源として Sentinel バリアで止まる multi-source BFS で全ピクセルを塗る。Sentinel 内部（ポリライン交差などで密に囲まれた孤立ピクセル）は BFS が到達できない場合があり、これらは **Step 4.5 で Unassigned(253) → Sentinel(254) に変換**してから 8-pass ダイレーション（Step 5）を行う。この変換がないと `debug_assert!` でパニックが発生する。

シェーダ側では raw byte の粗粒度 ID（`region_to_coarse_id`）と raw byte そのものを両方取得し:
- **fast path**: 2×2 バイリニアコーナー全角で coarse ID と raw byte が一致する場合のみ使用。ゾーントーン境界付近はコーナー間で raw byte が変わるため fast path に入らずバイリニアブレンドに進む。
- **バイリニアブレンド**: `feature_with_zone_tone(feature, raw_byte, terrain_id)` でコーナーごとに `feature.a`（ゾーンバイアス）を raw byte から確定し、正しいパレットバイアスを適用する。
- **`other_id` 先確定**: `feature_other`（相手地形の feature）検索前に、バイリニアコーナー (`id00`/`id10`/`id01`/`id11`) から `other_id` を確定してから 8 近傍を `== other_id` で検索する。`!= region_id` 検索では Dirt タイルが Grass・Sand の両方に隣接する場合に誤って Grass feature を Sand コーナーに適用してしまう問題を防ぐ。

- **生成データを変えない**。`GeneratedWorldLayout` の中身・`WorldMap` の地形グリッド・経路・当たり判定には影響しない。
- **装飾専用**。ノイズ変位やスプラインは見た目のためだけであり、セル種別の真実は常に `terrain_tiles` である。
- **seed**: `master_seed` と **ポリラインごとの幾何**（種別・開閉・端／中点のコーナーキー・全弧長など）から `PolylineNoiseParams`（シード・弧長位相・周波数倍率）を決定論的に導出する。異カテゴリの境界が複数あれば種別ごとに別波形になる（再現性はビジュアル検証用）。
- **三叉路**: 全境界エッジを合わせたグラフで次数 ≥ 3 のグリッドコーナー（`boundary_junction_corner_keys`）を検出し、そのコーナーを始端・終端とするリボンでは丸キャップを生成しない（`build_quad_strip_mesh` の `add_start_cap`/`add_end_cap` フラグ）。キャップを生成すると複数リボンの丸キャップが重なって輪状のアーティファクトが発生するため。

- **面取り（Chamfer）**: ノイズ変位後、Catmull-Rom スプライン適用前に `chamfer_polyline_points` を実行する。川岸など「水平基調 + 幅変化起因の 1 タイル垂直段差」を持つポリラインでは、段差コーナー（≈90°）で Catmull-Rom がオーバーシュートして「wavy staircase」が生じる。この関数は鋭角コーナー（内角コサイン < `CHAMFER_COS_THRESHOLD = 0.5`、すなわち 60° より鋭い角）を 2 つのベベル頂点で置換し（距離 `CHAMFER_DISTANCE = TILE_SIZE × 0.35 ≈ 11.2wu`）、スプラインに滑らかなガイドを与える。ジャンクション頂点（`displace_polyline` で変位=0 なので元座標にある）と開ポリラインの端点は変更しない。ノイズパラメータは変位前の元ポリラインから計算するため、面取りによってノイズハッシュは変化しない。

この経路はマップ生成アルゴリズムの契約外であり、本書の「出力契約」テーブルに新しい列を増やすものではない。

## 非目標と設計上の線引き

- `hw_world` は pure data と pure algorithm を返す
- `bevy_app` は Resource 化、`Commands`、メッシュ・マテリアル・Entity spawn を担当する
- したがって、生成仕様書で保証すべき対象は「どのセルが何であるか」「どの候補が有効か」「同じ seed で何が再現されるか」であり、見た目や Bevy の実体生成順ではない

## 関連ファイル

- [`../crates/hw_world/src/mapgen/mod.rs`](../crates/hw_world/src/mapgen/mod.rs): モジュールルート（`generate_base_terrain_tiles`、`generate_world_layout` の公開）
- [`../crates/hw_world/src/mapgen/pipeline.rs`](../crates/hw_world/src/mapgen/pipeline.rs): `generate_world_layout()` のオーケストレーション本体
- [`../crates/hw_world/src/mapgen/types.rs`](../crates/hw_world/src/mapgen/types.rs): `GeneratedWorldLayout` 契約
- [`../crates/hw_world/src/mapgen/validate/mod.rs`](../crates/hw_world/src/mapgen/validate/mod.rs): validate 公開面（`lightweight_validate`, `debug_validate`, `ValidationError`, `ValidationWarning`）
- [`../crates/hw_world/src/mapgen/validate/terrain.rs`](../crates/hw_world/src/mapgen/validate/terrain.rs): 地形フェーズ validate（`lightweight_validate`, `ValidatorPathWorld`, 必須資源候補の収集）
- [`../crates/hw_world/src/mapgen/validate/post_resource.rs`](../crates/hw_world/src/mapgen/validate/post_resource.rs): 資源配置後 validate（`validate_post_resource`, `ResourceObstaclePathWorld`）
- [`../crates/hw_world/src/mapgen/validate/debug.rs`](../crates/hw_world/src/mapgen/validate/debug.rs): debug / test ビルド専用診断（`debug_validate`）
- [`../crates/hw_world/src/mapgen/resources.rs`](../crates/hw_world/src/mapgen/resources.rs): 木・岩・regrowth zone 配置
- [`../crates/hw_world/src/mapgen/wfc_adapter.rs`](../crates/hw_world/src/mapgen/wfc_adapter.rs): WFC adapter と fallback
- [`../crates/hw_world/src/world_masks.rs`](../crates/hw_world/src/world_masks.rs): 生成中間マスク
- [`../crates/hw_world/src/anchor.rs`](../crates/hw_world/src/anchor.rs): Site/Yard と Yard 内固定物
- [`../crates/bevy_app/src/world/map/spawn.rs`](../crates/bevy_app/src/world/map/spawn.rs): startup での resource 化と地形スポーン
- [`../crates/bevy_app/src/world/map/boundary.rs`](../crates/bevy_app/src/world/map/boundary.rs): 境界曲線メッシュ（純粋ビジュアル、`terrain_tiles` からの派生）
- [`world_layout.md`](world_layout.md): 地形タイプ、座標系、レンダリングとゲーム内意味づけ
