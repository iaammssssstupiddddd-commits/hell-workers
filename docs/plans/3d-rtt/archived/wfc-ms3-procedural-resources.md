# MS-WFC-3: 木・岩の procedural 配置

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms3-procedural-resources` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-06` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2-5-terrain-zone-mask.md`](wfc-ms2-5-terrain-zone-mask.md) |
| 次MS | [`wfc-ms4-startup-integration.md`](wfc-ms4-startup-integration.md) |
| 前提 | `MS-WFC-2.5` 完了。`WorldMasks` に `grass_zone_mask` / `dirt_zone_mask` / `inland_sand_mask` が入り、`GeneratedWorldLayout` / `WfcForestZone` は定義済み。`lightweight_validate()` 成功時は `resource_spawn_candidates` の `water_tiles` / `sand_tiles` は既に埋まるが、`initial_tree_positions` / `forest_regrowth_zones` / `initial_rock_positions` および `rock_candidates` は未だ空。`WorldMasks` には `rock_protection_band`（width=2）と `tree_dense_protection_band`（width=2）も既に実装済み |

---

## この文書の読み方

- **§4.1** … パイプラインと `generate_world_layout` 統合の全体像（必読）
- **§4.2〜4.3** … 定数・配置アルゴリズム・exclusion（`combined_protection_band` は使わない）
- **§4.5** … `validate_post_resource` の契約と実装スケッチ
- **§5** … 実装の作業順
- **§7〜8** … 受け入れ条件とテスト例

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
| `crates/hw_world/src/mapgen.rs` | 上記マスク生成 → WFC → `lightweight_validate()`（水・砂候補まで）まで実装済み。木・森・岩の 3 フィールドと `rock_candidates` は未埋め | resource generation と post-resource 検証を **同一 attempt 内**で組み込み、通ったものだけ採用 |
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

**パイプライン（`find_map` 内の 1 attempt）**:

```
1. masks を生成（attempt 開始前に共通化済み）
2. run_wfc → terrain_tiles を得る
3. lightweight_validate() → ResourceSpawnCandidates (water_tiles / sand_tiles) を得る
4. generate_resource_layout() → ResourceLayout (trees / zones / rocks) を得る
5. validate_post_resource() → 木・岩を障害物として導線再確認
6. 3〜5 すべて Ok → Some(GeneratedWorldLayout) で採用。いずれか失敗 → None で次 sub-seed へ
```

**resource generation 本体は `mapgen/resources.rs` に切り出す**（`mapgen.rs` に直書きしない）。関数シグネチャ:

```rust
// crates/hw_world/src/mapgen/resources.rs

/// 木・岩・森林ゾーンを純粋関数として生成する。
/// `layout` は lightweight_validate 通過済みを前提とし、
/// water_tiles / sand_tiles が埋まっていることを仮定する。
/// 配置可能な候補セルが不足した場合は `None` を返し、attempt ごと捨てる。
pub fn generate_resource_layout(
    layout: &GeneratedWorldLayout,
    seed: u64,
) -> Option<ResourceLayout>;

