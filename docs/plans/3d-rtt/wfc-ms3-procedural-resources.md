# MS-WFC-3: 木・岩の procedural 配置

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms3-procedural-resources` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2-5-terrain-zone-mask.md`](wfc-ms2-5-terrain-zone-mask.md) |
| 次MS | [`wfc-ms4-startup-integration.md`](wfc-ms4-startup-integration.md) |
| 前提 | `MS-WFC-2.5` 完了。`WorldMasks` に `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` が入り、`GeneratedWorldLayout` / `WfcForestZone` は定義済み。ただし `generate_world_layout()` はまだ `initial_tree_positions` / `forest_regrowth_zones` / `initial_rock_positions` を空で返す |

---

## 1. 目的

WFC 地形生成の結果を使い、**木・岩の初期配置と森林再生エリアを pure data として確定する**。

- `GeneratedWorldLayout::initial_tree_positions` / `forest_regrowth_zones` / `initial_rock_positions` を実データで埋める
- `MS-WFC-2.5` で導入した `grass_zone_mask` / `dirt_zone_mask` を、木・岩の優先配置領域として再利用する
- 固定座標テーブル (`TREE_POSITIONS` / `ROCK_POSITIONS`) 依存を `hw_world` の生成経路から外す
- `bevy_app` 側の startup / regrowth 切り替えに必要な pure data を揃える

この MS の責務は **pure 生成結果を返せるようにするところまで**。  
`bevy_app` 側の startup 切り替えと `RegrowthManager` への本接続は **MS-WFC-4** に残す。

---

## 2. 現状の実装スナップショット

| 箇所 | 現状 | この MS での扱い |
| --- | --- | --- |
| `crates/hw_world/src/mapgen.rs` | `fill_river_from_seed` → `fill_sand_from_river_seed` → `fill_terrain_zones_from_seed` → WFC → `lightweight_validate()` まで実装済み。返却時の `initial_tree_positions` / `forest_regrowth_zones` / `initial_rock_positions` は全て空 | この返却経路に resource generation を追加する |
| `crates/hw_world/src/mapgen/types.rs` | `GeneratedWorldLayout` と `WfcForestZone` は定義済み。`WfcForestZone::contains()` も実装済み | 型は流用し、ここに乗るデータだけ埋める |
| `crates/hw_world/src/terrain_zones.rs` / `world_masks.rs` | `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` と距離場が実装済み | 木は `grass_zone_mask`、岩は `dirt_zone_mask` を優先利用する |
| `crates/hw_world/src/layout.rs` | `TREE_POSITIONS` / `ROCK_POSITIONS` はまだ現役だが、すでに deprecated コメント付き | この MS では削除しない。MS-WFC-4 で startup 切り替え後に撤去する |
| `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs` | 木・岩の初期スポーンは依然として固定座標テーブルを使用 | この MS では未変更。MS-WFC-4 の責務 |
| `crates/bevy_app/src/world/regrowth.rs` / `crates/hw_world/src/regrowth.rs` | 旧 `ForestZone` と `default_forest_zones()` を使う固定ロジックが生きている | この MS では pure data を返すまで。実際の差し替えは MS-WFC-4 |
| `ResourceSpawnCandidates::rock_candidates` | validator 側の再検証経路はあるが、現状 producer がなく空のまま | MS-WFC-3 で ownership を明確化し、実際に埋める |

### 実装上の重要な制約

- `lightweight_validate()` が保証しているのは **地形だけの到達可能性**。  
  木・岩を障害物として置いた後の到達可能性は、現状まだ検証されていない。
- したがって MS-WFC-3 では、**資源配置後に導線を壊していないことを別途確認する仕組み**が必要。

---

## 3. スコープ

### In Scope

- `hw_world` 内での木・岩・森林再生エリアの procedural 生成
- `generate_world_layout()` への resource generation 統合
- `grass_zone_mask` / `dirt_zone_mask` の resource 配置への接続
- 資源配置後の到達性・禁止領域チェック
- `GeneratedWorldLayout` に pure data を載せること

### Out of Scope

- `bevy_app` startup を `GeneratedWorldLayout` 読み出しへ切り替えること
- `RegrowthManager` を `forest_regrowth_zones` 参照へ切り替えること
- `TREE_POSITIONS` / `ROCK_POSITIONS` の削除
- `bevy::Resource` の導入や startup 時の resource 注入

---

