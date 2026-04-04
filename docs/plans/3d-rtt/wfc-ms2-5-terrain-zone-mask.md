# MS-WFC-2.5: 地形バイアスゾーンマスク生成

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2-5-terrain-zone-mask` |
| ステータス | `Ready` |
| 作成日 | `2026-04-04` |
| 最終更新日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2e-sand-shore-shape.md`](wfc-ms2e-sand-shore-shape.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `fill_sand_from_river_seed()` と `final_sand_mask` 反映が実装済み（**MS-WFC-2d/2e 完了必須**） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| 解決したいこと | WFC の Grass/Dirt 分布が全面均一になりやすく、地形にメリハリがない。また砂浜以外の少量の砂地（内陸砂）を Grass エリアに点在させたい |
| 主変更 | アンカー距離場（D）を起点に flood fill（B）で `grass_zone_mask` / `dirt_zone_mask` を生成し、`post_process_tiles` で確率的バイアス（A）を適用する。加えて `inland_sand_mask` を `grass_zone_mask` 内に生成し、4 近傍 Grass チェック付きで post_process する |
| 維持するもの | `final_sand_mask` / `river_mask` の優先度、`deterministic` 契約、WFC 本体の隣接制約 |
| 期待効果 | アンカー近傍に Dirt 帯、マップ遠端に Grass 帯が出現。Grass エリアに小さな砂地が点在し、地形の「場所の性格」が生まれる |
| 内陸砂の隣接制約 | `inland_sand_mask` セルの 4 近傍は必ず Grass（River・Dirt・既存 Sand と隣接しない） |
| MS-WFC-3 との接続 | `grass_zone_mask` / `dirt_zone_mask` を `generate_resource_layout()` に渡し、フォレスト・岩配置の優先エリアとして再利用する |
| やらないこと | WFC 制約への新 hard constraint 追加、Sand/River の変更、アンカー位置の変更 |

---

## 1. 背景

現行の WFC は全セル共通の重み（WEIGHT_GRASS=5, WEIGHT_DIRT=2）で動作するため、Grass/Dirt の分布がマップ全域でほぼ均一になる。

これはコントラストがなく、「廃墟建設現場の近くは荒れた土地」「マップ遠端は手つかずの草原」といった地形の性格が出ない。また MS-WFC-3 でフォレスト・岩を配置する際も、適切な優先エリアの情報がなく均一な分散になりやすい。

---

## 2. 目的

- `grass_zone_mask` / `dirt_zone_mask` を `WorldMasks` に追加し、地形バイアスの空間情報を持たせる
- アンカー（Site/Yard）に近いエリアを Dirt 帯、遠いエリアを Grass 帯として生成する
- MS-WFC-3 が `generate_resource_layout()` でこれらのマスクを参照し、フォレスト・岩配置の優先エリアとして再利用できる設計にする
- WFC 本体・Sand/River の決定論的契約は一切変更しない
- `inland_sand_mask` を `WorldMasks` に追加し、Grass エリアに点在する小砂地を生成する
  - 隣接ルール: inland sand の 4 近傍は Grass のみ（River・Dirt・既存 Sand と隣接させない）
  - WFC テーブルの変更は不要（post_process の 4 近傍チェックで保証する）

---

## 3. 設計方針

4つの処理を順に適用する。

```
D: anchor_distance_field → Dirt/Grass ゾーンの起点選択
          ↓
B: bounded flood fill → grass_zone_mask / dirt_zone_mask
          ↓
  inland sand: grass_zone_mask 内から小パッチ → inland_sand_mask
          ↓
A: post_process_tiles での確率的バイアス + inland sand 配置
```

### 3.1 D: アンカー距離場による起点選択

`anchor_mask`（site | yard）からの 4 近傍 BFS 距離場を計算する（`compute_protection_band` と同型）。

- **Dirt ゾーン起点**: `dist` が `ZONE_DIRT_DIST_MIN..=ZONE_DIRT_DIST_MAX` に収まる許可セル群から seed を選ぶ
  - 意味: 保護帯のすぐ外（文明に近い荒れた地）
- **Grass ゾーン起点**: `dist >= ZONE_GRASS_DIST_MIN` の許可セル群から seed を選ぶ
  - 意味: アンカーから遠い手つかずの野原

**許可セル**: `!anchor_mask && !river_mask && !river_protection_band && !final_sand_mask`

### 3.2 B: Flood fill パッチ生成

各起点から 4 近傍 bounded flood fill でパッチを作り、`grass_zone_mask` / `dirt_zone_mask` に加算する。

- 許可セルのみ展開（上記と同じ制約セット）
- 各パッチは面積上限（`ZONE_*_REGION_AREA_MAX`）で打ち切る
- 2 ゾーンが重複した場合: Dirt 優先（`grass_zone_mask` から重複セルを除外）

### 3.3 内陸砂マスク（`inland_sand_mask`）

`grass_zone_mask` 確定後に、その内側から小パッチを生成する。

**生成**:
- 候補: `grass_zone_mask && !anchor_mask && !river_mask && !river_protection_band && !final_sand_mask`
- seed 由来で `INLAND_SAND_PATCH_COUNT_MIN..=MAX` 個の起点を選択
- 各起点から bounded flood fill（面積 `INLAND_SAND_PATCH_AREA_MAX` 以下、許可セルのみ）
- 各パッチの全隣接セルが `grass_zone_mask` に含まれる場合のみ採用（パッチ全体を棄却 or 採用）

**post_process_tiles での配置（§3.4 より後）**:
- `inland_sand_mask` セルで `tile != River && tile != Sand` の場合、4 近傍の `tiles` をすべてチェック
- 全近傍が `Grass` なら Sand に変換、1 つでも非 Grass があれば変換しない
- これにより River・Dirt・既存 Sand との隣接を完全に防ぐ

### 3.4 A: 確率的バイアス

`post_process_tiles()` 内でゾーンマスクを参照する。ハードな上書きではなく **確率的フリップ** にすることでゾーン境界を有機的に見せる。

- `grass_zone_mask` かつ `tile == Dirt`: `ZONE_GRASS_ENFORCE_PERCENT` % の確率で Grass に変換
- `dirt_zone_mask` かつ `tile == Grass`: `ZONE_DIRT_ENFORCE_PERCENT` % の確率で Dirt に変換
- River / Sand は既存ロジックで保護済みのため skip

**post_process 適用順序（全体）**:

```
1. river_mask → River 固定
2. final_sand_mask → 砂浜 Sand 強制
3. stray Sand → Grass/Dirt フリップ
4. zone bias（grass/dirt zone mask, §3.4）
5. inland_sand_mask → 4 近傍 Grass チェック付き Sand 変換（§3.3）
```

inland sand を最後にすることで、zone bias 確定後の tile 状態に対して近傍チェックをかけられる。

---

## 4. 定数（`terrain_zones.rs` に定義）

```rust
/// Dirt ゾーン起点のアンカー距離下限（保護帯 PROTECTION_BAND_RIVER_WIDTH=3 の外）
pub const ZONE_DIRT_DIST_MIN: u32 = 5;
/// Dirt ゾーン起点のアンカー距離上限
pub const ZONE_DIRT_DIST_MAX: u32 = 12;
/// Grass ゾーン起点のアンカー距離下限
pub const ZONE_GRASS_DIST_MIN: u32 = 20;

/// Dirt ゾーン起点数の下限
pub const ZONE_DIRT_SEED_COUNT_MIN: u32 = 2;
/// Dirt ゾーン起点数の上限
pub const ZONE_DIRT_SEED_COUNT_MAX: u32 = 5;
/// Grass ゾーン起点数の下限
pub const ZONE_GRASS_SEED_COUNT_MIN: u32 = 2;
/// Grass ゾーン起点数の上限
pub const ZONE_GRASS_SEED_COUNT_MAX: u32 = 4;

/// 1 Dirt パッチの面積上限（セル数）
pub const ZONE_DIRT_REGION_AREA_MAX: usize = 180;
/// 1 Grass パッチの面積上限（セル数）
pub const ZONE_GRASS_REGION_AREA_MAX: usize = 280;

/// Grass ゾーン内 Dirt → Grass に変換する確率（%）
pub const ZONE_GRASS_ENFORCE_PERCENT: u32 = 85;
/// Dirt ゾーン内 Grass → Dirt に変換する確率（%）
pub const ZONE_DIRT_ENFORCE_PERCENT: u32 = 85;

// ── 内陸砂定数 ────────────────────────────────────────────────────────────────
/// 生成する内陸砂パッチ数の下限
pub const INLAND_SAND_PATCH_COUNT_MIN: u32 = 3;
/// 生成する内陸砂パッチ数の上限
pub const INLAND_SAND_PATCH_COUNT_MAX: u32 = 6;
/// 1 パッチの面積上限（セル数）。少量に抑える
pub const INLAND_SAND_PATCH_AREA_MAX: usize = 5;
```

---

## 5. データ構造と API 変更方針

### 5.1 `WorldMasks`（`world_masks.rs`）

フィールド追加（`final_sand_mask` の後）:

```rust
/// アンカーから遠い地点を起点にした Grass バイアスゾーン
pub grass_zone_mask: BitGrid,
/// アンカーに近い地点を起点にした Dirt バイアスゾーン
pub dirt_zone_mask: BitGrid,
/// grass_zone_mask 内に生成した内陸砂パッチ（砂浜とは独立）
pub inland_sand_mask: BitGrid,
```

`from_anchor()` 内で `BitGrid::map_sized()` として初期化（`fill_terrain_zones_from_seed` で設定）。

新メソッド追加:

```rust
/// `fill_sand_from_river_seed()` 完了後に呼ぶ。
/// anchor 距離場と final_sand_mask を参照し、terrain zone masks と inland_sand_mask を生成する。
pub fn fill_terrain_zones_from_seed(&mut self, seed: u64);
```

呼び出し順序:
```
from_anchor()
  → fill_river_from_seed()
    → fill_sand_from_river_seed()
      → fill_terrain_zones_from_seed()   ← new（zone mask + inland_sand_mask を両方生成）
```

### 5.2 `terrain_zones.rs`（新規）

```rust
pub fn generate_terrain_zone_masks(
    seed: u64,
    anchor_mask: &BitGrid,
    river_mask: &BitGrid,
    river_protection_band: &BitGrid,
    final_sand_mask: &BitGrid,
) -> (BitGrid, BitGrid); // (grass_zone_mask, dirt_zone_mask)
```

内部構成:
- `compute_anchor_distance_field(...)` — `compute_protection_band` と同型の BFS 距離場
- `pick_zone_seeds(...)` — 距離条件でフィルタ後に RNG で選択
- `flood_fill_zone_patches(...)` — Sand の carve/grow と同じ bounded flood fill パターン
- 重複解決: `grass_zone_mask &= !dirt_zone_mask`
- `generate_inland_sand_mask(grass_zone_mask, ...)` — grass_zone 内の小パッチ生成

### 5.3 `wfc_adapter.rs`（`post_process_tiles` 拡張）

既存の stray Sand 処理の後に追加（§3.4 と §3.3 の順序で実装）:

```rust
// Step 4: Zone bias（確率的フリップ）
if masks.grass_zone_mask.get((x, y)) && tiles[idx] == TerrainType::Dirt {
    if rng.gen_range(0..100) < ZONE_GRASS_ENFORCE_PERCENT {
        tiles[idx] = TerrainType::Grass;
    }
} else if masks.dirt_zone_mask.get((x, y)) && tiles[idx] == TerrainType::Grass {
    if rng.gen_range(0..100) < ZONE_DIRT_ENFORCE_PERCENT {
        tiles[idx] = TerrainType::Dirt;
    }
}
```

inland sand の適用は **zone bias ループの外**で別パスとして行う（近傍チェックが zone bias 後の状態を参照するため）:

```rust
// Step 5: inland sand（4 近傍 Grass チェック付き）
for y in 0..MAP_HEIGHT {
    for x in 0..MAP_WIDTH {
        let idx = (y * MAP_WIDTH + x) as usize;
        if !masks.inland_sand_mask.get((x, y)) { continue; }
        if tiles[idx] == TerrainType::River || tiles[idx] == TerrainType::Sand { continue; }
        let all_grass = CARDINAL_DIRS.iter().all(|&(dx, dy)| {
            let nx = x + dx; let ny = y + dy;
            if nx < 0 || nx >= MAP_WIDTH || ny < 0 || ny >= MAP_HEIGHT { return true; }
            tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::Grass
        });
        if all_grass { tiles[idx] = TerrainType::Sand; }
    }
}
```

### 5.4 `mapgen.rs`

`generate_world_layout()` 内で `fill_sand_from_river_seed()` の後に呼び出し追加:

```rust
masks.fill_terrain_zones_from_seed(master_seed);
```

---

## 6. MS-WFC-3 との接続設計

`generate_resource_layout()` のシグネチャは変更しない。`WorldMasks` に `grass_zone_mask` / `dirt_zone_mask` が追加されるため、`masks` 経由で自然に参照可能になる。

| resource | 優先エリア |
| --- | --- |
| 木・フォレスト再生ゾーン | `grass_zone_mask` が true のセル |
| 岩 | `dirt_zone_mask` が true のセル |
| どちらも false のセル | 既存の terrain_type ベース判定にフォールバック |

---

## 7. 実装ステップ

### Step 1: `terrain_zones.rs` 作成

- `compute_anchor_distance_field(...)` 実装
- `pick_zone_seeds(...)` 実装（Dirt: 距離 5..=12、Grass: 距離 ≥ 20）
- `flood_fill_zone_patches(...)` 実装（Sand の carve/grow 同型）
- `generate_inland_sand_mask(...)` 実装（grass_zone 内の小パッチ、面積 ≤ 5）
- `generate_terrain_zone_masks(...)` でまとめ（zone mask + inland_sand_mask を返す）

### Step 2: `WorldMasks` 拡張

- `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` フィールド追加
- `from_anchor()` に初期値追加
- `fill_terrain_zones_from_seed()` メソッド追加

### Step 3: `mapgen.rs` 呼び出し追加

- `generate_world_layout()` で `fill_terrain_zones_from_seed` を呼ぶ

### Step 4: `post_process_tiles` 拡張

- ゾーンバイアスの確率的フリップを追加（Step 4）
- inland sand の 4 近傍チェック付き Sand 変換を別パスで追加（Step 5）
- fallback_terrain にも zone / inland_sand 適用

### Step 5: テストを追加

- deterministic（同一 seed で同一ゾーン・同一 inland_sand）
- overlap 禁止（`grass_zone_mask & dirt_zone_mask` = 空）
- ゾーンと River / anchor / Sand の交差なし
- 代表 seed で「Dirt ゾーンが anchor 近傍に存在する」「Grass ゾーンが anchor から遠い位置に存在する」
- inland_sand セルの 4 近傍が全て Grass になっていること（post_process 後に確認）
- inland_sand が `final_sand_mask` / `river_mask` / `anchor_mask` と交差しないこと

### Step 6: docs 同期

- `world_layout.md` に地形ゾーン生成の説明追加
- 親計画 / ロードマップの MS-WFC-2.5 行を更新

---

## 8. 変更ファイル

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/terrain_zones.rs` | 新規。距離場・seed 選択・flood fill・zone mask 生成・inland sand パッチ生成 |
| `crates/hw_world/src/lib.rs` | `pub mod terrain_zones;` 追加 |
| `crates/hw_world/src/world_masks.rs` | `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` フィールド追加、`fill_terrain_zones_from_seed` 追加 |
| `crates/hw_world/src/mapgen.rs` | `fill_terrain_zones_from_seed` 呼び出し追加 |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | `post_process_tiles` のゾーンバイアス（Step 4）・inland sand 別パス（Step 5）追加、`fallback_terrain` にも反映 |
| `docs/world_layout.md` | 地形ゾーン生成説明追加 |
| `docs/plans/3d-rtt/wfc-terrain-generation-plan-2026-04-01.md` | MS-WFC-2.5 行を追加・ステータス更新 |
| `docs/plans/3d-rtt/milestone-roadmap.md` | MS-WFC-2.5 行を追加 |

---

## 9. 完了条件

- [ ] 同一 seed で `grass_zone_mask` / `dirt_zone_mask` が deterministic
- [ ] `grass_zone_mask & dirt_zone_mask` が空（重複なし）
- [ ] どちらのゾーンも `river_mask` / `anchor_mask` / `river_protection_band` / `final_sand_mask` と交差しない
- [ ] 代表 seed で anchor 近傍（dist ≤ 12）に Dirt ゾーンセルが存在する
- [ ] 代表 seed で anchor 遠端（dist ≥ 20）に Grass ゾーンセルが存在する
- [ ] マップ全体が単一ゾーンに占拠されない（両ゾーンの合計が許可セルの 60% 以下）
- [ ] `inland_sand_mask` が `final_sand_mask` / `river_mask` / `anchor_mask` と交差しない
- [ ] post_process 後、`inland_sand_mask` 上の Sand セルの 4 近傍が全て Grass（代表 seed で確認）
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 10. 検証

```bash
cargo test -p hw_world
cargo check --workspace
cargo clippy --workspace
```

代表 seed で目視確認:
- anchor（中央付近）の周囲に Dirt 帯が見えるか
- マップ四隅・遠端に Grass 帯が見えるか
- Sand/River のエリアがゾーンで上書きされていないか
- Grass 帯の中に小さな砂地が点在しているか（砂浜とは離れた位置）
- 内陸砂が Dirt / River / 砂浜 Sand に隣接していないか

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-04` | `Codex` | 初版作成。D（アンカー距離場）→ B（flood fill）→ A（確率的バイアス）3 層設計を整理。MS-WFC-3 との接続設計を明記。 |
| `2026-04-04` | `Codex` | 内陸砂（inland_sand_mask）追加。grass_zone 内の小パッチ生成と「4 近傍 Grass のみ」隣接ルールを §3.3 / §4 / §5 / §7〜10 に反映。 |