/// generate_resource_layout の出力型。bevy_app へは公開せず、
/// `mapgen.rs` と `validate.rs` の間で共有する `pub(crate)` 型とする。
/// `GeneratedWorldLayout` へのフラット展開は mapgen.rs の責務。
pub(crate) struct ResourceLayout {
    pub initial_tree_positions: Vec<GridPos>,
    pub forest_regrowth_zones: Vec<WfcForestZone>,
    pub initial_rock_positions: Vec<GridPos>,
    /// bevy_app が岩採掘対象を参照するときの候補 (= initial_rock_positions と同一でよい)
    pub rock_candidates: Vec<GridPos>,
}
```

**`generate_world_layout` への統合コード**（`mapgen.rs`）:

```rust
let layout = (0..=MAX_WFC_RETRIES).find_map(|attempt| {
    let sub_seed = derive_sub_seed(master_seed, attempt);
    let terrain_tiles = run_wfc(&masks, sub_seed, attempt).ok()?;

    // ─ Step 3: 地形フェーズ検証 ─
    let candidate = GeneratedWorldLayout {
        terrain_tiles,
        anchors: anchors.clone(),
        masks: masks.clone(),
        resource_spawn_candidates: ResourceSpawnCandidates::default(),
        initial_tree_positions: Vec::new(),
        forest_regrowth_zones: Vec::new(),
        initial_rock_positions: Vec::new(),
        master_seed,
        generation_attempt: attempt,
        used_fallback: false,
    };
    let validated_candidates = validate::lightweight_validate(&candidate).ok()?;
    let candidate = GeneratedWorldLayout {
        resource_spawn_candidates: validated_candidates,
        ..candidate
    };

    // ─ Step 4: 資源配置 ─
    let res = resources::generate_resource_layout(&candidate, sub_seed)?;

    // ─ Step 5: 資源配置後の導線再確認 ─
    validate::validate_post_resource(&candidate, &res).ok()?;

    // ─ 採用 ─
    let merged_candidates = ResourceSpawnCandidates {
        water_tiles: candidate.resource_spawn_candidates.water_tiles.clone(),
        sand_tiles: candidate.resource_spawn_candidates.sand_tiles.clone(),
        rock_candidates: res.rock_candidates,
    };

    Some(GeneratedWorldLayout {
        initial_tree_positions: res.initial_tree_positions,
        forest_regrowth_zones: res.forest_regrowth_zones,
        initial_rock_positions: res.initial_rock_positions,
        resource_spawn_candidates: merged_candidates,
        ..candidate
    })
}).unwrap_or_else(|| {
    // terrain fallback の後も deterministic に resource fallback を組み立てる。
    // 木・森・岩を空配列のまま返すのは MS-WFC-4 と衝突するため採らない。
    let fallback_candidate = GeneratedWorldLayout {
        terrain_tiles: fallback_terrain(&masks, master_seed),
        anchors,
        masks,
        resource_spawn_candidates: ResourceSpawnCandidates::default(),
        initial_tree_positions: Vec::new(),
        forest_regrowth_zones: Vec::new(),
        initial_rock_positions: Vec::new(),
        master_seed,
        generation_attempt: MAX_WFC_RETRIES + 1,
        used_fallback: true,
    };
    let validated = validate::lightweight_validate(&fallback_candidate)
        .expect("fallback terrain must satisfy lightweight_validate");
    let fallback_candidate = GeneratedWorldLayout {
        resource_spawn_candidates: validated,
        ..fallback_candidate
    };
    let res = resources::generate_resource_layout_fallback(&fallback_candidate, master_seed)
        .expect("fallback resource generation must not return empty world");
    GeneratedWorldLayout {
        initial_tree_positions: res.initial_tree_positions,
        forest_regrowth_zones: res.forest_regrowth_zones,
        initial_rock_positions: res.initial_rock_positions,
        resource_spawn_candidates: ResourceSpawnCandidates {
            water_tiles: fallback_candidate.resource_spawn_candidates.water_tiles.clone(),
            sand_tiles: fallback_candidate.resource_spawn_candidates.sand_tiles.clone(),
            rock_candidates: res.rock_candidates,
        },
        ..fallback_candidate
    }
});
```

`used_fallback = true` は **地形 WFC が fallback へ落ちた**ことだけを意味し、MS-WFC-3 完了後は
resource まで空配列になることは許容しない。terrain fallback 上でも deterministic な
resource fallback を生成し、MS-WFC-4 以降の startup が `GeneratedWorldLayout` だけで完結できる状態を保つ。

**`unwrap_or_else` 内の `expect` について**: 地形 fallback が `lightweight_validate` を満たさない、または `generate_resource_layout_fallback` が `None` を返すのは **実装バグまたは定数設定の不整合**とみなす。本番でも panic で気づけるようにし、サイレントに空レイアウトを返さない。将来、フォールバック階層を増やす場合は `Result` に昇格させて明示的に扱う。

### 4.2 配置候補の取り方と定数

`mapgen/resources.rs` 冒頭に定義する公開定数:

```rust
// ── 森林ゾーン ────────────────────────────────────────────────────────────────
/// ゾーン数の下限（0 は許容しない：fallback として None を返す）
pub const FOREST_ZONE_COUNT_MIN: u32 = 2;
pub const FOREST_ZONE_COUNT_MAX: u32 = 4;
/// ゾーン半径（チェビシェフ正方形の半辺）の範囲
pub const FOREST_ZONE_RADIUS_MIN: u32 = 5;
pub const FOREST_ZONE_RADIUS_MAX: u32 = 10;
/// ゾーン中心点同士の最低チェビシェフ距離（centers が近すぎるとゾーンが重複する）
pub const FOREST_ZONE_CENTER_SPACING: u32 = 12;
/// 1 ゾーンあたりの配置木数の範囲
pub const TREES_PER_ZONE_MIN: usize = 6;
pub const TREES_PER_ZONE_MAX: usize = 16;
/// 木同士の最低チェビシェフ距離（密集しすぎ防止）
pub const TREE_MIN_SPACING: u32 = 2;

