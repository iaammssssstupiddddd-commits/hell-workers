# MS-WFC-2.5: 地形バイアスゾーンマスク生成

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2-5-terrain-zone-mask` |
| ステータス | `実装完了・テスト不足` |
| 作成日 | `2026-04-04` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2e-sand-shore-shape.md`](wfc-ms2e-sand-shore-shape.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `fill_sand_from_river_seed()` と `final_sand_mask` 反映が実装済み（**MS-WFC-2d/2e 完了必須**） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| 解決したいこと | WFC の Grass/Dirt 分布が全面均一になりやすく、地形にメリハリがない。また砂浜以外の少量の砂地（内陸砂）を Grass エリアに点在させたい |
| 主変更 | アンカー距離場（D）を起点に flood fill（B）で `grass_zone_mask` / `dirt_zone_mask` を生成し、`post_process_tiles` で確率的バイアス（B）・ゾーン端部グラデーション（C）・完全中立リージョンバイアスを適用する。`inland_sand_mask` を `grass_zone_mask` 内に生成し、8 近傍 Grass チェック付きで post_process する |
| 維持するもの | `final_sand_mask` / `river_mask` の優先度、マスク由来の決定性、`deterministic` 契約（※下記 RNG 節）、WFC 本体の隣接制約 |
| 期待効果 | ゾーン内に Dirt/Grass のメリハリが出現。ゾーン端部は 3 マスのグラデーション、完全中立は 8×8 リージョン単位の偏りで変化が生まれる。Grass エリアに小さな砂地が点在 |
| 内陸砂の隣接制約 | `inland_sand_mask` セルの 8 近傍は必ず Grass（River・Dirt・既存 Sand と隣接しない） |
| MS-WFC-3 との接続 | `grass_zone_mask` / `dirt_zone_mask` を `generate_resource_layout()` に渡し、フォレスト・岩配置の優先エリアとして再利用する |
| やらないこと | WFC 制約への新 hard constraint 追加、Sand/River の変更、アンカー位置の変更 |

### 計画との主な差分（実装時に変更した点）

| 項目 | 計画時 | 実装後 |
| --- | --- | --- |
| Dirt 種点距離上限 | 12 | 16 |
| Grass 種点距離下限 | 20 | 18 |
| Dirt 1 パッチ面積上限 | 180 | 500 |
| Grass 1 パッチ面積上限 | 280 | 700 |
| ゾーン間離隔 | なし（重複禁止のみ） | 3 マス（`ZONE_MIN_SEPARATION`） |
| B 強制率 | 固定 85% | ランダム範囲 72〜98%（`ENFORCE_MIN/MAX`） |
| C グラデーション | アンカー距離閾値ベース | **ゾーン境界から 3 マス**（`ZONE_GRADIENT_WIDTH`）|
| 完全中立バイアス | なし | 8×8 リージョン単位 ±20%（`NEUTRAL_REGION_BIAS_PERCENT`） |
| WorldMasks 距離場 | `anchor_distance_field` | `dirt_zone_distance_field` / `grass_zone_distance_field` |

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
  - 隣接ルール: inland sand の 8 近傍は Grass のみ（River・Dirt・既存 Sand と隣接させない）
  - WFC テーブルの変更は不要（post_process の 8 近傍チェックで保証する）

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

**`compute_anchor_distance_field` の定義**（`compute_protection_band` とは別物。混同しないこと）:

| 項目 | `compute_protection_band`（既存・`world_masks.rs`） | `compute_anchor_distance_field`（本 MS 新規） |
| --- | --- | --- |
| 目的 | アンカー外周 `width` セル幅の禁止帯マスク | 各セルから**最寄りのアンカーセル**までの**最短ステップ数** |
| 起点 | アンカーに 4 隣接する**非アンカー**セルを距離 1 としてキュー投入 | すべての `anchor_mask == true` セルを距離 **0** としてキュー投入 |
| 展開 | 4 近傍 BFS、**非アンカー**セルのみ距離伝播 | 4 近傍 BFS、**全セル**に `dist = 前 + 1`（アンカーは 0 固定・上書きしない） |
| 距離の意味 | 帯の内側フラグ（閾値 `width`） | グリッド上の geodesic 距離（マップ内のみ、4 方向） |

ゾーン seed 選択に使う `dist` は **`compute_anchor_distance_field` の結果**とする（`ZONE_DIRT_*` / `ZONE_GRASS_*` はこの距離を参照）。

- **Dirt ゾーン起点**: `dist` が `ZONE_DIRT_DIST_MIN..=ZONE_DIRT_DIST_MAX` に収まる許可セル群から seed を選ぶ
  - 意味: 保護帯のすぐ外（文明に近い荒れた地）
- **Grass ゾーン起点**: `dist >= ZONE_GRASS_DIST_MIN` の許可セル群から seed を選ぶ
  - 意味: アンカーから遠い手つかずの野原

**許可セル**: `!anchor_mask && !river_mask && !river_protection_band && !final_sand_mask`

**実装ひな形**（`terrain_zones.rs` に配置。直交 4 近傍・`VecDeque` による BFS の**書き方**は `compute_protection_band` に近いが、起点と距離の意味は上表どおり別アルゴリズム）:

```rust
fn compute_anchor_distance_field(anchor_mask: &BitGrid) -> Vec<u32> {
    let w = MAP_WIDTH;
    let h = MAP_HEIGHT;
    let mut dist = vec![u32::MAX; (w * h) as usize];
    let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    // アンカーセルを距離 0 で全投入（マルチソース BFS）
    for y in 0..h {
        for x in 0..w {
            if anchor_mask.get((x, y)) {
                dist[(y * w + x) as usize] = 0;
                queue.push_back((x, y));
            }
        }
    }
    // BFS で全セルに距離を伝播（アンカー隣接 = dist 1、以降 +1）
    while let Some((cx, cy)) = queue.pop_front() {
        let d = dist[(cy * w + cx) as usize];
        for (dx, dy) in DIRS {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }
            let idx = (ny * w + nx) as usize;
            if dist[idx] == u32::MAX {          // 未訪問のみ（アンカーは 0 で設定済み→上書きされない）
                dist[idx] = d + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    dist
}
```

**`pick_zone_seeds` ひな形**:

```rust
/// `dist_max` に `u32::MAX` を渡すと上限なし（Grass ゾーン用）
fn pick_zone_seeds(
    rng: &mut StdRng,
    dist_field: &[u32],
    allowed_mask: &BitGrid,
    dist_min: u32,
    dist_max: u32,
    count_min: u32,
    count_max: u32,
) -> Vec<(i32, i32)> {
    let mut candidates: Vec<(i32, i32)> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let d = dist_field[(y * MAP_WIDTH + x) as usize];
            d >= dist_min && d <= dist_max && allowed_mask.get((x, y))
        })
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }
    let count = (rng.gen_range(count_min..=count_max) as usize).min(candidates.len());
    // partial Fisher-Yates: 先頭 count 個だけシャッフルして返す
    for i in 0..count {
        let j = rng.gen_range(i..candidates.len());
        candidates.swap(i, j);
    }
    candidates.truncate(count);
    candidates
}
```

### 3.2 B: Flood fill パッチ生成

各起点から 4 近傍 bounded flood fill でパッチを作り、`grass_zone_mask` / `dirt_zone_mask` に加算する。

- 許可セルのみ展開（上記と同じ制約セット）
- 各パッチは面積上限（`ZONE_*_REGION_AREA_MAX`）で打ち切る（**複数 seed のセル数を合算して数えない。各起点ごとに `count` を 1 から立ち上げ、`area_max` まで**）
- 2 ゾーンが重複した場合: Dirt 優先（`grass_zone_mask` から重複セルを除外）

**`flood_fill_zone_patches` ひな形**（`flood_fill_carve_region` / `flood_fill_growth_region` と同じ **bounded flood のパターン**。ゾーンは許可マスク＋面積上限が異なる）:

```rust
/// seeds から順に flood fill で BitGrid を生成する（結果は同一 `result` に累積）。
/// allowed_mask 外への展開は行わない。
/// area_max は「この起点から広がる 1 パッチあたり」の上限（seed 間で共有しない）。
fn flood_fill_zone_patches(
    seeds: &[(i32, i32)],
    allowed_mask: &BitGrid,
    area_max: usize,
) -> BitGrid {
    let mut result = BitGrid::map_sized();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    // 各 origin で count をリセットするので、複数 seed の合計面積は area_max × seeds.len() まで伸びうる
    for &origin in seeds {
        if !allowed_mask.get(origin) || result.get(origin) {
            continue;
        }
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        queue.push_back(origin);
        result.set(origin, true);
        let mut count = 1usize;

        'outer: while let Some(pos) = queue.pop_front() {
            for (dx, dy) in DIRS {
                if count >= area_max {
                    break 'outer;
                }
                let np = (pos.0 + dx, pos.1 + dy);
                if allowed_mask.get(np) && !result.get(np) {
                    result.set(np, true);
                    count += 1;
                    queue.push_back(np);
                }
            }
        }
    }
    result
}
```

### 3.3 内陸砂マスク（`inland_sand_mask`）

`grass_zone_mask` 確定後に、その内側から小パッチを生成する。

**近傍の使い分け**: アンカー距離場・ゾーン／内陸砂パッチの **flood fill は直交 4 近傍**（§3.1 / §3.2 と同じパターン）。内陸砂の **配置可否の隣接判定**（パッチ採用条件・`post_process_tiles` の Grass チェック）のみ **8 近傍**（斜め含む）とする。

**生成**:
- 候補: `grass_zone_mask && !anchor_mask && !river_mask && !river_protection_band && !final_sand_mask`
- seed 由来で `INLAND_SAND_PATCH_COUNT_MIN..=MAX` 個の起点を選択
- 各起点から bounded flood fill（面積 `INLAND_SAND_PATCH_AREA_MAX` 以下、許可セルのみ）
- 各パッチの全 8 近傍セルが `grass_zone_mask` に含まれる場合のみ採用（パッチ全体を棄却 or 採用）

**`generate_inland_sand_mask` ひな形**:

```rust
/// grass_zone_mask 内に小さな砂地パッチを生成する。
/// パッチ全体の 8 近傍が grass_zone 内に収まらない場合は採用しない。
fn generate_inland_sand_mask(
    rng: &mut StdRng,
    grass_zone_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_mask: &BitGrid,
    river_protection_band: &BitGrid,
    final_sand_mask: &BitGrid,
) -> BitGrid {
    // 候補セル: grass_zone かつ全禁止マスクを通過
    let mut candidate = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            if grass_zone_mask.get(p)
                && !anchor_mask.get(p)
                && !river_mask.get(p)
                && !river_protection_band.get(p)
                && !final_sand_mask.get(p)
            {
                candidate.set(p, true);
            }
        }
    }

    // 候補リストをシャッフルして起点を選択
    let mut cand_list: Vec<(i32, i32)> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
        .filter(|&p| candidate.get(p))
        .collect();
    if cand_list.is_empty() {
        return BitGrid::map_sized();
    }
    let patch_count =
        (rng.gen_range(INLAND_SAND_PATCH_COUNT_MIN..=INLAND_SAND_PATCH_COUNT_MAX) as usize)
            .min(cand_list.len());
    for i in 0..patch_count {
        let j = rng.gen_range(i..cand_list.len());
        cand_list.swap(i, j);
    }

    let mut result = BitGrid::map_sized();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    const OCTILE_DIRS: [(i32, i32); 8] = [
        (0, 1), (0, -1), (1, 0), (-1, 0),
        (1, 1), (1, -1), (-1, 1), (-1, -1),
    ];

    for &origin in &cand_list[..patch_count] {
        if !candidate.get(origin) || result.get(origin) {
            continue;
        }
        // 4 近傍 flood fill でパッチ収集
        let mut patch: Vec<(i32, i32)> = Vec::new();
        let mut visited = BitGrid::map_sized();
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        queue.push_back(origin);
        visited.set(origin, true);
        patch.push(origin);

        'fill: while let Some(pos) = queue.pop_front() {
            for (dx, dy) in DIRS {
                if patch.len() >= INLAND_SAND_PATCH_AREA_MAX {
                    break 'fill;
                }
                let np = (pos.0 + dx, pos.1 + dy);
                if candidate.get(np) && !visited.get(np) && !result.get(np) {
                    visited.set(np, true);
                    patch.push(np);
                    queue.push_back(np);
                }
            }
        }

        // パッチ全体の 8 近傍が grass_zone_mask に収まるか検証
        // 境界外セルは Grass とみなさない（マップ端のパッチを棄却するための安全側）
        let all_neighbors_in_grass = patch.iter().all(|&(px, py)| {
            OCTILE_DIRS.iter().all(|&(dx, dy)| {
                let np = (px + dx, py + dy);
                if np.0 < 0 || np.0 >= MAP_WIDTH || np.1 < 0 || np.1 >= MAP_HEIGHT {
                    return false;
                }
                grass_zone_mask.get(np)
            })
        });

        if all_neighbors_in_grass {
            for p in patch {
                result.set(p, true);
            }
        }
    }
    result
}
```

**post_process_tiles での配置（§3.4 より後）**:
- `inland_sand_mask` セルで `tile != River && tile != Sand` の場合、8 近傍の `tiles` をすべてチェック
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
   ↑ 上記 3 ステップは既存ループで一括処理（§5.3 参照）
4. zone bias（grass/dirt zone mask）   ← 既存ループの後に別パスで追加
5. inland_sand_mask → 8 近傍 Grass チェック付き Sand 変換  ← さらに別パスで追加
```

inland sand を最後にすることで、zone bias 確定後の tile 状態に対して近傍チェックをかけられる。

### 3.5 決定性と RNG の役割分担

| データ | 使用する乱数源 | 同一 `master_seed` で WFC を複数 attempt したとき |
| --- | --- | --- |
| `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` | `fill_terrain_zones_from_seed(master_seed)` から導出する RNG（`master_seed` 系） | **不変**（`masks` は attempt 前に確定） |
| ゾーンバイアス（Grass/Dirt の確率フリップ） | `run_wfc` 内の `post_process_tiles` が受け取る `rng`（= `StdRng::seed_from_u64(sub_seed)`、`sub_seed = derive_sub_seed(master_seed, attempt)`） | **attempt ごとに変わりうる** |
| stray Sand の Grass/Dirt 置換（既存 Step 3） | 同上 `rng` | 同上 |
| 最終地形タイル全体 | 上記の合成 | **完全に同一 seed の再現**は「同じ attempt が選ばれた場合」に限る（retry で別 attempt が採用されればタイルは変わりうる） |

ドキュメント・デバッグ時は、「マスクは master 決定」「タイル上の確率処理は sub_seed 依存」と切り分けて説明する。

### 3.6 `fallback_terrain` との整合

WFC 全試行失敗時のみ `fallback_terrain(&masks)` が使われる（`mapgen.rs`）。現状は River + `final_sand_mask` 上の Sand + 他 Grass のみで、**ゾーンバイアスも内陸砂も載らない**。

本 MS 実装時は次のいずれかを満たすこと（推奨は A）:

- **A（推奨）**: `post_process_tiles` の Step 4–5（ゾーン確率フリップ・内陸砂）を **共通ヘルパ `apply_zone_post_process`** に切り出し、`fallback_terrain` でも同ヘルパを呼ぶ。`fallback_terrain` には `master_seed: u64` 引数を追加し、内部で `fallback_post_seed(master_seed)` のみに依存した RNG を生成（attempt 非依存）。

  ```rust
  // wfc_adapter.rs に追加するヘルパ
  fn fallback_post_seed(master_seed: u64) -> u64 {
      master_seed ^ 0xfb7c_3a91_d5e2_4608  // 任意の定数で master_seed をミックス
  }

  /// Step 4（zone bias）と Step 5（inland sand）を共通化したヘルパ。
  /// post_process_tiles と fallback_terrain 両方から呼ぶ。
  fn apply_zone_post_process(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
      // Step 4: zone bias（確率的フリップ）
      for y in 0..MAP_HEIGHT {
          for x in 0..MAP_WIDTH {
              let idx = (y * MAP_WIDTH + x) as usize;
              // River / Sand は変更しない
              if masks.river_mask.get((x, y))
                  || tiles[idx] == TerrainType::River
                  || tiles[idx] == TerrainType::Sand
              {
                  continue;
              }
              if masks.grass_zone_mask.get((x, y)) && tiles[idx] == TerrainType::Dirt {
                  if rng.gen_range(0..100) < ZONE_GRASS_ENFORCE_PERCENT {
                      tiles[idx] = TerrainType::Grass;
                  }
              } else if masks.dirt_zone_mask.get((x, y)) && tiles[idx] == TerrainType::Grass {
                  if rng.gen_range(0..100) < ZONE_DIRT_ENFORCE_PERCENT {
                      tiles[idx] = TerrainType::Dirt;
                  }
              }
          }
      }
      // Step 5: inland sand（zone bias 後の状態を参照）
      const OCTILE_DIRS: [(i32, i32); 8] = [
          (0, 1), (0, -1), (1, 0), (-1, 0),
          (1, 1), (1, -1), (-1, 1), (-1, -1),
      ];
      for y in 0..MAP_HEIGHT {
          for x in 0..MAP_WIDTH {
              let idx = (y * MAP_WIDTH + x) as usize;
              if !masks.inland_sand_mask.get((x, y)) { continue; }
              if tiles[idx] == TerrainType::River || tiles[idx] == TerrainType::Sand { continue; }
              let all_grass = OCTILE_DIRS.iter().all(|&(dx, dy)| {
                  let nx = x + dx; let ny = y + dy;
                  if nx < 0 || nx >= MAP_WIDTH || ny < 0 || ny >= MAP_HEIGHT { return false; }
                  tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::Grass
              });
              if all_grass { tiles[idx] = TerrainType::Sand; }
          }
      }
  }

  // fallback_terrain のシグネチャ変更（master_seed 引数追加）
  pub(crate) fn fallback_terrain(masks: &WorldMasks, master_seed: u64) -> Vec<TerrainType> {
      let mut tiles = vec![TerrainType::Grass; (MAP_WIDTH * MAP_HEIGHT) as usize];
      for y in 0..MAP_HEIGHT {
          for x in 0..MAP_WIDTH {
              let idx = (y * MAP_WIDTH + x) as usize;
              if masks.river_mask.get((x, y)) {
                  tiles[idx] = TerrainType::River;
              } else if masks.final_sand_mask.get((x, y)) {
                  tiles[idx] = TerrainType::Sand;
              }
          }
      }
      // attempt 非依存の専用 seed で zone / inland_sand を適用
      let mut rng = StdRng::seed_from_u64(fallback_post_seed(master_seed));
      apply_zone_post_process(&mut tiles, masks, &mut rng);
      tiles
  }
  ```

  `mapgen.rs` 側の呼び出しを `fallback_terrain(&masks, master_seed)` に更新すること（§5.4 参照）。

- **B**: フォールバックではゾーン／内陸砂を**適用しない**ことを `used_fallback == true` の仕様として明文化する（実装コストは低いが、レアケースでも草原に小砂地が出ない）。

`lightweight_validate` はリソース候補の存在などを見るため、フォールバック時の挙動差は **MS-WFC-3 前提ドキュメント**に一言足す。

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

`from_anchor()` の初期化ブロックにも追加（空 `BitGrid::map_sized()` で初期化し、後続メソッドで設定）:

```rust
WorldMasks {
    // ... 既存フィールド ...
    final_sand_mask: BitGrid::map_sized(),
    // ↓ 追加
    grass_zone_mask: BitGrid::map_sized(),
    dirt_zone_mask: BitGrid::map_sized(),
    inland_sand_mask: BitGrid::map_sized(),
}
```

新メソッド追加:

```rust
/// `fill_sand_from_river_seed()` 完了後に呼ぶ。
/// anchor 距離場と final_sand_mask を参照し、terrain zone masks と inland_sand_mask を生成する。
pub fn fill_terrain_zones_from_seed(&mut self, seed: u64) {
    // `final_sand_mask` が空の seed は river/sand パイプライン上は稀（`river.rs` は空なら候補全面を final にする等で通常は非空）。
    // 空でもゾーン生成自体は可能だが、allowed が砂浜を除かないなど挙動が変わるため、本プロジェクトでは debug のみで呼び出し順を保証する。
    debug_assert!(
        self.final_sand_mask.count_set() > 0,
        "fill_terrain_zones_from_seed は fill_sand_from_river_seed の後に呼ぶこと（final_sand_mask 非空を期待）"
    );
    let (grass, dirt, inland_sand) = crate::terrain_zones::generate_terrain_zone_masks(
        seed,
        &self.anchor_mask,
        &self.river_mask,
        &self.river_protection_band,
        &self.final_sand_mask,
    );
    self.grass_zone_mask = grass;
    self.dirt_zone_mask = dirt;
    self.inland_sand_mask = inland_sand;
}
```

`fill_terrain_zones_from_seed` の直前に **`fill_sand_from_river_seed` を必ず呼ぶ**こと（`mapgen::generate_world_layout` の順序を崩さない）。`final_sand_mask` が空になりうる異常系をテストする場合は、`debug_assert` を外すか `#[cfg(test)]` 専用パスで別処理する。

呼び出し順序:
```
from_anchor()
  → fill_river_from_seed()
    → fill_sand_from_river_seed()
      → fill_terrain_zones_from_seed()   ← new（zone mask + inland_sand_mask を両方生成）
```

### 5.2 `terrain_zones.rs`（新規）

**公開 API**（`inland_sand_mask` も含めて 3-tuple で返す）:

```rust
/// grass_zone_mask / dirt_zone_mask / inland_sand_mask を一括生成して返す。
/// 戻り値: (grass_zone_mask, dirt_zone_mask, inland_sand_mask)
pub fn generate_terrain_zone_masks(
    seed: u64,
    anchor_mask: &BitGrid,
    river_mask: &BitGrid,
    river_protection_band: &BitGrid,
    final_sand_mask: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid)
```

**必要な use 宣言**（ファイル冒頭）:

```rust
use std::collections::VecDeque;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use crate::world_masks::BitGrid;
```

内部構成と処理フロー:

```rust
pub fn generate_terrain_zone_masks(...) -> (BitGrid, BitGrid, BitGrid) {
    let mut rng = StdRng::seed_from_u64(seed);

    // 許可セルマスク（ゾーン展開を禁止する領域を除いた残り）
    let mut allowed = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT { for x in 0..MAP_WIDTH {
        let p = (x, y);
        if !anchor_mask.get(p) && !river_mask.get(p)
            && !river_protection_band.get(p) && !final_sand_mask.get(p) {
            allowed.set(p, true);
        }
    }}

    // D: 距離場（全セルへの最短アンカー距離）
    let dist_field = compute_anchor_distance_field(anchor_mask);

    // B1: Dirt ゾーン
    let dirt_seeds = pick_zone_seeds(
        &mut rng, &dist_field, &allowed,
        ZONE_DIRT_DIST_MIN, ZONE_DIRT_DIST_MAX,
        ZONE_DIRT_SEED_COUNT_MIN, ZONE_DIRT_SEED_COUNT_MAX,
    );
    let dirt_zone_mask = flood_fill_zone_patches(&dirt_seeds, &allowed, ZONE_DIRT_REGION_AREA_MAX);

    // B2: Grass ゾーン（Dirt と重複しないよう allowed を絞る）
    let allowed_for_grass = {
        let mut a = allowed.clone();
        for y in 0..MAP_HEIGHT { for x in 0..MAP_WIDTH {
            if dirt_zone_mask.get((x, y)) { a.set((x, y), false); }
        }}
        a
    };
    let grass_seeds = pick_zone_seeds(
        &mut rng, &dist_field, &allowed_for_grass,
        ZONE_GRASS_DIST_MIN, u32::MAX,   // Grass は上限なし
        ZONE_GRASS_SEED_COUNT_MIN, ZONE_GRASS_SEED_COUNT_MAX,
    );
    let grass_zone_mask = flood_fill_zone_patches(&grass_seeds, &allowed_for_grass, ZONE_GRASS_REGION_AREA_MAX);

    // debug: Grass ∩ Dirt = 空（allowed_for_grass から Dirt 除外済みなので自明）
    debug_assert!(
        !(0..MAP_HEIGHT).flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .any(|p| grass_zone_mask.get(p) && dirt_zone_mask.get(p)),
        "grass_zone と dirt_zone が重複しています"
    );

    // 内陸砂マスク
    let inland_sand_mask = generate_inland_sand_mask(
        &mut rng, &grass_zone_mask, anchor_mask,
        river_mask, river_protection_band, final_sand_mask,
    );

    (grass_zone_mask, dirt_zone_mask, inland_sand_mask)
}
```

### 5.3 `wfc_adapter.rs`（`post_process_tiles` 拡張）

既存の `post_process_tiles`（Step 1–3 を 1 パスで処理）**の直後**に Step 4/5 を別パスとして追加する。
既存ループ内への組み込みは行わない（Step 4 が Step 3 の `else if` チェーンに干渉するため）。

```rust
fn post_process_tiles(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
    // ── 既存: Step 1–3（変更しない） ─────────────────────────────────────────
    let total = WEIGHT_GRASS + WEIGHT_DIRT;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                continue;
            }
            if masks.final_sand_mask.get((x, y)) {
                tiles[idx] = TerrainType::Sand;
            } else if tiles[idx] == TerrainType::Sand {
                let r = rng.gen_range(0..total);
                tiles[idx] = if r < WEIGHT_GRASS { TerrainType::Grass } else { TerrainType::Dirt };
            }
        }
    }
    // ── 追加: Step 4/5 を共通ヘルパに委譲（§3.6 Option A） ─────────────────
    apply_zone_post_process(tiles, masks, rng);
}
```

`apply_zone_post_process` の実装は §3.6 の `fallback_terrain` 節に記載。
`run_wfc` からは `post_process_tiles` を呼ぶだけでよい（既存と同じ）。

### 5.4 `mapgen.rs`

`generate_world_layout()` 内で `fill_sand_from_river_seed()` の後に呼び出し追加:

```rust
masks.fill_sand_from_river_seed(master_seed);
masks.fill_terrain_zones_from_seed(master_seed);   // ← 追加（この位置に挿入）
```

また `fallback_terrain` のシグネチャが変わるため（§3.6 Option A）、呼び出し箇所を更新:

```rust
// 変更前
terrain_tiles: fallback_terrain(&masks),
// 変更後
terrain_tiles: fallback_terrain(&masks, master_seed),
```

### 5.5 `mapgen/validate.rs`（debug 診断の整合）

現状 `check_no_stray_sand_outside_mask`（`debug_validate`）は「`final_sand_mask == false` のセルに `Sand` があれば警告」としている。内陸砂を導入すると **`TerrainType::Sand` は `inland_sand_mask` 上にも合法**になるため、次のように拡張する:

- **許容条件**: `Sand` タイルは `final_sand_mask || inland_sand_mask` のいずれかが true のセルにのみ存在してよい（両方 false で `Sand` → stray 警告）。
- **補助チェック**（任意・debug）: `inland_sand_mask` が true なのに `Sand` でない（post で隣接不足により変換されなかった）セルは情報警告としてよい。

**修正後のコード**（既存の `check_no_stray_sand_outside_mask` を置き換え）:

```rust
#[cfg(any(test, debug_assertions))]
fn check_no_stray_sand_outside_mask(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            // final_sand_mask と inland_sand_mask の合法領域を合わせて判定
            let in_legal_sand = layout.masks.final_sand_mask.get(pos)
                || layout.masks.inland_sand_mask.get(pos);
            if !in_legal_sand {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] == TerrainType::Sand {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::SandMaskMismatch,
                        message: format!("Stray Sand outside sand masks at ({x},{y})"),
                    });
                }
            }
        }
    }
}
```

`check_final_sand_mask_applied`（`final_sand_mask` 上は必ず Sand）は変更しない。`inland_sand_mask` は `final_sand_mask` と排他になるよう生成するため、マスク同士の重複チェックを debug に追加してもよい。

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
- `flood_fill_zone_patches(...)` 実装（Sand の carve/grow と同様の bounded flood）
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
- inland sand の 8 近傍チェック付き Sand 変換を別パスで追加（Step 5）
- fallback_terrain にも zone / inland_sand 適用

### Step 5: テストを追加

テストは `terrain_zones.rs` 末尾の `#[cfg(test)] mod tests` に配置する。
`WorldMasks::from_anchor` + `fill_*` の実際の呼び出しチェーンを使ってエンドツーエンドで検証する。

**代表 seed に依存するテスト**（Dirt が近傍に必ず出る等）は **単一の `seed = 0` 固定を避ける**。実装後に一度 `0..N` やゴールデン候補リストを走査し、条件を満たす seed を定数化するか、次のように **複数候補のいずれかで成功**とする。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;

    /// テスト用ヘルパ: 固定 seed で完全な WorldMasks を生成
    fn make_masks(seed: u64) -> WorldMasks {
        let anchors = AnchorLayout::fixed();
        let mut masks = WorldMasks::from_anchor(&anchors);
        masks.fill_river_from_seed(seed);
        masks.fill_sand_from_river_seed(seed);
        masks.fill_terrain_zones_from_seed(seed);
        masks
    }

    #[test]
    fn test_zone_masks_deterministic() {
        // 同一 seed で 2 回生成し、カウントが一致することで決定性を確認
        let m1 = make_masks(12345);
        let m2 = make_masks(12345);
        assert_eq!(m1.grass_zone_mask.count_set(), m2.grass_zone_mask.count_set());
        assert_eq!(m1.dirt_zone_mask.count_set(), m2.dirt_zone_mask.count_set());
        assert_eq!(m1.inland_sand_mask.count_set(), m2.inland_sand_mask.count_set());
    }

    #[test]
    fn test_zone_masks_no_overlap() {
        let masks = make_masks(42);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let p = (x, y);
                assert!(
                    !(masks.grass_zone_mask.get(p) && masks.dirt_zone_mask.get(p)),
                    "grass_zone と dirt_zone が ({x},{y}) で重複"
                );
            }
        }
    }

    #[test]
    fn test_zone_masks_no_intersection_with_blocked_cells() {
        let masks = make_masks(99);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let p = (x, y);
                let blocked = masks.anchor_mask.get(p)
                    || masks.river_mask.get(p)
                    || masks.river_protection_band.get(p)
                    || masks.final_sand_mask.get(p);
                if blocked {
                    assert!(!masks.grass_zone_mask.get(p), "grass_zone が禁止セル ({x},{y}) と交差");
                    assert!(!masks.dirt_zone_mask.get(p), "dirt_zone が禁止セル ({x},{y}) と交差");
                }
            }
        }
    }

    #[test]
    fn test_inland_sand_mask_no_intersection_with_river_anchor_sand() {
        let masks = make_masks(7);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let p = (x, y);
                if masks.inland_sand_mask.get(p) {
                    assert!(!masks.final_sand_mask.get(p), "inland_sand が final_sand と交差 ({x},{y})");
                    assert!(!masks.river_mask.get(p), "inland_sand が river と交差 ({x},{y})");
                    assert!(!masks.anchor_mask.get(p), "inland_sand が anchor と交差 ({x},{y})");
                }
            }
        }
    }

    /// アンカー距離 5..=12 に Dirt ゾーンが少なくとも 1 セル存在するか（候補 seed のいずれかで成立すれば OK）
    #[test]
    fn test_dirt_zone_exists_near_anchor() {
        let candidates: [u64; 4] = [42, 12345, 99, 0];
        let anchors = AnchorLayout::fixed();
        let dirt_near_anchor = candidates.iter().copied().any(|seed| {
            let mut masks = WorldMasks::from_anchor(&anchors);
            masks.fill_river_from_seed(seed);
            masks.fill_sand_from_river_seed(seed);
            masks.fill_terrain_zones_from_seed(seed);
            let dist_field = compute_anchor_distance_field(&masks.anchor_mask);
            (0..MAP_HEIGHT)
                .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
                .any(|p| {
                    let d = dist_field[(p.1 * MAP_WIDTH + p.0) as usize];
                    masks.dirt_zone_mask.get(p)
                        && d >= ZONE_DIRT_DIST_MIN
                        && d <= ZONE_DIRT_DIST_MAX
                })
        });
        assert!(
            dirt_near_anchor,
            "いずれの候補 seed でも Dirt ゾーンがアンカー近傍（dist {}..={}）に現れなかった。候補リストを `make_masks` で走査して更新すること",
            ZONE_DIRT_DIST_MIN,
            ZONE_DIRT_DIST_MAX
        );
    }
}
```

### Step 6: docs 同期

- `world_layout.md` に地形ゾーン生成の説明追加（§3.5 の RNG 分担・§3.6 のフォールバック仕様を必要なら記載）
- 親計画 / ロードマップの MS-WFC-2.5 行を更新

### Step 7: `validate.rs`（debug）

- §5.5 に従い `check_no_stray_sand_outside_mask` を `inland_sand_mask` 対応に更新

---

## 8. 変更ファイル

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/terrain_zones.rs` | 新規。距離場・seed 選択・flood fill・zone mask 生成・inland sand パッチ生成 |
| `crates/hw_world/src/lib.rs` | `pub mod terrain_zones;` 追加 |
| `crates/hw_world/src/world_masks.rs` | `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` フィールド追加（`from_anchor` の初期化ブロックも含む）、`fill_terrain_zones_from_seed` 追加 |
| `crates/hw_world/src/mapgen.rs` | `fill_terrain_zones_from_seed` 呼び出し追加、`fallback_terrain(&masks, master_seed)` に引数追加 |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | `post_process_tiles` に `apply_zone_post_process` 呼び出し追加（Step 4/5）、`apply_zone_post_process` / `fallback_post_seed` 追加、`fallback_terrain` シグネチャ変更（`master_seed: u64` 引数追加） |
| `crates/hw_world/src/mapgen/validate.rs` | `check_no_stray_sand_outside_mask` を `inland_sand_mask` 許容に拡張（§5.5） |
| `docs/world_layout.md` | 地形ゾーン生成説明追加 |
| `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` | MS-WFC-2.5 行を追加・ステータス更新 |
| `docs/plans/3d-rtt/milestone-roadmap.md` | MS-WFC-2.5 行を追加 |

