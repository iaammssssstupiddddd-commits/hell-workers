# MS-WFC-4: Startup 統合と Yard 内固定資源の移行

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms4-startup-integration` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 次MS | [`wfc-ms45-docs-tests.md`](wfc-ms45-docs-tests.md) |
| 前提 | `GeneratedWorldLayout` に地形・木・岩・アンカー情報が揃っている（MS-WFC-3 完了） |

---

## 1. 目的

`bevy_app` の初期スポーンを、固定座標テーブルから `GeneratedWorldLayout` を使うように切り替える。

- `initial_resource_spawner` が生成結果から木・岩・初期木材・猫車置き場を spawn する
- `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` の絶対座標依存を削除する
- `INITIAL_WOOD_POSITIONS` の固定配列依存を削除する
- `ForestRegrowthZones` Resource を startup 時に挿入し、`regrowth` システムへ接続する
- ゲームが正常に起動し、`Site↔Yard` 到達可能性と固定 Yard 資源が維持されることを確認する

---

## 2. 前提確認（着手前）

以下が完了していることを確認する:

- [ ] `GeneratedWorldLayout` に `initial_tree_positions`, `initial_rock_positions`, `forest_regrowth_zones` が格納されている
- [ ] `AnchorLayout` から `initial_wood_grid` と `wheelbarrow_parking_grid` が取得できる
- [ ] `lightweight_validate()` が startup フローで呼ばれる準備がある

---

## 3. 移行戦略

段階的に切り替え、毎ステップで `cargo check` が通ることを確認する。

### Step 1: `GeneratedWorldLayout` を startup の Resource として挿入

```rust
// bevy_app の startup または PostStartup で:
let layout = hw_world::generate_world_layout(seed);
commands.insert_resource(GeneratedWorldLayout(layout));
```

`GeneratedWorldLayout` を `bevy::prelude::Resource` として使うために、`hw_world` 側で `#[derive(Resource)]` を追加する（または `bevy_app` 側で newtype wrapper を用意する）。

### Step 2: 木・岩 spawn を生成結果から読むように切り替え

対象ファイル: `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`

```rust
// Before: TREE_POSITIONS.iter() で固定座標ループ
// After:  layout.initial_tree_positions.iter() でループ
```

### Step 3: 初期木材 spawn を Yard 内固定アンカーから読むように切り替え

対象ファイル: `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs` または `facilities.rs`

```rust
// Before: INITIAL_WOOD_POSITIONS 固定配列
// After:  layout.anchors.initial_wood_grid
```

### Step 4: 猫車置き場 spawn を Yard 内固定アンカーから読むように切り替え

対象ファイル: `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`

```rust
// Before: INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)
// After:  layout.anchors.wheelbarrow_parking_grid
```

### Step 5: ForestRegrowthZones Resource を挿入

```rust
commands.insert_resource(ForestRegrowthZones(layout.forest_regrowth_zones.clone()));
```

`regrowth.rs` がこの Resource を参照するよう変更する。

### Step 6: 旧固定座標テーブルの削除

- `INITIAL_WHEELBARROW_PARKING_GRID` 削除
- `INITIAL_WOOD_POSITIONS` 削除（または残す場合はコメントで廃止理由を明記）
- `TREE_POSITIONS` / `ROCK_POSITIONS` 削除

削除前に `cargo check --workspace` でコンパイルエラーがないことを確認する。

---

## 4. seed 管理

`generate_world_layout()` に渡す `master_seed` の取得場所を決める。

| 方法 | トレードオフ |
| --- | --- |
| 起動引数 `--seed <u64>` | デバッグしやすい。省略時は `rand::random()` |
| 環境変数 `HELL_WORKERS_SEED` | テスト自動化に便利 |
| 起動時刻ベース（現状の乱数） | 再現しにくい |

**推奨**: `--seed` 引数優先、省略時は `SystemTime::now()` から `u64` を生成し、起動ログに seed 値を出力する。

---

## 5. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs` | startup で `generate_world_layout()` を呼び出し、Resource として挿入。猫車置き場・初期木材 spawn を AnchorLayout から読む |
| `crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs` | `compute_site_yard_layout()` を `AnchorLayout::fixed()` と統合、重複排除 |
| `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs` | 木・岩 spawn を `GeneratedWorldLayout` から読む |
| `crates/bevy_app/src/systems/logistics/initial_spawn/facilities.rs` | 施設 spawn の固定座標依存を AnchorLayout に切り替え |
| `crates/bevy_app/src/world/regrowth.rs` | `ForestRegrowthZones` Resource を参照するよう変更 |
| `crates/hw_world/src/layout.rs` | `TREE_POSITIONS`, `ROCK_POSITIONS`, `INITIAL_WOOD_POSITIONS`, `INITIAL_WHEELBARROW_PARKING_GRID` 削除 |
| `crates/hw_world/src/generated_world_layout.rs` または `lib.rs` | `#[derive(Resource)]` 追加、または bevy_app 側に newtype wrapper |

---

## 6. 完了条件チェックリスト

- [ ] startup が `generate_world_layout()` を使って世界を生成する
- [ ] 初期木材が `AnchorLayout::initial_wood_grid` の位置に spawn される
- [ ] 猫車置き場が `AnchorLayout::wheelbarrow_parking_grid` の位置に spawn される
- [ ] 旧 `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` への依存が消えている
- [ ] 旧 `INITIAL_WOOD_POSITIONS` / `TREE_POSITIONS` / `ROCK_POSITIONS` への依存が消えている
- [ ] `ForestRegrowthZones` が Resource として挿入され、`regrowth.rs` が参照している
- [ ] 木・岩 spawn が生成結果から読まれている
- [ ] `Site↔Yard` と Yard から固定/必須資源への到達可能性が維持されている
- [ ] `cargo check --workspace` / `cargo clippy --workspace` が通る
- [ ] `cargo run` でゲームが正常に起動する

---

## 7. 手動確認シナリオ

- `cargo run` を複数回実行し、地形・木・岩の分布が毎回変わることを確認する
- `Site/Yard` が毎回同じ位置にあり、内部が `Grass` / `Dirt` のみであることを確認する
- Yard 内に初期木材と猫車置き場が固定位置で生成されることを確認する
- `Site/Yard` 周辺に木・岩・River が食い込んでいないことを確認する
- `Site` から `Yard`、および `Yard` から初期木材・猫車置き場・最低 1 つの水源/砂源/岩源へ到達できることを確認する

---

## 8. ロールバック手順

問題が起きた場合:

1. `initial_resource_spawner` を旧固定 spawn 経路（`TREE_POSITIONS` 等）に戻す
2. `generate_world_layout()` 呼び出しを一時的に無効化する
3. `mapgen.rs` の内部を `generate_base_terrain_tiles()` ラッパーに戻す

---

## 9. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
cargo clippy --workspace
cargo run
```

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