// ── 岩 ───────────────────────────────────────────────────────────────────────
pub const ROCK_COUNT_MIN: usize = 4;
pub const ROCK_COUNT_MAX: usize = 12;
/// 岩同士の最低チェビシェフ距離
pub const ROCK_MIN_SPACING: u32 = 3;
```

#### 木の配置アルゴリズム（`generate_resource_layout` 内部）

```
1. exclusion_mask = anchor_mask | tree_dense_protection_band | river_mask
                  | final_sand_mask | inland_sand_mask
2. grass_candidates = {p | terrain[p] == Grass && grass_zone_mask[p] && !exclusion_mask[p]}
3. grass_zone_mask 外の Grass を 2 次候補として補助利用可（TREES_PER_ZONE_MIN が不足する場合のみ）
4. rng から zone 数 N ∈ [FOREST_ZONE_COUNT_MIN, FOREST_ZONE_COUNT_MAX] を決定
5. grass_candidates からチェビシェフ距離 FOREST_ZONE_CENTER_SPACING 以上を保ちつつ
   N 個の中心点を選択 → N 個の WfcForestZone (radius はランダム RADIUS_MIN..=MAX)
6. 各ゾーン内の有効セル (zone.contains(p) && !exclusion_mask[p]) から
   最低 TREE_MIN_SPACING 以上離れた点を乱択して木を配置
7. ゾーン内に TREES_PER_ZONE_MIN 本以上確保できなければ 2 次候補で補充
8. 2 次候補を使っても足りない場合は None を返して attempt を破棄
```

#### 岩の配置アルゴリズム

```
1. exclusion_mask = anchor_mask | rock_protection_band | river_mask
                  | final_sand_mask | inland_sand_mask
2. dirt_candidates = {p | terrain[p] == Dirt && dirt_zone_mask[p] && !exclusion_mask[p]}
3. rng から岩数 N ∈ [ROCK_COUNT_MIN, ROCK_COUNT_MAX] を決定
4. dirt_candidates からチェビシェフ距離 ROCK_MIN_SPACING 以上を保ちつつ N 点を乱択
5. N 点に満たない場合は dirt_zone_mask 外の Dirt で補充
6. それでも ROCK_COUNT_MIN に満たなければ None を返して attempt を破棄
7. initial_rock_positions = rock_candidates = 選択した N 点
```

**fallback 時の扱い**:

- 通常 attempt では `generate_resource_layout()` が `None` を返したらその attempt を破棄する
- terrain fallback へ落ちた後は `generate_resource_layout_fallback()` を使い、
  zone 数や本数の下限を緩めてもよいが、**木・森・岩の 3 フィールドが空にならないこと**を優先する
- fallback resource でも `validate_post_resource()` は通す

### 4.3 exclusion zone（木・岩で使うマスクを分離）

**木の除外領域**（`tree_dense_protection_band` width=2 を使う）:

| マスク | 理由 |
| --- | --- |
| `masks.anchor_mask` | Site / Yard 内には配置しない |
| `masks.tree_dense_protection_band` | アンカー外周 2 マスは高密度木禁止帯（ms0 §3.1） |
| `masks.river_mask` | River 上に木を置かない |
| `masks.final_sand_mask` / `masks.inland_sand_mask` | 砂地との見た目・導線の一貫性 |

**岩の除外領域**（`rock_protection_band` width=2 を使う）:

| マスク | 理由 |
| --- | --- |
| `masks.anchor_mask` | Site / Yard 内には配置しない |
| `masks.rock_protection_band` | アンカー外周 2 マスは岩禁止帯（ms0 §3.1） |
| `masks.river_mask` | River 上に岩を置かない |
| `masks.final_sand_mask` / `masks.inland_sand_mask` | 砂地との見た目・導線の一貫性 |

> **注**: `initial_wood_positions` / `wheelbarrow_parking` は Yard 内にあるため `anchor_mask` で既に除外される。
> `tree_dense_protection_band` / `rock_protection_band` はアンカー外周 2 マスを含むため、
> Yard 直外の作業導線も自動的に保護される。`combined_protection_band()` は使わない（river band まで
> 含んで過剰に除外してしまうため）。

### 4.4 森林ゾーンの表現

`WfcForestZone` 自体はすでに `mapgen/types.rs` にあるため、MS-WFC-3 では shape を変えない。

```rust
pub struct WfcForestZone {
    pub center: GridPos,
    pub radius: u32,
}
```

- 包含判定は既存どおりチェビシェフ距離ベース
- この MS では **旧 `regrowth::ForestZone` の削除・`WfcForestZone` への名称統一は行わない**（`mapgen/types.rs` のドキュメントと一致させた）
- 必要なら `hw_world` 側に pure な変換 helper を追加するが、`bevy_app` への注入は MS-WFC-4

### 4.5 到達性の扱い（`validate_post_resource` の実装方針）

現行の `lightweight_validate` は木・岩を置く前の地形だけを見ている。
MS-WFC-3 では `validate.rs` に以下を追加し、**木・岩をパス不可障害物として導線を再確認**する。

**追加する crate 内公開関数**:

```rust
// crates/hw_world/src/mapgen/validate.rs に追加

