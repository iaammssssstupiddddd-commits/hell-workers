# MS-WFC-2e: 砂浜輪郭依存の緩和

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2e-sand-shore-shape` |
| ステータス | `完了`（`crates/hw_world/src/river.rs`, `crates/hw_world/src/world_masks.rs`） |
| 作成日 | `2026-04-04` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2d-river-driven-sand-mask.md`](wfc-ms2d-river-driven-sand-mask.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `WorldMasks::fill_sand_from_river_seed()` と `final_sand_mask` 反映が実装済み（**MS-WFC-2d 完了必須**） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| 解決したいこと | 現在の砂浜が `river_mask` の 8 近傍 1 層候補に強く依存し、川の輪郭をそのままなぞる見た目になりやすい |
| 実装内容 | **完了**: `sand_candidate_mask` を「River 輪郭の 8 近傍リングを核にした距離場ベースの岸帯 + seed 由来の加算的な浜の膨らみ」へ更新した |
| 維持するもの | `final_sand_mask` の deterministic 契約、`non-sand carve`、`post_process_tiles()` / `fallback_terrain()` による後段反映、WFC 非依存の砂責務 |
| 期待効果 | 砂浜が River 輪郭のトレースから外れ、より面として見える。対角許容・連続した非砂領域との両立は維持する |
| やらないこと | retry 方針の変更、WFC への新 hard constraint 追加、資源配置ロジック変更、木/岩生成の前倒し |

---

## 1. 背景

MS-WFC-2d により、`Sand` は WFC 出力ではなく `river_mask` 由来の deterministic mask になった。これは責務分離として正しいが、現行実装の候補生成は次の性質を持つ。

- `build_sand_candidate_mask()` が `river_mask` の **8 近傍 1 層**だけを候補化する
- `build_sand_carve_mask()` は候補を **減算**するだけで、外側へ面を広げる操作を持たない
- そのため最終形が「川の輪郭 + 欠け」の見た目になりやすい

結果として、砂浜が「岸辺の帯」ではなく「River 輪郭の縁取り」に見えやすい。これは deterministic 契約そのものではなく、**candidate 生成アルゴリズムの表現力不足**が原因である。

---

## 2. 目的

- `final_sand_mask` を川輪郭トレース中心の形から、**岸帯・砂州・ふくらみを持つ面**へ寄せる
- `non-sand carve` を維持しつつ、砂浜生成に **加算ステップ**を導入する
- `Sand` を WFC 外で決める現行アーキテクチャは維持する
- 100x100 グリッド前提で、WFC コストを増やさずに見た目だけ改善する

---

## 3. 設計方針

### 3.1 基本方針

`final_sand_mask` の生成順を次へ変更する。

1. `river_mask` から **2d の 8 近傍 shoreline 契約を保った river distance field** を作る
2. 距離 1..=2 を **base shoreline mask** として確保する
3. seed 由来で選んだ少数の起点から、距離 3..=4 までの **sand growth** を加算する
4. その後に既存の `non-sand carve` を適用する
5. `final_sand_mask` を `post_process_tiles()` / `fallback_terrain()` で最終反映する

重要なのは、現在の `candidate - carve` 一辺倒をやめ、**`base + growth - carve`** に変えること。これにより、非砂領域実装とは競合せず、役割分担が明確になる。

### 3.2 なぜこの形か

- **距離場**を使うと、2d の 8 近傍 shoreline を保ちながら、その外側にだけ面としての岸帯を足せる
- **growth** を少数の seed 起点に限定すると、全周一様に太い帯にならず、局所的な砂浜の膨らみを作れる
- **carve を最後に残す**ことで、2d の「連続した non-sand エリアで単調さを崩す」意図をそのまま生かせる

---

## 4. 提案アルゴリズム

### 4.0 定数（`river.rs` に追加・修正）

```rust
// ── 砂マスク定数（2e 追加・変更分）──────────────────────────────────────────

/// 距離場で許可セルにラベルを付与する最大距離（これを超えたセルは u32::MAX）
pub const SAND_SHORE_MAX_DISTANCE: u32 = 4;
/// base_candidate_mask に含める最小距離
pub const SAND_BASE_DIST_MIN: u32 = 1;
/// base_candidate_mask に含める最大距離
pub const SAND_BASE_DIST_MAX: u32 = 2;
/// growth 起点数の下限
pub const SAND_GROWTH_SEED_COUNT_MIN: u32 = 3;
/// growth 起点数の上限
pub const SAND_GROWTH_SEED_COUNT_MAX: u32 = 8;
/// growth flood が侵入してよい距離場の上限
pub const SAND_GROWTH_DIST_MAX: u32 = 4;
/// growth flood が起点から伸びてよい最大ステップ数
pub const SAND_GROWTH_STEP_LIMIT: usize = 6;
/// 1 growth region の最大面積（セル数）
pub const SAND_GROWTH_REGION_AREA_MAX: usize = 30;

// ── carve 定数（2e で candidate 面積増に合わせ再調整）────────────────────────
// ※ 変わる定数のみ。変わらないものは 2d の値を維持する。
// 2d → 2e  SAND_CARVE_SEED_COUNT_MIN: 2 → 3
// 2d → 2e  SAND_CARVE_SEED_COUNT_MAX: 5 → 7
// 2d → 2e  SAND_CARVE_REGION_SIZE_MAX: 24 → 32
// SAND_CARVE_MAX_RATIO_PERCENT: 35 のまま（candidate が増えれば carve 量も連動して増加）
// SAND_CARVE_REGION_SIZE_MIN: 6 のまま
```

**変更前後対比（変更する定数のみ）**

| 定数 | 2d | 2e |
| --- | --- | --- |
| `SAND_CARVE_SEED_COUNT_MIN` | `2` | `3` |
| `SAND_CARVE_SEED_COUNT_MAX` | `5` | `7` |
| `SAND_CARVE_REGION_SIZE_MAX` | `24` | `32` |

**残す定数・削除する関数**

`build_sand_candidate_mask()` は削除するが、`EIGHT_DIRS` は 2e でも「river の 8 近傍 shoreline を距離 1 として扱う」ために引き続き使用する。`CARDINAL_DIRS_4` も `flood_fill_carve_region()` と growth 展開が引き続き使用するため残す。

### 4.1 river distance field（定義を実装と一致させる）

**許可セル（距離ラベルを付ける対象）**: マップ内かつ `!river_mask && !anchor_mask && !river_protection_band`。2d の候補生成と同じ禁止集合である。

**距離 1 の定義**: 2d と同じく、許可セルで **River の 8 近傍**に入るセルを距離 **1** とする。これにより diagonal shoreline 許容の外部契約を維持する。

**それ以降の展開**: 距離 **2 以上**は 4 近傍 BFS で外側へ伸ばす。つまり「距離 1 は 8 近傍 shoreline」「距離 2+ はそこからの 4 近傍拡張」という二段定義にする。

**距離の意味**: 許可セル `p` の距離 `dist(p)` は、`dist=1` の shoreline shell を始点として、許可セルのみを通過する 4 近傍経路で外側へ何段離れているかを表す。

**計算**: 多点始点 BFS。キュー初期化は「許可セルで、かつ 8 近傍のいずれかが `river_mask` である」セルをすべて距離 **1** として入れる（重複は除去）。以降、許可セルにのみ 4 近傍展開し `dist+1` を付与。`river_mask` / `anchor_mask` / `river_protection_band` 上は**通過しない**。`dist > SAND_SHORE_MAX_DISTANCE` への展開は打ち切る。

**注**: `river_mask` 内部には岸候補ラベルは不要（`Sand` にならない）。距離は「川の外側の許可セル」にだけ付く。

```rust
/// 4 近傍 BFS で許可セルの river 距離場を計算する。
///
/// 戻り値: `Vec<u32>` indexed by `y * MAP_WIDTH + x`。
/// - 許可セルかつ `dist <= SAND_SHORE_MAX_DISTANCE` のセル: 距離値 1..=SAND_SHORE_MAX_DISTANCE
/// - river/anchor/protection_band 上または未到達セル: `u32::MAX`
fn compute_river_distance_field(
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> Vec<u32> {
    let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
    let mut dist = vec![u32::MAX; size];
    let mut queue: VecDeque<GridPos> = VecDeque::new();

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            // river/anchor/protection_band セルは許可外
            if river_mask.get(p) || anchor_mask.get(p) || river_protection_band.get(p) {
                continue;
            }
            // 8 近傍のいずれかが river なら距離 1 で初期化（2d の diagonal 契約を維持）
            let adjacent_to_river = EIGHT_DIRS.iter().any(|&(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                (0..MAP_WIDTH).contains(&nx)
                    && (0..MAP_HEIGHT).contains(&ny)
                    && river_mask.get((nx, ny))
            });
            if adjacent_to_river {
                let idx = (y * MAP_WIDTH + x) as usize;
                dist[idx] = 1;
                queue.push_back(p);
            }
        }
    }

    while let Some(pos) = queue.pop_front() {
        let d = dist[(pos.1 * MAP_WIDTH + pos.0) as usize];
        // SAND_SHORE_MAX_DISTANCE に達したセルはここで打ち切り
        if d >= SAND_SHORE_MAX_DISTANCE {
            continue;
        }
        for &(dx, dy) in &CARDINAL_DIRS_4 {
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                continue;
            }
            let np = (nx, ny);
            // 許可セルのみ展開
            if river_mask.get(np) || anchor_mask.get(np) || river_protection_band.get(np) {
                continue;
            }
            let nidx = (ny * MAP_WIDTH + nx) as usize;
            if dist[nidx] == u32::MAX {
                dist[nidx] = d + 1;
                queue.push_back(np);
            }
        }
    }

    dist
}
```

### 4.2 base shoreline mask

距離場から **`SAND_BASE_DIST_MIN..=SAND_BASE_DIST_MAX`（初期案 1..=2）** を `base_candidate_mask` とする。`dist == 1` が 2d と同じ 8 近傍 shoreline を含み、`dist == 2` がその外側の薄い岸帯になる。

ここでは「必ず残したい砂浜の芯」を作る。現行 8 近傍リング相当の責務はこの層に吸収する。

```rust
fn build_base_shoreline_mask(dist_field: &[u32]) -> BitGrid {
    let mut base = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let d = dist_field[(y * MAP_WIDTH + x) as usize];
            if (SAND_BASE_DIST_MIN..=SAND_BASE_DIST_MAX).contains(&d) {
                base.set((x, y), true);
            }
        }
    }
    base
}
```

### 4.3 sand growth mask

**River 側 frontier（起点候補）**: `base_candidate_mask` が true のセルのうち、**距離場で `dist == 1`** のセルとする。ここには 2d と同じ diagonal shoreline も含まれる。

**起点選択**: frontier セル一覧から、seed 付き RNG で **3〜8 個**（`SAND_GROWTH_SEED_*`）を選ぶ。`choose_multiple` を使い、候補数で cap する（2d と同様）。

**起点が 0 個のとき**: `sand_growth_mask` は空のまま（growth ステップをスキップ）。`sand_candidate_mask = base | growth` は base のみになる。

**growth の幾何（初版の推奨）**: **同一距離場の `dist` を参照しつつ**、各起点から **別の bounded flood fill**（4 近傍・許可セルのみ・`river_mask` 非貫通）を行い `sand_growth_mask` に加算する。各 region は **面積上限**、**`dist` が `SAND_GROWTH_DIST_MAX` を超えたセルへは侵入しない**、**起点からの BFS 深さが `SAND_GROWTH_STEP_LIMIT` を超えたら止める**、の 3 条件で打ち切る。これにより「川から遠すぎる膨らみ」と「shoreline 帯を横に這うだけの growth」を両方抑制する。

※ 2e の growth は「2d の 8 近傍 shoreline shell から、4 近傍で局所的に外へ広げる」処理であり、diagonal shoreline の許容自体は削らない。

これで「川に沿っただけの帯」ではなく、ところどころ広がった砂浜ができる。

```rust
fn build_sand_growth_mask(
    dist_field: &[u32],
    base_candidate: &BitGrid,
    rng: &mut StdRng,
) -> BitGrid {
    // frontier: base cells that are dist==1 (directly adjacent to river)
    let frontier: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let idx = (y * MAP_WIDTH + x) as usize;
                (base_candidate.get((x, y)) && dist_field[idx] == 1).then_some((x, y))
            })
        })
        .collect();

    let mut growth = BitGrid::map_sized();
    if frontier.is_empty() {
        return growth;
    }

    let num_origins =
        rng.gen_range(SAND_GROWTH_SEED_COUNT_MIN..=SAND_GROWTH_SEED_COUNT_MAX);
    let k = (num_origins as usize).min(frontier.len());
    let origins: Vec<GridPos> = frontier.choose_multiple(rng, k).copied().collect();

    for origin in origins {
        flood_fill_growth_region(dist_field, &mut growth, origin, SAND_GROWTH_REGION_AREA_MAX);
    }

    growth
}

/// dist_field が SAND_GROWTH_DIST_MAX 以下の許可セルに対して、
/// 4 近傍 BFS で最大 `area_max` セル、かつ起点から最大 `SAND_GROWTH_STEP_LIMIT`
/// ステップまで growth する。
/// 戻り値: 実際に growth したセル数。
fn flood_fill_growth_region(
    dist_field: &[u32],
    growth: &mut BitGrid,
    origin: GridPos,
    area_max: usize,
) -> usize {
    let origin_idx = (origin.1 * MAP_WIDTH + origin.0) as usize;
    let origin_d = dist_field[origin_idx];
    // 起点が許可セル外または距離上限超なら即終了
    if origin_d == u32::MAX || origin_d > SAND_GROWTH_DIST_MAX || growth.get(origin) {
        return 0;
    }

    let mut queue: VecDeque<(GridPos, usize)> = VecDeque::new();
    queue.push_back((origin, 0));
    growth.set(origin, true);
    let mut count = 1usize;

    while let Some((pos, steps)) = queue.pop_front() {
        if count >= area_max {
            break;
        }
        if steps >= SAND_GROWTH_STEP_LIMIT {
            continue;
        }
        for &(dx, dy) in &CARDINAL_DIRS_4 {
            if count >= area_max {
                break;
            }
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                continue;
            }
            let np = (nx, ny);
            let nd = dist_field[(ny * MAP_WIDTH + nx) as usize];
            // 許可セル（u32::MAX 以外）かつ距離上限以内かつ未 growth
            if nd != u32::MAX && nd <= SAND_GROWTH_DIST_MAX && !growth.get(np) {
                growth.set(np, true);
                count += 1;
                queue.push_back((np, steps + 1));
            }
        }
    }
    count
}
```

### 4.4 non-sand carve の位置づけ

`non-sand carve` は **growth 後** に適用する。

順序は固定する。

1. `base_candidate_mask`
2. `sand_growth_mask`
3. `sand_candidate_mask = base_candidate_mask | sand_growth_mask`
4. `sand_carve_mask`
5. `final_sand_mask = sand_candidate_mask - sand_carve_mask`

`carve` を先に適用すると、growth が非砂領域を埋め戻して意味を壊すため採らない。

**`generate_sand_masks()` 更新後の全体フロー**

シグネチャは変わらない（外部 API 変更なし）:

```rust
pub fn generate_sand_masks(
    seed: u64,
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid)
```

内部実装のみ変わる:

```rust
pub fn generate_sand_masks(
    seed: u64,
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid) {
    let mut rng = StdRng::seed_from_u64(seed ^ SAND_SEED_SALT);

    // 1. 距離場
    let dist_field =
        compute_river_distance_field(river_mask, anchor_mask, river_protection_band);

    // 2. base shoreline (dist 1..=2)
    let base = build_base_shoreline_mask(&dist_field);

    // 3. additive growth
    let growth = build_sand_growth_mask(&dist_field, &base, &mut rng);

    // 4. candidate = base | growth
    let mut candidate = base;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if growth.get((x, y)) {
                candidate.set((x, y), true);
            }
        }
    }

    // 5. carve
    let mut carve = build_sand_carve_mask(&candidate, &mut rng);

    // 6. final = candidate & !carve
    let mut final_mask = candidate.clone();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if carve.get((x, y)) {
                final_mask.set((x, y), false);
            }
        }
    }

    // フォールバック: final が空なら carve を全捨てして candidate 全面を採用
    if final_mask.count_set() == 0 {
        carve = BitGrid::map_sized();
        final_mask = candidate.clone();
    }

    (candidate, carve, final_mask)
}
```

### 4.5 smoothing は後回し

形態演算的な smoothing / closing は今回の第一段では入れない。まずは **distance field + additive growth** で輪郭依存を下げる。

理由:

- 加算ステップだけで改善量が大きい
- smoothing は `non-sand carve` の抜けを潰しやすい
- 後段で必要なら `final_sand_mask` ではなく `sand_candidate_mask` 側に限定して追加できる

---

## 5. データ構造と API 変更方針

### 5.1 `WorldMasks`

公開フィールドは増やさない。既存の

- `sand_candidate_mask`
- `sand_carve_mask`
- `final_sand_mask`

をそのまま使う。

意味だけを更新する。

- `sand_candidate_mask`: 「8 近傍リング」ではなく「base shoreline + growth を合成した候補」

### 5.2 `river.rs`

主な変更対象。内部 helper を追加する。

| 変更種別 | 対象 |
| --- | --- |
| **削除** | `fn build_sand_candidate_mask(...)` |
| **定数値変更** | `SAND_CARVE_SEED_COUNT_MIN: 2→3`, `SAND_CARVE_SEED_COUNT_MAX: 5→7`, `SAND_CARVE_REGION_SIZE_MAX: 24→32` |
| **追加（定数）** | `SAND_SHORE_MAX_DISTANCE`, `SAND_BASE_DIST_MIN/MAX`, `SAND_GROWTH_SEED_COUNT_MIN/MAX`, `SAND_GROWTH_DIST_MAX`, `SAND_GROWTH_STEP_LIMIT`, `SAND_GROWTH_REGION_AREA_MAX` |
| **追加（private fn）** | `compute_river_distance_field`, `build_base_shoreline_mask`, `build_sand_growth_mask`, `flood_fill_growth_region` |
| **内部実装更新** | `generate_sand_masks()` のみ変更（シグネチャは据え置き） |

`CARDINAL_DIRS_4` は `flood_fill_carve_region` と growth 展開が引き続き使う。`EIGHT_DIRS` も shoreline shell の初期化に引き続き使う。

---

## 6. 期待されるパフォーマンス影響

### 6.1 実行コスト

- 距離場 BFS: `O(MAP_WIDTH * MAP_HEIGHT)`
- growth flood fill: 起点数と距離上限で bounded。マップ全域に対して軽い
- 100x100 マップでは、WFC 本体に比べて十分小さい

### 6.2 実装コスト

- 主変更は `river.rs` に閉じる
- `WorldMasks` / `mapgen.rs` / `wfc_adapter.rs` の公開契約は基本維持
- `validate.rs` は任意で debug warning を 1 つ足す程度で済む

### 6.3 リスク

- growth が強すぎると `Sand` 面積が増えすぎる
- carve 定数が現行のままだと、改善後の candidate 面積に対して抜き量が不足する可能性がある

したがって **growth 定数と carve 定数の再調整**は同じ MS で扱う。

---

## 7. 実装ステップ

### Step 1: 定数追加・変更（`river.rs`）

- `build_sand_candidate_mask()` を削除する
- 新定数（`SAND_SHORE_MAX_DISTANCE` 等）を追加する
- `SAND_GROWTH_STEP_LIMIT` を追加する
- carve 定数 3 個を更新する

### Step 2: 距離場・base shoreline・growth helper を追加（`river.rs`）

4 関数を追加する（§4.1〜4.3 のコードをそのまま使用）:
1. `compute_river_distance_field(river_mask, anchor_mask, river_protection_band) -> Vec<u32>`  
   `dist == 1` は 8 近傍 shoreline shell、`dist >= 2` は 4 近傍外側展開
2. `build_base_shoreline_mask(dist_field: &[u32]) -> BitGrid`
3. `build_sand_growth_mask(dist_field, base_candidate, rng) -> BitGrid`
4. `flood_fill_growth_region(dist_field, growth, origin, area_max) -> usize`

### Step 3: `generate_sand_masks()` の内部実装を差し替え（`river.rs`）

§4.4 のコードで関数本体を置き換える。シグネチャは変わらない。

### Step 4: `world_masks.rs` の doc comment 更新

`sand_candidate_mask` フィールドの `///` コメントを「8 近傍リング」→「base shoreline + growth を合成した候補」に変更。

### Step 5: テスト追加（`river.rs` の `mod tests`）

以下のテストを追加する:

```rust
#[test]
fn final_sand_has_cells_not_adjacent_to_river() {
    // 2e 以降: dist >= 2 のセルが存在する = 川に直接隣接しない Sand が生まれている
    let masks = {
        let anchor = AnchorLayout::fixed();
        let mut m = WorldMasks::from_anchor(&anchor);
        m.fill_river_from_seed(42);
        m.fill_sand_from_river_seed(42);
        m
    };
    let has_non_adjacent = (0..MAP_HEIGHT).any(|y| {
        (0..MAP_WIDTH).any(|x| {
            if !masks.final_sand_mask.get((x, y)) {
                return false;
            }
            !CARDINAL_DIRS_4.iter().any(|&(dx, dy)| masks.river_mask.get((x + dx, y + dy)))
        })
    });
    assert!(
        has_non_adjacent,
        "全 Sand が river 直接隣接: 2e は dist>=2 の Sand を生成するはず"
    );
}

#[test]
fn final_sand_count_stays_bounded_for_representative_seed() {
    let anchor = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchor);
    masks.fill_river_from_seed(42);
    masks.fill_sand_from_river_seed(42);

    let sand = masks.final_sand_mask.count_set();
    let river = masks.river_mask.count_set();
    assert!(
        sand <= river * 3,
        "Sand area exploded for seed=42: sand={sand}, river={river}"
    );
}
```

既存テスト `sand_mask_is_deterministic_for_same_seed` / `sand_mask_does_not_overlap_anchor_or_protection_band` / `final_sand_mask_is_non_empty` は変更不要（距離場への切り替えでも同じ性質が成立する）。

### Step 6: `cargo check / clippy / test`

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
```

### Step 7: docs を同期

- `docs/world_layout.md`: 砂浜生成説明を 8 近傍リングから distance-field + growth へ更新
- `crates/hw_world/README.md`: `river.rs` / `world_masks.rs` の責務説明更新
- `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md`: MS-WFC-2e 行をステータス更新
- `docs/plans/3d-rtt/milestone-roadmap.md`: MS-WFC-2e 行のステータス更新

---

## 8. 変更ファイル

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/river.rs` | `build_sand_candidate_mask` 削除、carve 定数 3 個更新、新定数 8 個追加、`compute_river_distance_field` / `build_base_shoreline_mask` / `build_sand_growth_mask` / `flood_fill_growth_region` 追加、`generate_sand_masks` 内部実装更新、テスト `final_sand_has_cells_not_adjacent_to_river` と `final_sand_count_stays_bounded_for_representative_seed` 追加 |
| `crates/hw_world/src/world_masks.rs` | `sand_candidate_mask` の doc comment を「growth 込み候補」へ更新 |
| `docs/world_layout.md` | 砂浜生成説明を 8 近傍リングから distance-field + growth へ更新 |
| `crates/hw_world/README.md` | `river.rs` / `world_masks.rs` の責務説明更新 |
| `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` | MS-WFC-2e の**実装状況・本文**を更新（既にサブ計画表に行がある場合は**追記ではなく更新**） |
| `docs/plans/3d-rtt/milestone-roadmap.md` | 同様に MS-WFC-2e 行の**ステータス更新**（未着手→進行/完了） |

`mapgen.rs` と `wfc_adapter.rs` と `validate.rs` は変更不要（`generate_sand_masks()` のシグネチャが変わらないため）。

---

## 9. 完了条件

- [ ] 同一 seed で `sand_candidate_mask` / `sand_carve_mask` / `final_sand_mask` が deterministic（既存テスト `sand_mask_is_deterministic_for_same_seed` が通る）
- [ ] `final_sand_mask` が `river_mask` / `anchor_mask` / `river_protection_band` と交差しない（既存テスト `sand_mask_does_not_overlap_anchor_or_protection_band` が通る）
- [ ] `final_sand_mask` 上が常に最終 `Sand` になる（既存テスト `test_sand_matches_final_sand_mask` が通る）
- [ ] 2d と同じ 8 近傍 shoreline 契約を壊さない（`dist == 1` shell が diagonal shoreline を保持する実装になっている）
- [ ] representative seed（42）で `river` から距離 2 以上の `Sand` が存在する（新テスト `final_sand_has_cells_not_adjacent_to_river` が通る）
- [ ] representative seed（42）で `final_sand_mask.count_set() <= river_mask.count_set() * 3` を満たし、面積が暴れすぎない（新テスト `final_sand_count_stays_bounded_for_representative_seed` が通る）
- [ ] `build_sand_candidate_mask` 関数が削除され、`cargo clippy` で dead code 警告が出ない
- [ ] carve 定数 3 個（`SAND_CARVE_SEED_COUNT_MIN/MAX`, `SAND_CARVE_REGION_SIZE_MAX`）が 2d から再調整済み
- [ ] `Sand` が map 全体へ拡散せず、`river` 由来の岸帯として保たれる（代表 seed で全面砂にならず抜けが残る）
- [ ] `cargo test -p hw_world` 全テスト通過
- [ ] `cargo check --workspace` エラーなし
- [ ] `cargo clippy --workspace` 警告なし

---

## 10. 検証

- `cargo test -p hw_world`
- `cargo check --workspace`
- `cargo clippy --workspace`

加えて、golden seed で以下を目視確認する。

- 直線に近い川
- 強く蛇行する川
- 保護帯ぎりぎりを通る川

確認観点:

- 砂浜が「輪郭の縁取り」ではなく面として見えるか
- 砂浜の膨らみが局所的に存在するか
- `non-sand carve` により単調な全面砂になっていないか

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-04` | `Codex` | 初版作成。砂浜の輪郭依存を下げるため、distance field + additive growth + 既存 carve 維持の方針を整理。 |
| `2026-04-05` | — | ブラッシュアップ: §4.0 に定数の型・値・変更前後対比表・削除対象を追記。§4.1〜4.3 に具体的な Rust 実装コードを追記。§4.4 に更新後の `generate_sand_masks()` 全体を追記。§5.2 の変更対象を表形式へ拡充。§7 を Step 形式に置き換えテストコード含む。§8 変更ファイル表に変更しないファイルの明示を追記。§9 完了条件に既存テスト名・削除確認・clippy 確認を追加。 |
| `2026-04-05` | `Codex` | 実装完了を反映。`river distance field + base shoreline + bounded growth` への差し替えと関連テスト追加を完了扱いに更新。補足として、`final_sand_has_cells_not_adjacent_to_river` は非カーディナル隣接を検査するもので、厳密な `dist>=2` 証明ではない。 |