---

## 9. 完了条件

- [x] 同一 seed で `grass_zone_mask` / `dirt_zone_mask` が deterministic（`test_zone_masks_deterministic`）
- [x] `grass_zone_mask & dirt_zone_mask` が空（重複なし）（`test_zone_masks_no_overlap`）
- [x] どちらのゾーンも `river_mask` / `anchor_mask` / `river_protection_band` / `final_sand_mask` と交差しない（`test_zone_masks_no_intersection_with_blocked_cells`）
- [x] 代表 seed で anchor 近傍（dist ≤ 16）に Dirt ゾーンセルが存在する（`test_dirt_zone_exists_near_anchor`）※計画時は dist ≤ 12 だったが実装で 16 に変更済み
- [ ] 代表 seed で anchor 遠端（dist ≥ 18）に Grass ゾーンセルが存在する ※計画時は dist ≥ 20 だったが実装で 18 に変更済み。**テスト未実装**
- [ ] マップ全体が単一ゾーンに占拠されない（両ゾーンの合計が許可セルの **約 60% 以下**を目安。定数・マップサイズ変更で超えうるため、閾値はチューニング可能なパラメータとして扱い、固定値テストは緩めまたは比率のスモークに留める）**テスト未実装**
- [x] `inland_sand_mask` が `final_sand_mask` / `river_mask` / `anchor_mask` と交差しない（`test_inland_sand_mask_no_intersection_with_river_anchor_sand`）
- [ ] post_process 後、`inland_sand_mask` 上の Sand セルの 8 近傍が全て Grass（代表 seed で確認）**テスト未実装**
- [x] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

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
| `2026-04-04` | `Codex` | 内陸砂（inland_sand_mask）追加。grass_zone 内の小パッチ生成と **8 近傍** Grass 隣接ルールを §3.3 / §4 / §5 / §7〜10 に反映。疑似コードは境界外をチェック失敗に。flood fill は直交 4 近傍のまま（§3.3 注記）。 |
| `2026-04-04` | — | レビュー懸念の反映: §3.1 で `compute_anchor_distance_field` と `compute_protection_band` を表で区別。§3.5 RNG 分担、§3.6 fallback、§5.5 validate stray Sand、§7/§9 の決定性・60% 目安を追記。 |
| `2026-04-04` | `Copilot` | ブラッシュアップ: 既存コードパターン（`compute_protection_band`, `flood_fill_carve_region`, `post_process_tiles`, `fallback_terrain` の実装）を参照し各セクションを具体化。`compute_anchor_distance_field` / `pick_zone_seeds` / `flood_fill_zone_patches` / `generate_inland_sand_mask` の完全実装ひな形を §3.1〜3.3 に追加。`generate_terrain_zone_masks` の戻り値を `(BitGrid, BitGrid)` → `(BitGrid, BitGrid, BitGrid)` に修正（§5.2）。`from_anchor` 拡張コードを §5.1 に明記。`post_process_tiles` への追加を共通ヘルパ方式（`apply_zone_post_process`）に整理し §3.6 / §5.3 に実装ひな形を記載。`fallback_terrain` シグネチャ変更（`master_seed: u64` 引数追加）を §5.4 に明記。`check_no_stray_sand_outside_mask` 修正コードを §5.5 に追加。テストひな形コードを §7 Step 5 に追加。 |
| `2026-04-04` | — | レビュー後修正: §3.2 で `area_max` を「起点ごと」と明記し、複数 seed 時の累積面積の注意を追加。§5.1 で `final_sand_mask` 空の稀ケースと `debug_assert` の意図を注記。§7 で `test_dirt_zone_exists_near_anchor` を複数候補 seed の any に変更し、失敗時メッセージで候補更新を指示。 |