/// 木・岩配置後の到達性確認。
/// `layout` は lightweight_validate 通過済み（water_tiles / sand_tiles が入っている）。
/// `resource` の tree / rock positions を walk 不可障害物として追加し、
/// Site↔Yard, Yard→水, Yard→砂, Yard→岩（隣接）を再確認する。
pub(crate) fn validate_post_resource(
    layout: &GeneratedWorldLayout,
    resource: &ResourceLayout,
) -> Result<(), ValidationError>;
```

**岩への到達の定義（地形フェーズとの差）**:

- 地形のみの `collect_required_resource_candidates` では、岩候補に対し `can_reach_target(..., target_walkable: true)` で **Dirt マスをゴールとして**到達を確認する（`validate.rs` 既存どおり）。
- 資源配置後は岩・木の座標を **歩行不可の障害物**として重ねるため、岩タイル自体には立てない。よって `validate_post_resource` では岩に対し `can_reach_target(..., false)` とし、**隣接タイルからの到達**を要求する。いずれもゲーム上「その資源にアクセス可能」を表すが、フェーズで前提が変わることは意図的である。

**内部実装スケッチ**:

```rust
pub fn validate_post_resource(
    layout: &GeneratedWorldLayout,
    resource: &ResourceLayout,
) -> Result<(), ValidationError> {
    // 木・岩を障害物セットとして構築
    let mut obstacles: HashSet<GridPos> = HashSet::new();
    obstacles.extend(resource.initial_tree_positions.iter().copied());
    obstacles.extend(resource.initial_rock_positions.iter().copied());

    let world = ResourceObstaclePathWorld {
        tiles: &layout.terrain_tiles,
        obstacles: &obstacles,
    };
    let mut ctx = PathfindingContext::default();
    let site_rep = (layout.anchors.site.min_x, layout.anchors.site.min_y);
    let yard_rep = (layout.anchors.yard.min_x, layout.anchors.yard.min_y);

    // Site ↔ Yard
    if !can_reach_target(&world, &mut ctx, site_rep, yard_rep, true) {
        return Err(ValidationError::SiteYardNotReachable);
    }
    // Yard → 水源（1 件でも到達可能な隣接があれば OK）
    let has_water = layout.resource_spawn_candidates.water_tiles.iter()
        .any(|&p| can_reach_target(&world, &mut ctx, yard_rep, p, false));
    if !has_water {
        return Err(ValidationError::RequiredResourceNotReachable);
    }
    // Yard → 砂源（1 件でも到達可能なら OK）
    let has_sand = layout.resource_spawn_candidates.sand_tiles.iter()
        .any(|&p| can_reach_target(&world, &mut ctx, yard_rep, p, true));
    if !has_sand {
        return Err(ValidationError::RequiredResourceNotReachable);
    }
    // Yard → 岩（1 件でも隣接到達可能なら OK）
    let has_rock = resource.initial_rock_positions.iter()
        .any(|&p| can_reach_target(&world, &mut ctx, yard_rep, p, false));
    if !has_rock {
        return Err(ValidationError::RequiredResourceNotReachable);
    }
    Ok(())
}

