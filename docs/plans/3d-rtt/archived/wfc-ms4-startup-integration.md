# MS-WFC-4: Startup 統合と Yard 内固定資源の移行

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms4-startup-integration` |
| ステータス | `実装完了・クリーンアップ残あり` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 次MS | [`wfc-ms45-docs-tests.md`](wfc-ms45-docs-tests.md) |
| 前提 | `GeneratedWorldLayout` に `terrain_tiles` / `anchors` / `masks` / `resource_spawn_candidates` / `initial_tree_positions` / `initial_rock_positions` / `forest_regrowth_zones` 等が入り、`generate_world_layout(master_seed)` 内で `lightweight_validate()`・資源 fallback・`validate_post_resource()` まで完了している（startup は検証済みレイアウトを受け取るだけ） |

---

## 1. この文書の位置づけ

この MS は、**WFC 生成結果を bevy_app の startup 実行経路へ実際に接続する**段階である。  
現行コードでは主要経路はすでに切り替わっているため、本書は「未着手の計画」ではなく
**実装済み内容の確認メモ + 残課題整理**として扱う。

MS-WFC-4.5 の責務は別で、docs 全体の整合、roadmap ステータス更新、追加テスト、
debug 導線の整理はそちらに残る。

---

## 2. 実装で達成されたこと

現行実装では、startup の主要経路は固定座標テーブルではなく
`GeneratedWorldLayoutResource` を参照する。

- `Startup` の `setup()` が `prepare_generated_world_layout_resource()` を呼び、`GeneratedWorldLayoutResource` を `Resource` として挿入する
- `PostStartup` の chain で `spawn_map_timed` → `initial_resource_spawner_timed` が同じ `GeneratedWorldLayoutResource` を消費する
- 地形スポーンは `layout.terrain_tiles` を直接使う
- 初期木・岩は `layout.initial_tree_positions` / `layout.initial_rock_positions` から生成する
- 初期木材は `layout.anchors.initial_wood_positions` から生成する
- Site / Yard は `layout.anchors.site` / `layout.anchors.yard` を `site_yard_layout_from_anchor()` で写して生成する
- `layout.anchors` は `generate_world_layout` 内で `AnchorLayout::aligned_to_worldgen_seed(master_seed)` により決まり、プレビュー川と縦位置が整合する（`docs/world_layout.md` の固定アンカー・川節）
- 猫車置き場は `layout.anchors.wheelbarrow_parking` の左下セルを基準に `compute_parking_layout()` で walkability を確認してから生成する
- 森林再生は独立 `ForestRegrowthZones` Resource ではなく、`configure_regrowth_from_generated_layout()` が `RegrowthManager.zones` を上書きする形で接続されている

---

## 3. 現行フロー

### 3.1 Startup での worldgen resource 準備

実装箇所: `crates/bevy_app/src/plugins/startup/startup_systems.rs`,
`crates/bevy_app/src/world/map/spawn.rs`

1. `setup()` が `prepare_generated_world_layout_resource()` を呼ぶ
2. `prepare_generated_world_layout_resource()` が `resolve_worldgen_seed()` で seed を決める
3. `generate_world_layout(master_seed)` を実行し、`GeneratedWorldLayoutResource { master_seed, layout }` を返す
4. `setup()` がその resource を `commands.insert_resource(...)` する

現在の seed 入力は **環境変数 `HELL_WORKERS_WORLDGEN_SEED` のみ**である。
旧計画にあった CLI 引数 `--seed` は未導入。

### 3.2 PostStartup での消費順序

実装箇所: `crates/bevy_app/src/plugins/startup/mod.rs`

`StartupPlugin` は `PostStartup` で次を **この順序で** `chain()` 登録している（`crates/bevy_app/src/plugins/startup/mod.rs` と一致）。

1. `visual_handles::init_visual_handles`
2. `spawn_map_timed`
3. `initial_resource_spawner_timed`
4. `spawn_entities`
5. `spawn_familiar_wrapper`
6. `setup_perf_scenario_if_enabled`
7. `setup_ui`
8. `crate::interface::ui::dev_panel::spawn_dev_panel_system`
9. `populate_resource_spatial_grid`
10. `rtt_composite::spawn_rtt_composite_sprite`

このため、`WorldMap` の地形は `spawn_map_timed` で確定した後に、
初期木・岩・木材・Site/Yard・猫車置き場が `initial_resource_spawner_timed` から生成される。

### 3.3 地形スポーン

実装箇所: `crates/bevy_app/src/world/map/spawn.rs`

- `spawn_map()` は `generated_layout.layout.terrain_tiles` を row-major で読み、
  `WorldMap` の terrain を更新しつつ 3D 地形タイルを生成する
- 起動ログには `worldgen seed`, `attempt`, `fallback` が出る

ここで使うのは **WFC の最終出力**であり、旧来の固定地形テーブルではない。

### 3.4 初期スポーン

実装箇所: `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`,
`terrain_resources.rs`, `layout.rs`, `facilities.rs`

- `spawn_trees()` は `layout.initial_tree_positions`
- `spawn_rocks()` は `layout.initial_rock_positions`
- `spawn_initial_wood()` は `layout.anchors.initial_wood_positions`
- `spawn_site_and_yard()` は `site_yard_layout_from_anchor(&layout.anchors)`
- `spawn_wheelbarrow_parking()` は `layout.anchors.wheelbarrow_parking.min_x/min_y` を基準に生成

猫車置き場は `compute_parking_layout()` の walkability 判定を通らなければスキップされ、
`INITIAL_SPAWN: skipped initial wheelbarrow parking at … (not walkable)` が warn される。

### 3.5 森林再生接続

実装箇所: `crates/bevy_app/src/plugins/startup/startup_systems.rs`,
`crates/bevy_app/src/world/regrowth.rs`

`initial_resource_spawner_timed()` は初期スポーン前に
`configure_regrowth_from_generated_layout(&mut regrowth, &generated_layout.layout)` を呼ぶ。

この関数は:

- `layout.forest_regrowth_zones` を `RegrowthManager.zones` へ変換する
- 各 zone に含まれる `initial_tree_positions` を集計して `initial_count` を決める
- 木が 0 本の zone は捨てる

つまり、旧計画のような `ForestRegrowthZones` 追加 Resource は採用されていない。
**最終設計は `RegrowthManager` の初期化更新**である。

---

## 4. 旧計画から変わった点

### 4.1 `GeneratedWorldLayout` の扱い

旧案どおり、`hw_world::GeneratedWorldLayout` 自体に Bevy 依存は持たせず、
`bevy_app` 側で `GeneratedWorldLayoutResource` wrapper を作る方式になった。

### 4.2 validator の接続点

旧案では「startup フローで `lightweight_validate()` を呼ぶ準備」を前提にしていたが、
現行コードでは validator は `generate_world_layout()` の内部責務である。
startup 側は **検証済みレイアウトを受け取るだけ**になっている。

### 4.3 regrowth の設計

旧案の `ForestRegrowthZones` Resource は未採用。
`RegrowthManager` をそのまま使い続け、生成結果から zone 群を注入する設計に収束した。

### 4.4 旧固定定数の扱い

`TREE_POSITIONS` / `ROCK_POSITIONS` / `INITIAL_WOOD_POSITIONS` は **削除済み**
（`layout.rs` は `RIVER_*` / `SAND_WIDTH` のみ残す）。`bevy_app` / `hw_world` の re-export も除去済み。

---

## 5. 変更ファイルと責務

| ファイル | 現行責務 |
| --- | --- |
| `crates/bevy_app/src/world/map/spawn.rs` | seed 解決、`GeneratedWorldLayoutResource` 構築、地形タイルの 3D スポーン |
| `crates/bevy_app/src/world/map/mod.rs` | `GeneratedWorldLayoutResource` / `spawn_map` / seed helper の再 export |
| `crates/bevy_app/src/plugins/startup/startup_systems.rs` | startup で layout resource を挿入し、regrowth 初期化と `initial_resource_spawner` 呼び出しを配線 |
| `crates/bevy_app/src/plugins/startup/mod.rs` | `PostStartup` の chain 順序を定義 |
| `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs` | 生成済み layout を消費して初期木・岩・木材・Site/Yard・猫車置き場を生成 |
| `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs` | 木・岩・初期木材の各スポーン helper |
| `crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs` | `AnchorLayout` から Site/Yard と parking の pure data を導出 |
| `crates/bevy_app/src/systems/logistics/initial_spawn/facilities.rs` | Site/Yard と猫車置き場の実スポーン |
| `crates/bevy_app/src/world/regrowth.rs` | `GeneratedWorldLayout` から `RegrowthManager` を再設定 |
| `crates/hw_world/src/mapgen.rs` | validator 済み `GeneratedWorldLayout` の生成本体 |
| `crates/hw_world/src/layout.rs` | レガシー固定川範囲 `RIVER_*` と `SAND_WIDTH` |

---

## 6. 実装済みチェックリスト

- [x] startup が `generate_world_layout()` を使って世界を生成する
- [x] `bevy_app` 側で `GeneratedWorldLayoutResource` wrapper を作って `Resource` 挿入している
- [x] 地形スポーンが `layout.terrain_tiles` を使う
- [x] 初期木が `layout.initial_tree_positions` から生成される
- [x] 初期岩が `layout.initial_rock_positions` から生成される
- [x] 初期木材が `layout.anchors.initial_wood_positions` から生成される
- [x] 猫車置き場が `layout.anchors.wheelbarrow_parking` を基準に生成される
- [x] Site / Yard が `layout.anchors` から生成される
- [x] `RegrowthManager` が `layout.forest_regrowth_zones` と `layout.initial_tree_positions` から初期化される
- [x] 起動ログに seed / attempt / fallback が出る
- [x] fallback 時でも startup が `GeneratedWorldLayout` をそのまま消費できる

### 未完了または後続へ送る項目

- [x] `TREE_POSITIONS` / `ROCK_POSITIONS` / `INITIAL_WOOD_POSITIONS` の定数定義そのものを削除する
- [x] `crates/bevy_app/src/world/map/mod.rs` / `crates/hw_world/src/lib.rs` の旧定数 re-export を外す
- [ ] seed 入力を CLI 引数まで拡張するか、環境変数専用で確定する
- [ ] roadmap / debug docs / README 系の「preview only」表現を現状に合わせて更新する

---

## 7. 現時点の判断

MS-WFC-4 の本質である「**WFC 生成結果を startup 本経路へ接続する**」は達成済みである。  
旧木・岩・初期木材の固定座標テーブル（`TREE_POSITIONS` 等）も **撤去済み**である。

一方で、seed を CLI で渡すかどうかや、roadmap / README の表現更新は **未着手**のため、
マイルストーン全体としては **実装は完了・運用・ドキュメントの細部は後続** が妥当である。

したがって、この MS の残りは「起動経路の実装」ではなく、
**seed 方針の確定と docs の整合化**（および MS-WFC-4.5 の範囲）として扱う。

---

## 8. ロールバック観点

問題が起きた場合に戻すべきポイントは次の 3 つである。

1. `setup()` での `GeneratedWorldLayoutResource` 挿入を外す
2. `spawn_map()` の入力を固定地形へ戻す
3. `initial_resource_spawner()` の各入力を旧固定定数へ戻す

ただし現行コードでは startup がすでに `GeneratedWorldLayoutResource` 前提で配線されているため、
部分ロールバックではなく **resource 準備・地形スポーン・初期スポーンの 3 点をまとめて戻す**前提で考える。

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-04` | — | レビュー反映（前提・PostStartup chain・anchors・warn 文言）。旧定数 `TREE_POSITIONS` / `ROCK_POSITIONS` / `INITIAL_WOOD_POSITIONS` と re-export 削除、`hw_world` README・§4.4・§5・§6 同期 |
| `2026-04-05` | `Codex` | 現行実装に合わせて全面更新。`GeneratedWorldLayoutResource` / `setup` + `PostStartup` chain / `RegrowthManager` 初期化 / 環境変数 seed / legacy 定数残存を反映 |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
