# MS-WFC-2d: River 派生の砂マスク再設計

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2d-river-driven-sand-mask` |
| ステータス | `Draft` |
| 作成日 | `2026-04-05` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `river_mask` / `post_process_tiles()` / `lightweight_validate()` が実装済み（MS-WFC-2b/2c 完了） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| やること | `river_mask` から **8 近傍 shoreline sand mask** を deterministic に生成し、さらに **連続した non-sand carve** を差し引いた `final_sand_mask` を `WorldMasks` に保持する |
| WFC との関係 | WFC 本体には weighted な Sand hard constraint を追加しない。`final_sand_mask` は `run_wfc()` の入力文脈として渡し、**後段 `post_process_tiles()` で最終地形に強制反映**する |
| 置き換えるもの | 「Sand は WFC が出したものを cardinal-adjacent 条件で残す」という 2b の後処理方針 |
| 目的 | diagonal Sand を仕様として許容しつつ、砂地の形は River 派生で deterministic に制御する |
| やらないこと | retry 条件の追加、WFC crate の置換、validator での自動修復、木・岩配置 |

---

## 1. 背景

現状の 2b 実装では、`Sand` は WFC 結果から選ばれ、`post_process_tiles()` が
「River に **4 近傍**で接していない `Sand`」を `Grass/Dirt` に落としている。

この方式には以下の欠点がある。

- 砂地の形が River 由来の deterministic な説明になっていない
- diagonal Sand を仕様として扱えない
- Sand の見た目が `wfc` の weighted pattern 出力に引きずられる
- `wfc` 0.10.x の priority queue 制約により、weighted `Sand` を `ForbidPattern` で直接制御しづらい

今回の方針では、**砂は WFC の「候補」ではなく River 由来の派生マスク**として扱う。

---

## 2. 目的

- `Sand` の責務を「WFC がたまたま選ぶ地形」から「River の岸辺を表す deterministic 地形」へ移す
- diagonal 方向の砂を仕様として許容する
- 一方で 8 近傍をそのまま全面 `Sand` にせず、seed 由来の **連続した non-sand carve** で砂浜の抜けを作る
- weighted pattern の追加制約を WFC 本体へ持ち込まず、現行アダプタ構造を維持する

---

## 3. 設計方針

### 3.1 生成順

砂に関する生成順を以下へ変更する。

1. `AnchorLayout::fixed()`
2. `WorldMasks::from_anchor()`
3. `fill_river_from_seed(master_seed)` で `river_mask` / `river_centerline`
4. `fill_sand_from_river_seed(master_seed)` で以下を生成
   - `sand_candidate_mask`
   - `sand_carve_mask`
   - `final_sand_mask`
5. WFC は River 固定を前提に Grass/Dirt を中心に collapse
6. `post_process_tiles()` が `final_sand_mask` を最終地形へ反映

重要: **`final_sand_mask` は WFC に「直接 forbid/fix する hard constraint」としては渡さない。**
`WorldMasks` の一部として `run_wfc()` に渡し、後段 `post_process_tiles()` で最終地形を確定する。

### 3.2 なぜ WFC 側で直接やらないか

- 現行 `wfc` 0.10.x では weighted pattern の `forbid_pattern` が stale entry 問題を起こしうる
- `Sand` は weighted pattern なので、River 同様の制約適用をそのまま流用できない
- retry を増やして解く方針は採らない

したがって本 MS では、**WFC の役割を Grass/Dirt の分布に絞り、Sand は River 派生 mask として外側で決める。**

---

## 4. `WorldMasks` の拡張

`crates/hw_world/src/world_masks.rs` に以下を追加する。

```rust
pub struct WorldMasks {
    // 既存
    pub site_mask: BitGrid,
    pub yard_mask: BitGrid,
    pub anchor_mask: BitGrid,
    pub river_protection_band: BitGrid,
    pub rock_protection_band: BitGrid,
    pub tree_dense_protection_band: BitGrid,
    pub river_mask: BitGrid,
    pub river_centerline: Vec<GridPos>,