// validate.rs 内部専用。ValidatorPathWorld に障害物オーバーレイを追加した版。
struct ResourceObstaclePathWorld<'a> {
    tiles: &'a [TerrainType],
    obstacles: &'a HashSet<GridPos>,
}

impl PathWorld for ResourceObstaclePathWorld<'_> {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> { /* 同上 */ }
    fn idx_to_pos(&self, idx: usize) -> GridPos { /* 同上 */ }
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        !self.obstacles.contains(&(x, y))
            && self.pos_to_idx(x, y)
                .map(|i| self.tiles[i].is_walkable())
                .unwrap_or(false)
    }
    fn get_door_cost(&self, _x: i32, _y: i32) -> i32 { 0 }
}
```

---

## 5. 実装ステップ

### Step 1: `mapgen/resources.rs` を新規作成

**追加内容**:
- 公開定数（§4.2 の定数表すべて）
- `pub(crate) ResourceLayout` 構造体
- `generate_resource_layout(layout: &GeneratedWorldLayout, seed: u64) -> Option<ResourceLayout>` 本体
  - RNG: `StdRng::seed_from_u64(seed)` で determinism を保証
  - `generate_forest_zones()` / `place_trees()` / `place_rocks()` を内部 helper に分割
- `generate_resource_layout_fallback(layout: &GeneratedWorldLayout, seed: u64) -> Option<ResourceLayout>`
  - terrain fallback 用の縮退版。下限を緩めてもよいが空 world は返さない
- `mapgen.rs` に `pub mod resources;` を追加

**`generate_forest_zones` 内部 helper の骨格**:

```rust
fn generate_forest_zones(
    rng: &mut StdRng,
    layout: &GeneratedWorldLayout,
) -> Option<Vec<WfcForestZone>> {
    let exclusion = build_tree_exclusion(layout);
    let candidates: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).filter_map(move |x| {
            let p = (x, y);
            let idx = (y * MAP_WIDTH + x) as usize;
            (layout.terrain_tiles[idx] == TerrainType::Grass
                && layout.masks.grass_zone_mask.get(p)
                && !exclusion.get(p))
            .then_some(p)
        }))
        .collect();
    if candidates.is_empty() { return None; }

    let zone_count = rng.gen_range(FOREST_ZONE_COUNT_MIN..=FOREST_ZONE_COUNT_MAX);
    let mut centers: Vec<GridPos> = Vec::new();
    let mut shuffled = candidates.clone();
    shuffled.shuffle(rng);
    for p in shuffled {
        if centers.iter().all(|&c| chebyshev(c, p) >= FOREST_ZONE_CENTER_SPACING) {
            centers.push(p);
            if centers.len() == zone_count as usize { break; }
        }
    }
    if (centers.len() as u32) < FOREST_ZONE_COUNT_MIN { return None; }

    Some(centers.into_iter().map(|center| WfcForestZone {
        center,
        radius: rng.gen_range(FOREST_ZONE_RADIUS_MIN..=FOREST_ZONE_RADIUS_MAX),
    }).collect())
}