## 4. 設計方針

### 4.1 統合ポイント

`generate_world_layout()` はすでに「WFC 生成の単一オーケストレータ」になっているため、この形を維持する。

想定パイプライン:

```rust
1. masks を生成
2. WFC で terrain_tiles を生成
3. lightweight_validate() で terrain-only の成立条件を確認
4. resource generation で trees / rocks / forest zones を作る
5. 資源配置後の到達性・禁止領域を再確認
6. すべて通った attempt だけを採用
```

resource generation 本体は `mapgen.rs` に直書きせず、**pure helper module** に切り出す。

```rust
// crates/hw_world/src/mapgen/resources.rs
pub fn generate_resource_layout(
    terrain_tiles: &[TerrainType],
    masks: &WorldMasks,
    anchors: &AnchorLayout,
    validated_candidates: &ResourceSpawnCandidates,
    seed: u64,
) -> ResourceLayout;
```

### 4.2 候補セルの取り方

#### 木

- 1 次候補: `TerrainType::Grass` かつ `masks.grass_zone_mask == true`
- 2 次候補: 1 次候補が不足する seed に限り、`grass_zone_mask` 外の `Grass` を補助的に使用してよい
- `forest_regrowth_zones` は木の初期配置とは別管理にせず、**同じ生成フェーズ**で決める
- 初期木は必ず `forest_regrowth_zones` の部分集合にする

#### 岩

- 1 次候補: `TerrainType::Dirt` かつ `masks.dirt_zone_mask == true`
- `ResourceSpawnCandidates::rock_candidates` は現状 producer 不在なので、この MS で生成責務を明示する
- 推奨案:
  - resource generation 前半で岩の raw 候補集合を決める
  - その後、到達性チェックで「実際に採用してよい岩配置」に絞る

### 4.3 exclusion zone

以下の領域には木・岩を置かない。

| 領域 | 理由 |
| --- | --- |
| `masks.anchor_mask` | `Site/Yard` 内の進行導線を壊さない |
| `masks.combined_protection_band()` | 序盤導線の安全確保 |
| `masks.river_mask` | River 上に障害物を置かない |
| `masks.final_sand_mask` / `masks.inland_sand_mask` | 砂地資源導線と見た目の一貫性を保つ |
| `anchors.initial_wood_positions` 周辺 1 タイル | 初期木材の視認性・取得導線を確保 |
| `anchors.wheelbarrow_parking.iter_cells()` 周辺 1 タイル | 猫車置き場の操作性を守る |

### 4.4 森林ゾーンの表現

`WfcForestZone` 自体はすでに `mapgen/types.rs` にあるため、MS-WFC-3 では shape を変えない。

```rust
pub struct WfcForestZone {
    pub center: GridPos,
    pub radius: u32,
}
```

- 包含判定は既存どおりチェビシェフ距離ベース
- この MS では **旧 `ForestZone` 型の削除や改名はしない**
- 必要なら `hw_world` 側に pure な変換 helper を追加するが、`bevy_app` への注入は MS-WFC-4

### 4.5 到達性の扱い

現行 validator は木・岩を置く前の地形しか見ていないため、そのままでは不十分。

MS-WFC-3 の完了条件では、少なくとも以下のいずれかを満たすこと:

1. **資源配置後 validator** を追加し、木・岩を障害物として扱って再確認する
2. または resource generation 側で corridor を reservation して、`Site↔Yard` と `Yard→必須資源` を塞がないことを構造的に保証する

このリポジトリのデバッグ規約に従い、単なる密度調整ではなく **成立条件を先に固定する**。

---

## 5. 実装ステップ

### Step 1: `mapgen/resources.rs` を追加

- `ResourceLayout` を定義する
- 木・岩の候補集合を決める pure helper を置く
- seed 決定論に従って zone center / radius / jitter / density を決める

### Step 2: 木配置を `grass_zone_mask` に接続

- `forest_regrowth_zones` を生成
- zone 内から `initial_tree_positions` を選ぶ
- すべての初期木が zone 内にあることを保証する

### Step 3: 岩配置を `dirt_zone_mask` に接続

- 岩候補セルを決める
- `initial_rock_positions` を確定する
- `rock_candidates` の producer 不在をここで解消する

### Step 4: `generate_world_layout()` に統合

- `lightweight_validate()` 成功後に resource generation を呼ぶ
- 成功した attempt だけ `GeneratedWorldLayout` の 3 フィールドを埋めて返す
- fallback 時は引き続き空配列でよいが、その前提を明文化する

