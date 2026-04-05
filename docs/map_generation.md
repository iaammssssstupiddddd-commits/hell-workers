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

### 3. WFC 地形生成

- `mapgen::wfc_adapter::run_wfc(&masks, sub_seed, attempt)` を呼ぶ
- hard constraint と post-process の責務は `wfc_adapter` に閉じ込める
- WFC の素の出力をそのまま採用せず、後段で `final_sand_mask` と terrain zone バイアスを反映した最終 `terrain_tiles` を作る

WFC 実行後の最終地形では、少なくとも次が成立している必要がある。

- `river_mask` 上は `TerrainType::River`
- `final_sand_mask` 上は `TerrainType::Sand`
- `rock_field_mask` 上は `TerrainType::Dirt`
- `inland_sand_mask` は zone post-process の条件を満たすセルだけ `Sand` にできる

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
- [`world_layout.md`](world_layout.md): 地形タイプ、座標系、レンダリングとゲーム内意味づけ