/// anchor_mask | tree_dense_protection_band | river_mask | final_sand_mask | inland_sand_mask
fn build_tree_exclusion(layout: &GeneratedWorldLayout) -> BitGrid {
    let mut ex = layout.masks.anchor_mask.clone();
    ex |= &layout.masks.tree_dense_protection_band;
    ex |= &layout.masks.river_mask;
    ex |= &layout.masks.final_sand_mask;
    ex |= &layout.masks.inland_sand_mask;
    ex
}
```

**`place_trees` 内部 helper の骨格**:

```rust
fn place_trees(
    rng: &mut StdRng,
    zones: &[WfcForestZone],
    layout: &GeneratedWorldLayout,
    exclusion: &BitGrid,
) -> Option<Vec<GridPos>> {
    let mut all_trees: Vec<GridPos> = Vec::new();
    for zone in zones {
        let mut zone_trees: Vec<GridPos> = Vec::new();
        let target = rng.gen_range(TREES_PER_ZONE_MIN..=TREES_PER_ZONE_MAX);
        let mut zone_cells: Vec<GridPos> = // zone.contains(p) && terrain==Grass && !exclusion
            ...collect...;
        zone_cells.shuffle(rng);
        for p in zone_cells {
            if all_trees.iter().chain(&zone_trees)
                .all(|&t| chebyshev(t, p) >= TREE_MIN_SPACING)
            {
                zone_trees.push(p);
                if zone_trees.len() >= target { break; }
            }
        }
        if zone_trees.len() < TREES_PER_ZONE_MIN {
            // 2 次候補（grass_zone_mask 外の Grass）で補充を試みる
            // ...
        }
        if zone_trees.len() < TREES_PER_ZONE_MIN { return None; }
        all_trees.extend(zone_trees);
    }
    Some(all_trees)
}
```

**`place_rocks` 内部 helper の骨格**:

```rust
fn place_rocks(
    rng: &mut StdRng,
    layout: &GeneratedWorldLayout,
) -> Option<Vec<GridPos>> {
    let exclusion = build_rock_exclusion(layout);
    let count = rng.gen_range(ROCK_COUNT_MIN..=ROCK_COUNT_MAX);
    let mut candidates: Vec<GridPos> = // terrain==Dirt && dirt_zone_mask && !exclusion
        ...collect...;
    candidates.shuffle(rng);
    let mut rocks: Vec<GridPos> = Vec::new();
    for p in candidates {
        if rocks.iter().all(|&r| chebyshev(r, p) >= ROCK_MIN_SPACING) {
            rocks.push(p);
            if rocks.len() >= count { break; }
        }
    }
    // 2 次候補（dirt_zone_mask 外の Dirt）で補充
    if rocks.len() < ROCK_COUNT_MIN { /* try fallback candidates */ }
    if rocks.len() < ROCK_COUNT_MIN { return None; }
    Some(rocks)
}

fn build_rock_exclusion(layout: &GeneratedWorldLayout) -> BitGrid {
    let mut ex = layout.masks.anchor_mask.clone();
    ex |= &layout.masks.rock_protection_band;
    ex |= &layout.masks.river_mask;
    ex |= &layout.masks.final_sand_mask;
    ex |= &layout.masks.inland_sand_mask;
    ex
}
```

**`generate_resource_layout` 全体フロー**:

```rust
pub fn generate_resource_layout(
    layout: &GeneratedWorldLayout,
    seed: u64,
) -> Option<ResourceLayout> {
    let mut rng = StdRng::seed_from_u64(seed);
    let forest_regrowth_zones = generate_forest_zones(&mut rng, layout)?;
    let exclusion = build_tree_exclusion(layout);
    let initial_tree_positions = place_trees(&mut rng, &forest_regrowth_zones, layout, &exclusion)?;
    let initial_rock_positions = place_rocks(&mut rng, layout)?;
    let rock_candidates = initial_rock_positions.clone();
    Some(ResourceLayout {
        initial_tree_positions,
        forest_regrowth_zones,
        initial_rock_positions,
        rock_candidates,
    })
}
```

### Step 2: 木配置を `grass_zone_mask` に接続

- `forest_regrowth_zones` を生成（Step 1 で実装済み）
- `initial_tree_positions` が各 zone に包含されることを `debug_assert!` で確認
- 全初期木が zone 内にある不変条件はコード上で保証する（テストで追加確認）

### Step 3: 岩配置を `dirt_zone_mask` に接続

- `initial_rock_positions` を確定し `rock_candidates` と等値で設定
- `ResourceSpawnCandidates::rock_candidates` の producer 不在を解消

### Step 4: `generate_world_layout()` に統合

- §4.1 の統合コードを `mapgen.rs` の `find_map` クロージャに書き込む
- fallback 分岐でも `generate_resource_layout_fallback()` を通して木・森・岩を埋める

### Step 5: `validate_post_resource` を `validate.rs` に追加

- §4.5 の実装スケッチをそのまま実装
- `ResourceObstaclePathWorld` を `validate.rs` に private 型として追加
- 失敗時は `ValidationError::SiteYardNotReachable` または `RequiredResourceNotReachable` を返す

---

## 6. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/resources.rs` (新規) | `generate_resource_layout()` / `generate_resource_layout_fallback()` / `pub(crate) ResourceLayout` / 候補選定・配置ロジック |
| `crates/hw_world/src/mapgen.rs` | `pub mod resources;` を追加し、`generate_world_layout()` に resource generation・post-resource validation・resource fallback を統合 |
| `crates/hw_world/src/mapgen/types.rs` | 既存 `GeneratedWorldLayout` / `WfcForestZone` を流用。`ResourceLayout` はここには置かない |
| `crates/hw_world/src/regrowth.rs` | 必要なら `WfcForestZone` → legacy `ForestZone` の pure 変換 helper を追加。ただし本接続は MS-WFC-4 |
| `crates/bevy_app/src/world/regrowth.rs` | 原則この MS では未変更。MS-WFC-4 で `RegrowthManager` の入力切り替え |
| `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs` | 原則この MS では未変更。MS-WFC-4 で `GeneratedWorldLayout` 読み出しへ切り替え |