### Step 5: 資源配置後の成立条件チェック

- 木・岩配置後に導線が壊れていないか確認する
- 不成立ならその attempt を破棄して次の sub-seed へ進む

---

## 6. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/resources.rs` (新規) | `generate_resource_layout()` / `ResourceLayout` / 候補選定・配置ロジック |
| `crates/hw_world/src/mapgen.rs` | `pub mod resources;` を追加し、`generate_world_layout()` に resource generation と post-resource validation を統合 |
| `crates/hw_world/src/mapgen/types.rs` | 必要なら `ResourceLayout` 公開型や変換 helper を追加。既存 `GeneratedWorldLayout` / `WfcForestZone` は流用 |
| `crates/hw_world/src/regrowth.rs` | 必要なら `WfcForestZone` → legacy `ForestZone` の pure 変換 helper を追加。ただし本接続は MS-WFC-4 |
| `crates/bevy_app/src/world/regrowth.rs` | 原則この MS では未変更。MS-WFC-4 で `RegrowthManager` の入力切り替え |
| `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs` | 原則この MS では未変更。MS-WFC-4 で `GeneratedWorldLayout` 読み出しへ切り替え |

`crates/hw_world/src/layout.rs` の deprecated コメントは **すでに追加済み** なので、この MS の必須変更対象ではない。

---

## 7. 完了条件チェックリスト

- [ ] `generate_world_layout()` が成功レイアウトで `initial_tree_positions` / `forest_regrowth_zones` / `initial_rock_positions` を空ではなく返す
- [ ] 木が `grass_zone_mask` を主な優先領域として生成される
- [ ] 岩が `dirt_zone_mask` を主な優先領域として生成される
- [ ] 木・岩が `anchor_mask` / `combined_protection_band()` / `river_mask` / `final_sand_mask` / `inland_sand_mask` に入らない
- [ ] `initial_tree_positions` が必ず `forest_regrowth_zones` の部分集合になっている
- [ ] `ResourceSpawnCandidates::rock_candidates` の producer 不在が解消されている
- [ ] 資源配置後も `Site↔Yard` と `Yard→必須資源` の到達可能性が維持される
- [ ] fallback 経路では資源配置が空でもよいこと、startup 側がそれを前提に扱うことが文書化されている
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 8. テスト

```rust
#[test]
fn trees_not_in_exclusion_zone() {
    let layout = generate_world_layout(TEST_SEED_A);
    for pos in &layout.initial_tree_positions {
        assert!(!layout.masks.anchor_mask.get(*pos));
        assert!(!layout.masks.combined_protection_band().get(*pos));
        assert!(!layout.masks.river_mask.get(*pos));
        assert!(!layout.masks.final_sand_mask.get(*pos));
        assert!(!layout.masks.inland_sand_mask.get(*pos));
    }
}

#[test]
fn trees_are_inside_some_forest_zone() {
    let layout = generate_world_layout(TEST_SEED_A);
    for pos in &layout.initial_tree_positions {
        assert!(layout.forest_regrowth_zones.iter().any(|z| z.contains(*pos)));
    }
}

#[test]
fn rocks_not_in_exclusion_zone() {
    let layout = generate_world_layout(TEST_SEED_A);
    for pos in &layout.initial_rock_positions {
        assert!(!layout.masks.anchor_mask.get(*pos));
        assert!(!layout.masks.combined_protection_band().get(*pos));
        assert!(!layout.masks.river_mask.get(*pos));
    }
}

#[test]
fn resource_layout_keeps_required_paths_open() {
    let layout = generate_world_layout(TEST_SEED_A);
    assert!(!layout.used_fallback);
    // 具体的な post-resource validator 呼び出しをここに置く
}
```

---

## 9. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
cargo check --workspace
cargo clippy --workspace
```

手動確認:

- `cargo run` で seed を変えると木・岩の分布が変化する
- `Site/Yard` とその保護帯に木・岩が食い込まない
- 旧 startup 経路のままでも、MS-WFC-4 着手時に読み替えるための pure data が揃っている

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-05` | `Codex` | 現状実装に合わせて全面更新。`terrain_zones` 前提、`WfcForestZone` 先行実装、deprecated コメント追加済み、startup/regrowth 未接続、`rock_candidates` producer 不在を反映 |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
