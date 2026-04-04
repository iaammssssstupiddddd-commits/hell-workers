# MS-WFC-3: 木・岩の procedural 配置

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms3-procedural-resources` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md) |
| 次MS | [`wfc-ms4-startup-integration.md`](wfc-ms4-startup-integration.md) |
| 前提 | `generate_world_layout()` が地形グリッドと `WorldMasks` を返せる（MS-WFC-2c 完了） |

---

## 1. 目的

WFC 地形生成の結果を使い、**木・岩の初期配置と森林再生エリアを procedural に生成する**。

- 固定座標テーブル（`TREE_POSITIONS` / `ROCK_POSITIONS`）への依存を取り除く
- 木の「初期配置」と「再生可能エリア（ForestZone）」を **同じ生成フェーズ** で決める（F9.5 方針）
- 木の `regrowth` システムが固定座標ではなく `forest_regrowth_zones` を参照するよう変更する
- `GeneratedWorldLayout::initial_tree_positions` / `initial_rock_positions` / `forest_regrowth_zones` に結果を格納する

---

## 2. 設計方針

### 2.1 生成単位

木・岩の配置は `generate_world_layout()` 内から呼ばれる **pure 関数**として実装する。

```rust
// crates/hw_world/src/mapgen/resources.rs
pub fn generate_resource_layout(
    terrain_tiles: &[TerrainType],
    masks: &WorldMasks,
    anchors: &AnchorLayout,
    seed: u64,
) -> ResourceLayout;

pub struct ResourceLayout {
    pub initial_tree_positions: Vec<IVec2>,
    pub forest_regrowth_zones: Vec<ForestZone>,
    pub initial_rock_positions: Vec<IVec2>,
    pub resource_spawn_candidates: ResourceSpawnCandidates,
}
```

### 2.2 exclusion zone の定義

以下の領域には木・岩を配置しない。

| 領域 | 理由 |
| --- | --- |
| `masks.anchor_mask`（Site/Yard） | ゲームプレイ導線の確保 |
| `masks.protection_band` | 序盤導線の安全確保 |
| River タイル | 川に木・岩は不自然 |
| `anchors.initial_wood_grid` 周辺 1 タイル | 初期木材の視認性 |
| `anchors.wheelbarrow_parking_grid` 周辺 1 タイル | 猫車置き場の操作性 |
| 最短歩行経路の通路幅（最低 2 タイル）| 到達可能性の維持 |

### 2.3 木の配置戦略

```
1. Grass タイル（exclusion zone 以外）を候補セルとする
2. Poisson disk sampling（または均等グリッド + jitter）で森林クラスターを生成
   - クラスター数は seed から決まる乱数で FOREST_CLUSTER_COUNT_MIN〜MAX
   - 各クラスターの中心と半径（FOREST_CLUSTER_RADIUS_MIN〜MAX）を決める
3. 各クラスターを ForestZone として記録 → forest_regrowth_zones
4. 各 ForestZone 内の初期木セルを確率（INITIAL_TREE_DENSITY）で選択 → initial_tree_positions
5. walkable 到達可能性を壊していないか確認（MS-WFC-2c の BFS を使う）
```

### 2.4 岩の配置戦略

```
1. Grass または Dirt タイル（exclusion zone 以外）を候補セルとする
2. 岩はクラスター配置（ROCK_CLUSTER_COUNT, ROCK_CLUSTER_RADIUS）
3. 各岩クラスター内のセルを確率で選択 → initial_rock_positions
4. walkable 到達可能性を壊していないかを再確認
   - 特に Yard から岩源（必須資源）への到達が確保されているか
```

### 2.5 ForestZone のデータ構造（MS-WFC-1 からの確認）

```rust
pub struct ForestZone {
    pub center: IVec2,
    pub radius: u32,
    // 将来拡張可: density_curve, age, etc.
}
```

---

## 3. 定数（`resources.rs` または `mapgen/constants.rs` に集約）

```rust
pub const FOREST_CLUSTER_COUNT_MIN: u32 = 4;
pub const FOREST_CLUSTER_COUNT_MAX: u32 = 10;
pub const FOREST_CLUSTER_RADIUS_MIN: u32 = 3;
pub const FOREST_CLUSTER_RADIUS_MAX: u32 = 8;
pub const INITIAL_TREE_DENSITY: f32 = 0.5;  // zone 内の初期配置率