`crates/hw_world/src/layout.rs` の deprecated コメントは **すでに追加済み** なので、この MS の必須変更対象ではない。

---

## 7. 完了条件チェックリスト

- [ ] `generate_world_layout()` が成功レイアウトで `initial_tree_positions` / `forest_regrowth_zones` / `initial_rock_positions` を空ではなく返す
- [ ] 木が `grass_zone_mask` を主な優先領域として生成される
- [ ] 岩が `dirt_zone_mask` を主な優先領域として生成される
- [ ] 木が `anchor_mask` / `tree_dense_protection_band` / `river_mask` / `final_sand_mask` / `inland_sand_mask` に入らない（§4.3。`combined_protection_band()` は exclusion に使わない）
- [ ] 岩が `anchor_mask` / `rock_protection_band` / `river_mask` / `final_sand_mask` / `inland_sand_mask` に入らない（§4.3）
- [ ] `initial_tree_positions` が必ず `forest_regrowth_zones` の部分集合になっている
- [ ] `ResourceSpawnCandidates::rock_candidates` の producer 不在が解消されている
- [ ] 資源配置後も `Site↔Yard` と `Yard→必須資源` の到達可能性が維持される
- [ ] terrain fallback に入っても resource fallback により木・森・岩が空配列にならない
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 8. テスト

```rust
// crates/hw_world/src/mapgen/resources.rs (または mapgen.rs の #[cfg(test)])

const TEST_SEED_A: u64 = 10_182_272_928_891_625_829;
const TEST_SEED_B: u64 = 12_345_678;

#[test]
fn trees_not_in_exclusion_zone() {
    let layout = generate_world_layout(TEST_SEED_A);
    assert!(!layout.used_fallback, "seed={TEST_SEED_A}: fallback が使われた");
    for &pos in &layout.initial_tree_positions {
        assert!(!layout.masks.anchor_mask.get(pos),
            "tree at {pos:?} is inside anchor_mask");
        assert!(!layout.masks.tree_dense_protection_band.get(pos),
            "tree at {pos:?} is inside tree_dense_protection_band");
        assert!(!layout.masks.river_mask.get(pos),
            "tree at {pos:?} is inside river_mask");
        assert!(!layout.masks.final_sand_mask.get(pos),
            "tree at {pos:?} is inside final_sand_mask");
        assert!(!layout.masks.inland_sand_mask.get(pos),
            "tree at {pos:?} is inside inland_sand_mask");
    }
}

#[test]
fn trees_are_inside_some_forest_zone() {
    let layout = generate_world_layout(TEST_SEED_A);
    assert!(!layout.used_fallback);
    for &pos in &layout.initial_tree_positions {
        assert!(
            layout.forest_regrowth_zones.iter().any(|z| z.contains(pos)),
            "tree at {pos:?} is outside all forest_regrowth_zones"
        );
    }
}

#[test]
fn rocks_not_in_exclusion_zone() {
    let layout = generate_world_layout(TEST_SEED_A);
    assert!(!layout.used_fallback);
    for &pos in &layout.initial_rock_positions {
        assert!(!layout.masks.anchor_mask.get(pos),
            "rock at {pos:?} is inside anchor_mask");
        assert!(!layout.masks.rock_protection_band.get(pos),
            "rock at {pos:?} is inside rock_protection_band");
        assert!(!layout.masks.river_mask.get(pos),
            "rock at {pos:?} is inside river_mask");
        assert!(!layout.masks.final_sand_mask.get(pos),
            "rock at {pos:?} is inside final_sand_mask");
        assert!(!layout.masks.inland_sand_mask.get(pos),
            "rock at {pos:?} is inside inland_sand_mask");
    }
}

#[test]
fn resource_layout_keeps_required_paths_open() {
    for seed in [TEST_SEED_A, TEST_SEED_B] {
        let layout = generate_world_layout(seed);
        assert!(!layout.used_fallback, "seed={seed}: fallback が使われた");
        // 木・岩フィールドが実際に埋まっていること
        assert!(!layout.initial_tree_positions.is_empty(),
            "seed={seed}: initial_tree_positions が空");
        assert!(!layout.forest_regrowth_zones.is_empty(),
            "seed={seed}: forest_regrowth_zones が空");
        assert!(!layout.initial_rock_positions.is_empty(),
            "seed={seed}: initial_rock_positions が空");
        // post-resource 到達確認（validate_post_resource を直接呼ぶ）
        use crate::mapgen::resources::ResourceLayout;
        use crate::mapgen::validate::validate_post_resource;
        let res = ResourceLayout {
            initial_tree_positions: layout.initial_tree_positions.clone(),
            forest_regrowth_zones: layout.forest_regrowth_zones.clone(),
            initial_rock_positions: layout.initial_rock_positions.clone(),
            rock_candidates: layout.resource_spawn_candidates.rock_candidates.clone(),
        };
        assert!(
            validate_post_resource(&layout, &res).is_ok(),
            "seed={seed}: validate_post_resource failed"
        );
    }
}

#[test]
fn rock_candidates_equals_initial_rock_positions() {
    let layout = generate_world_layout(TEST_SEED_A);
    assert!(!layout.used_fallback);
    // rock_candidates は initial_rock_positions と同一集合であること
    let mut expected = layout.initial_rock_positions.clone();
    let mut actual = layout.resource_spawn_candidates.rock_candidates.clone();
    expected.sort();
    actual.sort();
    assert_eq!(expected, actual);
}

#[test]
fn resource_layout_is_deterministic() {
    let l1 = generate_world_layout(TEST_SEED_A);
    let l2 = generate_world_layout(TEST_SEED_A);
    assert_eq!(l1.initial_tree_positions, l2.initial_tree_positions);
    assert_eq!(l1.initial_rock_positions, l2.initial_rock_positions);
    assert_eq!(
        l1.forest_regrowth_zones.iter().map(|z| (z.center, z.radius)).collect::<Vec<_>>(),
        l2.forest_regrowth_zones.iter().map(|z| (z.center, z.radius)).collect::<Vec<_>>(),
    );
}
```