    // 新規（MS-WFC-2d）
    pub sand_candidate_mask: BitGrid,
    pub sand_carve_mask: BitGrid,
    pub final_sand_mask: BitGrid,
}
```

### フィールドの意味

| フィールド | 意味 |
| --- | --- |
| `sand_candidate_mask` | `river_mask` の 8 近傍から作る「砂にしてよい元候補」 |
| `sand_carve_mask` | seed 由来で candidate から削る連続 non-sand 領域 |
| `final_sand_mask` | `sand_candidate_mask - sand_carve_mask`。最終的に `Sand` にしたいセル |

---

## 5. 砂マスク生成アルゴリズム

### 5.1 `sand_candidate_mask`

`river_mask` の各セルから 8 近傍を見て、以下を満たすセルを `sand_candidate_mask = true` にする。

- `river_mask` 自身ではない
- `anchor_mask` ではない
- `river_protection_band` ではない
- マップ範囲内である

擬似コード:

```rust
const EIGHT_DIRS: [(i32, i32); 8] = [
    (0, -1), (1, 0), (0, 1), (-1, 0),
    (1, -1), (1, 1), (-1, 1), (-1, -1),
];

for each river cell r:
    for each dir in EIGHT_DIRS:
        let p = r + dir;
        if in_bounds(p)
            && !river_mask.get(p)
            && !anchor_mask.get(p)
            && !river_protection_band.get(p)
        {
            sand_candidate_mask.set(p, true);
        }
```

### 5.2 `sand_carve_mask`

candidate 全面をそのまま `Sand` にすると厚すぎるため、seed deterministic な
**連続した non-sand 領域**を candidate 内に作る。

初版は以下の簡易アルゴリズムでよい。

1. `sand_candidate_mask` のセル一覧から carve 起点を seed で数個選ぶ
2. 各起点から **4 近傍 flood fill / random walk** のどちらかで一定長だけ広げる
3. 広げたセルを `sand_carve_mask = true` にする
4. carve は `final_sand_mask` が空にならないよう上限面積を持つ

推奨定数:

```rust
pub const SAND_CARVE_SEED_COUNT_MIN: u32 = 2;
pub const SAND_CARVE_SEED_COUNT_MAX: u32 = 5;
pub const SAND_CARVE_MAX_RATIO_PERCENT: u32 = 35;
pub const SAND_CARVE_REGION_SIZE_MIN: u32 = 6;
pub const SAND_CARVE_REGION_SIZE_MAX: u32 = 24;
```

理由:

- carve 自体は **連続している**必要があるが、candidate 生成の都合で砂浜全体は 8 近傍でよい
- carve を 4 近傍で広げると、形が素直で制御しやすい
- 将来見た目を詰めるなら random walk から flood fill、またはその逆へ差し替えやすい

### 5.3 `final_sand_mask`

```rust
final_sand_mask = sand_candidate_mask.clone();
final_sand_mask -= sand_carve_mask;
```

完了条件:

- `final_sand_mask` は `river_mask` と交差しない
- `final_sand_mask` は `anchor_mask` と交差しない
- `final_sand_mask` は `river_protection_band` と交差しない
- `final_sand_mask.count_set() > 0`

---

## 6. WFC との統合

### 6.1 `run_wfc()` の責務

`run_wfc()` の基本方針は維持する。

- WFC 本体では River の固定・マスク外 River 禁止のみ直接適用
- `Sand` の fixed / forbid は `ForbidPattern` に入れない
- `post_process_tiles()` が最終地形へ `final_sand_mask` を適用する

### 6.2 `post_process_tiles()` の変更

現行の「River に 4 近傍接触しない `Sand` を潰す」ロジックを以下へ置き換える。

```rust
for each cell:
    if river_mask.get(pos):
        keep River
    else if final_sand_mask.get(pos):
        tiles[idx] = TerrainType::Sand
    else if tiles[idx] == TerrainType::Sand:
        tiles[idx] = random_grass_or_dirt(rng)