pub const ROCK_CLUSTER_COUNT_MIN: u32 = 3;
pub const ROCK_CLUSTER_COUNT_MAX: u32 = 7;
pub const ROCK_CLUSTER_RADIUS_MIN: u32 = 2;
pub const ROCK_CLUSTER_RADIUS_MAX: u32 = 5;
pub const INITIAL_ROCK_DENSITY: f32 = 0.6;
```

---

## 4. regrowth システムへの接続

`crates/bevy_app/src/world/regrowth.rs` の対応:

- 現状: 固定座標配列や固定ロジックで木の再生エリアを決めている（要調査）
- 変更後: `GeneratedWorldLayout::forest_regrowth_zones` を参照して再生エリアを決める

**接続方法**（MS-WFC-4 での startup 統合と連携）:
- `regrowth` システムが参照する Resource（または Component）に `ForestZone` のリストを格納する
- 初期化は `bevy_app` 側 startup で `GeneratedWorldLayout` から取り出して挿入する

```rust
// 例: bevy_app/src/systems/logistics/initial_spawn/mod.rs (MS-WFC-4 で実装)
commands.insert_resource(ForestRegrowthZones(layout.forest_regrowth_zones.clone()));
```

この MS では `regrowth.rs` の呼び出し先型を変更する準備（型定義・Resource 追加）のみ行う。
実際の startup 接続は MS-WFC-4 で行う。

---

## 5. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/resources.rs` (新規) | `generate_resource_layout()` / `ResourceLayout` / 配置定数 |
| `crates/hw_world/src/mapgen/mod.rs` | `resources` モジュール追加 |
| `crates/hw_world/src/mapgen.rs` | `generate_world_layout()` から `generate_resource_layout()` を呼び出す |
| `crates/hw_world/src/layout.rs` | `TREE_POSITIONS` / `ROCK_POSITIONS` に `#[deprecated]` または廃止コメント追加 |
| `crates/hw_world/src/lib.rs` | `ForestZone` / `ForestRegrowthZones` を pub に公開 |
| `crates/bevy_app/src/world/regrowth.rs` | `ForestRegrowthZones` Resource を使うよう型追加（接続は MS-WFC-4） |
| `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs` | 固定木・岩 spawn の廃止コメント追加（実際の切り替えは MS-WFC-4） |

---

## 6. 完了条件チェックリスト

- [ ] `generate_resource_layout()` が純粋関数として実装されている
- [ ] 木・岩が exclusion zone に配置されない
- [ ] 木の初期配置が `ForestZone` の一部として生成される
- [ ] `forest_regrowth_zones` が `GeneratedWorldLayout` に格納されている
- [ ] `initial_tree_positions` と `forest_regrowth_zones` が矛盾しない（初期木が zone 内に収まる）
- [ ] `regrowth.rs` が参照する型に `ForestRegrowthZones` Resource が定義されている
- [ ] 必須資源（岩源）への到達可能性が維持されている
- [ ] `TREE_POSITIONS` / `ROCK_POSITIONS` に廃止コメントが入っている
- [ ] `cargo test -p hw_world` の golden seed テストが通る
- [ ] `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 7. テスト

```rust
#[test]
fn test_trees_not_in_exclusion_zone() {
    let layout = generate_world_layout(GOLDEN_SEED_STANDARD);
    for pos in &layout.initial_tree_positions {
        let idx = pos.y as usize * MAP_WIDTH + pos.x as usize;
        assert!(!layout.masks.anchor_mask.get(idx), "tree in anchor zone at {pos:?}");
        assert!(!layout.masks.protection_band.get(idx), "tree in protection band at {pos:?}");
    }
}

#[test]
fn test_trees_in_forest_zones() {
    let layout = generate_world_layout(GOLDEN_SEED_STANDARD);
    for pos in &layout.initial_tree_positions {
        let in_some_zone = layout.forest_regrowth_zones.iter().any(|z| {
            let d = (*pos - z.center).as_vec2().length();
            d <= z.radius as f32
        });
        assert!(in_some_zone, "tree at {pos:?} not in any ForestZone");
    }
}

#[test]
fn test_rocks_not_in_exclusion_zone() {
    let layout = generate_world_layout(GOLDEN_SEED_STANDARD);
    for pos in &layout.initial_rock_positions {
        let idx = pos.y as usize * MAP_WIDTH + pos.x as usize;
        assert!(!layout.masks.anchor_mask.get(idx));
    }
}
```

---

## 8. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
cargo check --workspace
cargo clippy --workspace
```

手動確認:
- `cargo run` で木・岩が毎回異なる分布で生成されることを目視確認
- `Site/Yard` 周辺に木・岩が食い込んでいないことを確認

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