---

## 9. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace
```

手動確認:

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo run` で seed を変えると木・岩の分布が変化する
- `Site/Yard` とその保護帯に木・岩が食い込まない
- 旧 startup 経路のままでも、MS-WFC-4 着手時に読み替えるための pure data が揃っている

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-06` | `Cursor` | レビュー反映：メタ下に「この文書の読み方」、§4.1 の `expect` 方針、§4.5 に岩到達（地形フェーズ vs post-resource）の差の説明、§7 を §4.3 と整合（`tree_dense` / `rock` 帯に分離、`combined_protection_band` 明記）、重複 `---` 削除 |
| `2026-04-06` | `Codex` | レビュー反映。`ResourceLayout` を `mapgen` 内共有の `pub(crate)` 型へ整理し、統合コード例の所有権問題を解消。terrain fallback 後も deterministic な resource fallback を必須に変更し、MS-WFC-4 と整合する契約へ修正 |
| `2026-04-06` | `Copilot` | 全面ブラッシュアップ：`ResourceLayout` 構造体定義、`generate_resource_layout` シグネチャを `&GeneratedWorldLayout` へ統一、定数表追加（`FOREST_ZONE_*` / `ROCK_*`）、木・岩の exclusion mask を `tree_dense_protection_band` / `rock_protection_band` に分離（`combined_protection_band` 廃止）、`validate_post_resource` 実装スケッチ追加（`ResourceObstaclePathWorld`）、`generate_world_layout` 統合コード追加、テストを 6 件に増強（`TEST_SEED_B` 追加、`validate_post_resource` 直接呼び出し、決定論テスト追加） |
| `2026-04-05` | `Cursor` | レビュー反映：前提の `resource_spawn_candidates` 明確化、`validated_candidates` マージ方針、リトライ一塊、`generate_world_layout` 表の文言、§4.4 と `types.rs` の整合、岩テストの砂除外、検証コマンドの `CARGO_HOME` 統一 |
| `2026-04-05` | `Codex` | 現状実装に合わせて全面更新。`terrain_zones` 前提、`WfcForestZone` 先行実装、deprecated コメント追加済み、startup/regrowth 未接続、`rock_candidates` producer 不在を反映 |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