```

これで以下が保証される。

- `final_sand_mask` 上は必ず `Sand`
- `final_sand_mask` 外の stray `Sand` は残らない
- WFC の weighted `Sand` 出力は「Grass/Dirt と同等に落とせる候補」に戻る

### 6.3 `build_pattern_table()` の扱い

`Sand` を WFC の主要地形から外す方向に寄せるため、`WEIGHT_SAND` は次のどちらかにする。

1. **最小変更案**: 現状維持または低めに落とす
2. **整理案**: `WEIGHT_SAND = 1` など最低限まで落とし、最終的な Sand 量は `final_sand_mask` が支配する

初版は **最小変更案**でよい。2d の主目的は shape control であり、WFC の地形味付け最適化ではない。

---

## 7. validator の見直し

`MS-WFC-2c` の debug validator は、2d 採用後に以下を見直す。

### 廃止または役割変更するチェック

- `check_sand_river_adjacency_ratio`
- `check_sand_diagonal_only_contacts`

理由:

- diagonal Sand は 2d では **仕様**であり、警告対象ではない
- 砂の品質は「4 近傍比率」ではなく `final_sand_mask` との整合で測る方が正しい

### 新規に入れるチェック

```rust
check_final_sand_mask_applied(layout, warnings);
check_no_stray_sand_outside_mask(layout, warnings);
check_sand_mask_not_in_anchor_or_band(layout, warnings);
```

内容:

- `final_sand_mask == true` のセルがすべて `TerrainType::Sand`
- `final_sand_mask == false` のセルに `TerrainType::Sand` が残っていない
- `final_sand_mask` が `anchor_mask` / `river_protection_band` と交差しない

lightweight validator の「砂源到達可能性」は、従来どおり最終 `terrain_tiles` から判定してよい。

---

## 8. 変更ファイル

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/world_masks.rs` | `sand_candidate_mask` / `sand_carve_mask` / `final_sand_mask` を追加し、seed から砂マスクを埋める API を追加 |
| `crates/hw_world/src/river.rs` | River 派生の 8 近傍 sand candidate 生成と carve helper を実装 |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | `post_process_tiles()` を `final_sand_mask` 主導へ変更。`Sand` の stray 除去を mask ベースへ置換 |
| `crates/hw_world/src/mapgen/validate.rs` | diagonal-only sand warning を廃止または役割変更し、mask 整合チェックへ差し替え |
| `docs/plans/3d-rtt/wfc-terrain-generation-plan-2026-04-01.md` | F4 と subplan table を更新 |
| `docs/plans/3d-rtt/wfc-ms2c-validator.md` | 2d 採用後の validator 方針へ前後関係を更新 |

---

## 9. 完了条件

- [ ] `WorldMasks` に `sand_candidate_mask` / `sand_carve_mask` / `final_sand_mask` が追加されている
- [ ] 同一 seed で `final_sand_mask` が deterministic
- [ ] `final_sand_mask` は `river_mask` / `anchor_mask` / `river_protection_band` と交差しない
- [ ] `post_process_tiles()` が `final_sand_mask` 上を強制的に `Sand` にしている
- [ ] `final_sand_mask` 外に stray `Sand` が残らない
- [ ] debug validator が diagonal-only sand を warning しない
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 10. テスト

最低限、以下のテストを追加する。

```rust
#[test]
fn sand_mask_is_deterministic_for_same_seed() {}

#[test]
fn sand_mask_does_not_overlap_anchor_or_protection_band() {}

#[test]
fn post_process_forces_final_sand_mask_to_sand() {}

#[test]
fn post_process_removes_stray_sand_outside_mask() {}
```

golden seed には「straight river」「winding river」「tight band」の 3 系統を入れる。

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-05` | `Codex` | 初版作成。River 派生の 8 近傍 sand candidate mask、連続 non-sand carve、`post_process_tiles()` による最終反映方針を定義。 |
