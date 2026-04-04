# MS-WFC-1: 固定アンカー定義と生成結果モデル化

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms1-anchor-data-model` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms0-invariant-spec.md`](wfc-ms0-invariant-spec.md) |
| 次MS | [`wfc-ms2a-crate-adapter-river-mask.md`](wfc-ms2a-crate-adapter-river-mask.md) |

---

## 1. 目的

WFC 実装の前に、**生成データのコントラクト**を先に確定する。

- `Site/Yard` の固定領域と Yard 内固定アンカー（初期木材・猫車置き場）を pure な Rust 型として `hw_world` に定義する
- 生成結果全体を表す `GeneratedWorldLayout` 構造体を定義し、診断用中間結果と fallback 状態も保持できる形にする
- 現在の `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` は **Site 内に存在する誤った位置**であり、Yard 内の正しいオフセットへ移行する準備を行う
- `bevy_app` の startup 切り替えは後続 MS（MS-WFC-4）で行うが、**型とモジュール境界だけはここで確定させる**

---

## 2. コードベースの現状（調査済み）

### 2.1 現在の Site/Yard 座標

`hw_core::constants` から導かれる計算値:

| ゾーン | min (x, y) | max (x, y) | 注記 |
| --- | --- | --- | --- |
| Site | (30, 40) | (69, 59) | 両端含む inclusive |
| Yard | (70, 40) | (89, 59) | 両端含む inclusive |

計算式（`bevy_app::initial_spawn::layout::compute_site_yard_layout` と同じ）:
```
site_min_x = (MAP_WIDTH - SITE_WIDTH_TILES) / 2  = (100 - 40) / 2 = 30
site_max_x = site_min_x + 40 - 1                              = 69
yard_min_x = site_max_x + 1                                    = 70
yard_max_x = yard_min_x + 20 - 1                               = 89
```

### 2.2 現状の問題点

| 問題 | 場所 | 状況 |
| --- | --- | --- |
| `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` | `bevy_app/initial_spawn/mod.rs:15` | **(58,58) は Site 内** (x:30-69, y:40-59) — Yard 内固定条件と矛盾 |
| `INITIAL_WOOD_POSITIONS = [(48,48),(52,52),...]` | `hw_world/src/layout.rs` | **全 5 点が Site 内** — 同上 |
| `ROCK_POSITIONS`（クラスター1） | `hw_world/src/layout.rs` | **(75-79, 50-54) の 25 点が Yard 内** (x:70-89, y:40-59) — Yard 内には岩を生成しない invariant と矛盾。現行コードは固定配置のため既に衝突状態 |
| `TREE_POSITIONS` の一部 | `hw_world/src/layout.rs` | **(45,55), (55,45), (65,55), (38,58), (42,42) が Site 内、(70,45) が Yard 内** の 6 点 — Site/Yard 内には木を生成しない invariant と矛盾 |
| `ForestZone { min, max, initial_count, tree_positions }` | `hw_world/src/regrowth.rs` | ボックス形状 + 固定座標リスト埋め込み。新設計（center+radius）と形状が異なる |

### 2.3 座標型の規約

`hw_world` のグリッド座標は現在すべて `(i32, i32)` タプルを使用している。
`GridPos = (i32, i32)` が `hw_core::world` に定義されており、`hw_world` が `hw_core` に依存している。
新規型でも **`GridPos = (i32, i32)` を使う**（`IVec2` への統一は別 Issue）。

---

## 3. 設計方針

### 3.1 crate 境界

```
hw_world:
  - AnchorLayout         … Site/Yard 矩形 + Yard 内固定オフセット (pure data)
  - WorldMasks           … BitGrid ベースのマスク群 (診断用)
  - GeneratedWorldLayout … hw_world → bevy_app 間のコントラクト (pure data)

bevy_app:
  - 生成結果を消費するだけ（MS-WFC-4 で切り替える）
  - この MS では `compute_site_yard_layout()` を `AnchorLayout::try_fixed()` 由来へ寄せる最小変更のみ入れる
```

### 3.2 廃止予定（この MS ではコメントのみ、削除は MS-WFC-4）

| 廃止対象 | 場所 | 移行先 |
| --- | --- | --- |
| `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` | `bevy_app/initial_spawn/mod.rs` | `AnchorLayout::fixed().wheelbarrow_parking` |
| `INITIAL_WOOD_POSITIONS` | `hw_world/src/layout.rs` | `AnchorLayout::fixed().initial_wood_positions` |
| `ROCK_POSITIONS`（クラスター1: 75-79, 50-54） | `hw_world/src/layout.rs` | WFC 生成の岩配置に置き換え（Yard 内は生成禁止）|
| `TREE_POSITIONS`（Site/Yard 内の 6 点） | `hw_world/src/layout.rs` | WFC 生成の木配置に置き換え（Site/Yard 内は生成禁止）|
| `SiteYardLayout` (bevy_app 内) | `bevy_app/initial_spawn/layout.rs` | `AnchorLayout` (hw_world 内) |

> **Note**: `ROCK_POSITIONS` と `TREE_POSITIONS` の **Yard/Site 外の座標** は WFC 移行後も参照される可能性があるため、
> この MS では「一部座標が anchor 内に存在する」旨のコメント追加のみとし、定数自体の削除は MS-WFC-4 で判断する。

### 3.3 `ForestZone` 命名衝突の解決方針

既存の `hw_world::ForestZone` は `regrowth.rs` に `{ min, max, initial_count, tree_positions }` で存在する。
WFC 用の新型は形状が異なる（center + radius、**包含判定はチェビシェフ距離で確定**）ため、**この MS では `WfcForestZone` という仮名で追加**する。
MS-WFC-3 で既存 `ForestZone` の形状を `WfcForestZone` に統一し、名前を `ForestZone` に戻す。

### 3.4 `Site` / `Yard` 矩形の単一ソース（ドリフト防止）

唯一のソースは **`hw_world::AnchorLayout::try_fixed()`** とし、
**`bevy_app::compute_site_yard_layout()`** はその戻り値を `SiteYardLayout` へ写像するだけにする。

- これにより、定数（`SITE_WIDTH_TILES` 等）変更時の drift は `hw_world` 側の 1 箇所に閉じる。
- `bevy_app` 側の `SiteYardLayoutError` は互換性維持のため残し、`AnchorLayoutError` から変換する。
- `AnchorLayout::fixed()` は `try_fixed().expect(...)` の thin wrapper とする。

---

## 4. 実装詳細

### 4.1 新規ファイル: `crates/hw_world/src/anchor.rs`

```rust
use hw_core::world::GridPos;
use hw_core::constants::*;

/// 矩形グリッド領域（両端 inclusive）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridRect {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32, // inclusive
    pub max_y: i32, // inclusive
}

impl GridRect {
    pub fn contains(&self, pos: GridPos) -> bool {
        pos.0 >= self.min_x && pos.0 <= self.max_x
            && pos.1 >= self.min_y && pos.1 <= self.max_y
    }

    /// セル数（面積）
    pub fn area(&self) -> usize {
        ((self.max_x - self.min_x + 1) * (self.max_y - self.min_y + 1)) as usize
    }

    /// 全セルを row-major でイテレートする
    pub fn iter_cells(&self) -> impl Iterator<Item = GridPos> + '_ {
        let (min_x, min_y, max_x, max_y) = (self.min_x, self.min_y, self.max_x, self.max_y);
        (min_y..=max_y).flat_map(move |y| (min_x..=max_x).map(move |x| (x, y)))
    }
}

/// マップ上の固定アンカー配置。pure data（Bevy・WorldMap 依存なし）。
#[derive(Debug, Clone)]
pub struct AnchorLayout {
    /// Site が占有する矩形（両端 inclusive）
    pub site: GridRect,
    /// Yard が占有する矩形（両端 inclusive）
    pub yard: GridRect,
    /// Yard 内固定の初期木材座標（全点が yard 内に収まる）
    pub initial_wood_positions: Vec<GridPos>,
    /// Yard 内固定の猫車置き場フットプリント（2×2, 両端 inclusive）
    pub wheelbarrow_parking: GridRect,
}

impl AnchorLayout {
    /// 現行定数から固定配置を計算して返す。
    /// `compute_site_yard_layout()` と同じロジックを hw_world 側に持つ。
    pub fn fixed() -> Self {
        let site_w = SITE_WIDTH_TILES as i32;
        let site_h = SITE_HEIGHT_TILES as i32;
        let yard_w = YARD_INITIAL_WIDTH_TILES as i32;
        let yard_h = YARD_INITIAL_HEIGHT_TILES as i32;

        let site_min_x = (MAP_WIDTH - site_w) / 2;       // = 30
        let site_min_y = (MAP_HEIGHT - site_h) / 2;      // = 40
        let site_max_x = site_min_x + site_w - 1;        // = 69
        let site_max_y = site_min_y + site_h - 1;        // = 59

        let yard_min_x = site_max_x + 1;                 // = 70
        let yard_min_y = site_min_y;                     // = 40
        let yard_max_x = yard_min_x + yard_w - 1;        // = 89
        let yard_max_y = yard_min_y + yard_h - 1;        // = 59

        AnchorLayout {
            site: GridRect { min_x: site_min_x, min_y: site_min_y,
                             max_x: site_max_x, max_y: site_max_y },
            yard: GridRect { min_x: yard_min_x, min_y: yard_min_y,
                             max_x: yard_max_x, max_y: yard_max_y },
            // Yard 左端付近（yard_min_x + 1〜5、中央 y 付近）に 5 点配置
            // 旧 INITIAL_WOOD_POSITIONS は Site 内 (48-53, 46-52) — 誤り
            initial_wood_positions: vec![
                (yard_min_x + 1, yard_min_y + 5),  // (71, 45)
                (yard_min_x + 2, yard_min_y + 4),  // (72, 44)
                (yard_min_x + 3, yard_min_y + 6),  // (73, 46)
                (yard_min_x + 4, yard_min_y + 3),  // (74, 43)
                (yard_min_x + 5, yard_min_y + 8),  // (75, 48)
            ],
            // 旧 (58, 58) は Site 内 — 誤り。Yard 中央寄りの 2×2 空間を確保
            wheelbarrow_parking: GridRect {
                min_x: yard_min_x + 12,
                min_y: yard_min_y + 12,
                max_x: yard_min_x + 13,
                max_y: yard_min_y + 13,
            }, // (82,52) - (83,53)
        }
    }

    /// Site と Yard の合成マスク判定
    pub fn is_anchor_cell(&self, pos: GridPos) -> bool {
        self.site.contains(pos) || self.yard.contains(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_layout_fixed_wood_in_yard() {
        let layout = AnchorLayout::fixed();
        for pos in &layout.initial_wood_positions {
            assert!(
                layout.yard.contains(*pos),
                "initial_wood_positions {:?} is not inside Yard {:?}",
                pos, layout.yard
            );
        }
    }

    #[test]
    fn anchor_layout_fixed_parking_in_yard() {
        let layout = AnchorLayout::fixed();
        for pos in layout.wheelbarrow_parking.iter_cells() {
            assert!(
                layout.yard.contains(pos),
                "parking footprint {:?} not inside Yard",
                pos
            );
        }
    }

    #[test]
    fn anchor_layout_fixed_site_not_in_yard() {
        let layout = AnchorLayout::fixed();
        // 旧誤り位置 (58,58) が Site 内にあること（退行テスト）
        assert!(layout.site.contains((58, 58)));
        assert!(!layout.yard.contains((58, 58)));
    }
}
```

**注意**: `SITE_WIDTH_TILES` 等は `f32` なので `as i32` キャストが必要。整数変換の一貫性を保つため、
`anchor.rs` 内でのみキャストし、他の場所から直接 `SITE_WIDTH_TILES as i32` を書かないこと。
`MAP_WIDTH` / `MAP_HEIGHT` は `i32` なので `BitGrid::new(MAP_WIDTH, MAP_HEIGHT)` はそのまま使える。

---

### 4.2 新規ファイル: `crates/hw_world/src/world_masks.rs`

```rust
use hw_core::constants::{MAP_WIDTH, MAP_HEIGHT};
use hw_core::world::GridPos;

/// 2D boolean グリッド（row-major, (x + y * MAP_WIDTH) indexing）。
#[derive(Debug, Clone)]
pub struct BitGrid {
    data: Vec<bool>,
    width: i32,
    height: i32,
}

impl BitGrid {
    pub fn new(width: i32, height: i32) -> Self {
        Self { data: vec![false; (width * height) as usize], width, height }
    }

    /// MAP_WIDTH × MAP_HEIGHT で初期化するショートカット
    pub fn map_sized() -> Self {
        Self::new(MAP_WIDTH, MAP_HEIGHT)
    }

    pub fn get(&self, pos: GridPos) -> bool {
        match self.pos_to_idx(pos) {
            Some(i) => self.data[i],
            None => false,
        }
    }

    pub fn set(&mut self, pos: GridPos, val: bool) {
        if let Some(i) = self.pos_to_idx(pos) {
            self.data[i] = val;
        }
    }

    pub fn count_set(&self) -> usize {
        self.data.iter().filter(|&&b| b).count()
    }

    fn pos_to_idx(&self, pos: GridPos) -> Option<usize> {
        if pos.0 < 0 || pos.1 < 0 || pos.0 >= self.width || pos.1 >= self.height {
            return None;
        }
        Some((pos.1 * self.width + pos.0) as usize)
    }
}

impl std::ops::BitOrAssign<&BitGrid> for BitGrid {
    fn bitor_assign(&mut self, rhs: &BitGrid) {
        debug_assert_eq!(self.data.len(), rhs.data.len());
        for (a, b) in self.data.iter_mut().zip(&rhs.data) {
            *a |= b;
        }
    }
}

/// 各生成フェーズのマスク。診断とデバッグに使う。
/// 各フィールドは該当セルが true のとき「そのカテゴリに属する」。
#[derive(Debug, Clone)]
pub struct WorldMasks {
    /// Site が占有するセル
    pub site_mask: BitGrid,
    /// Yard が占有するセル
    pub yard_mask: BitGrid,
    /// site_mask | yard_mask
    pub anchor_mask: BitGrid,
    /// anchor 外周の River 禁止帯（MS0: PROTECTION_BAND_RIVER_WIDTH）
    pub river_protection_band: BitGrid,
    /// anchor 外周の岩禁止帯（MS0: PROTECTION_BAND_ROCK_WIDTH）
    pub rock_protection_band: BitGrid,
    /// anchor 外周の高密度木禁止帯（MS0: PROTECTION_BAND_TREE_DENSE_WIDTH）
    pub tree_dense_protection_band: BitGrid,
    /// 川タイル（WFC hard constraint として渡す）
    pub river_mask: BitGrid,
    /// 川の中心線点列（デバッグ表示・砂配置計算に使う）
    pub river_centerline: Vec<GridPos>,
}

impl WorldMasks {
    /// アンカー情報から site/yard/anchor マスクを初期化する。
    /// `river_*` / 各 `*_protection_band` は **MS-WFC-2a** で埋める。
    /// 帯の幾何は wfc-ms0-invariant-spec §3.1.1（アンカー外周からの距離）に合わせ、
    /// `anchor_mask` から純粋関数で BitGrid を生成する実装を推奨（実装詳細は MS-WFC-2a 計画へ委譲）。
    pub fn from_anchor(anchor: &crate::anchor::AnchorLayout) -> Self {
        let mut site_mask = BitGrid::map_sized();
        let mut yard_mask = BitGrid::map_sized();
        let mut anchor_mask = BitGrid::map_sized();

        for pos in anchor.site.iter_cells() {
            site_mask.set(pos, true);
            anchor_mask.set(pos, true);
        }
        for pos in anchor.yard.iter_cells() {
            yard_mask.set(pos, true);
            anchor_mask.set(pos, true);
        }

        WorldMasks {
            site_mask,
            yard_mask,
            anchor_mask,
            river_protection_band: BitGrid::map_sized(),      // MS-WFC-2a で設定
            rock_protection_band: BitGrid::map_sized(),       // MS-WFC-2a で設定
            tree_dense_protection_band: BitGrid::map_sized(), // MS-WFC-2a で設定
            river_mask: BitGrid::map_sized(),         // MS-WFC-2a で設定
            river_centerline: Vec::new(),             // MS-WFC-2a で設定
        }
    }

    /// debug report 用の合成保護帯。
    /// MS-WFC-0 / 親計画でいう `protection_band` はこの合成結果に相当する。
    pub fn combined_protection_band(&self) -> BitGrid {
        let mut combined = self.river_protection_band.clone();
        combined |= &self.rock_protection_band;
        combined |= &self.tree_dense_protection_band;
        combined
    }
}
```

---

### 4.3 新規ファイル: `crates/hw_world/src/mapgen/types.rs`

```rust
use hw_core::world::GridPos;
use crate::anchor::AnchorLayout;
use crate::world_masks::WorldMasks;
use crate::terrain::TerrainType;

/// WFC 地形生成の最終出力。hw_world → bevy_app 間のコントラクト。
/// すべてのフィールドは Bevy 依存なし（`#[derive(Resource)]` は bevy_app 側で newtype する）。
#[derive(Debug, Clone)]
pub struct GeneratedWorldLayout {
    // ── 地形 ────────────────────────────────────
    /// MAP_WIDTH × MAP_HEIGHT, row-major (y * MAP_WIDTH + x)
    pub terrain_tiles: Vec<TerrainType>,

    // ── 固定アンカー ──────────────────────────────
    pub anchors: AnchorLayout,

    // ── 診断用中間結果 ────────────────────────────
    pub masks: WorldMasks,

    // ── 資源配置候補（validator 到達確認済み） ──────
    pub resource_spawn_candidates: ResourceSpawnCandidates,

    // ── 木 ──────────────────────────────────────
    /// procedural 配置された初期木座標
    pub initial_tree_positions: Vec<GridPos>,
    /// 木の再生エリア定義（regrowth システムが参照する）
    pub forest_regrowth_zones: Vec<WfcForestZone>,

    // ── 岩 ──────────────────────────────────────
    /// procedural 配置された初期岩座標
    pub initial_rock_positions: Vec<GridPos>,

    // ── メタ ─────────────────────────────────────
    pub master_seed: u64,
    /// 何回目の試行（0-indexed）で収束したか
    pub generation_attempt: u32,
    /// MAX_WFC_RETRIES 後に deterministic fallback へ入ったか
    pub used_fallback: bool,
}

impl GeneratedWorldLayout {
    /// MS-WFC-2 実装前のスタブ。現行の固定地形を terrain_tiles に入れ、
    /// anchors と masks だけ正しく設定して返す。
    pub fn stub(master_seed: u64) -> Self {
        use crate::mapgen::generate_base_terrain_tiles;
        use hw_core::constants::{MAP_WIDTH, MAP_HEIGHT};
        use crate::layout::SAND_WIDTH;

        let anchors = AnchorLayout::fixed();
        let masks = WorldMasks::from_anchor(&anchors);

        GeneratedWorldLayout {
            terrain_tiles: generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, SAND_WIDTH),
            anchors,
            masks,
            resource_spawn_candidates: ResourceSpawnCandidates::default(),
            initial_tree_positions: Vec::new(),
            forest_regrowth_zones: Vec::new(),
            initial_rock_positions: Vec::new(),
            master_seed,
            generation_attempt: 0,
            used_fallback: false,
        }
    }
}

/// validator 到達確認済みの資源位置
#[derive(Debug, Clone, Default)]
pub struct ResourceSpawnCandidates {
    /// Yard から到達可能な River タイル
    pub water_tiles: Vec<GridPos>,
    /// Yard から到達可能な Sand タイル
    pub sand_tiles: Vec<GridPos>,
    /// 岩オブジェクトの候補座標（procedural 配置前）
    pub rock_candidates: Vec<GridPos>,
}

/// WFC 生成用の森林ゾーン定義（center + radius、手続き的に使う）。
/// 形状は MS0 の初期提案に合わせ、チェビシェフ距離ベースの正方形 zone で固定する。
///
/// # 既存型との関係
/// `hw_world::regrowth::ForestZone` は `{ min, max, initial_count, tree_positions }` の
/// ボックス形状で固定座標を持つ旧型。MS-WFC-3 でこちらに統一し、名称も `ForestZone` に戻す。
#[derive(Debug, Clone)]
pub struct WfcForestZone {
    pub center: GridPos,
    pub radius: u32,
    // 将来: density_weight, age_category など
}

impl WfcForestZone {
    pub fn contains(&self, pos: GridPos) -> bool {
        let dx = (pos.0 - self.center.0).abs();
        let dy = (pos.1 - self.center.1).abs();
        // チェビシェフ距離（正方形 zone）。MS-WFC-1 で geometry を固定する。
        dx <= self.radius as i32 && dy <= self.radius as i32
    }
}
```

**注意（MS-WFC-0 との関係）**

- `GeneratedWorldLayout::stub` は現行 `generate_base_terrain_tiles` を載せるだけなので、**MS-WFC-0 の lightweight 到達 invariant**（Yard から水源・砂源・岩まで歩行可能など）を**満たすとは限らない**（川・障害の配置が旧ロジックのまま）。その検証は **WFC 地形・資源パイプラインが入る MS-WFC-2 以降**で行う。
- `resource_spawn_candidates` / `initial_tree_positions` / `initial_rock_positions` の中身は **MS-WFC-2 / MS-WFC-3** で埋める。MS-WFC-1 のスタブでは **空のままでよい**。

---

### 4.4 `crates/hw_world/src/mapgen.rs` への `types` 追加

現在 `mapgen.rs` はフラットファイルだが、`src/mapgen.rs` を維持したまま
`src/mapgen/types.rs` を追加できる。**この MS ではリネームは必須ではない**。

```
crates/hw_world/src/mapgen.rs        (既存、維持)
crates/hw_world/src/mapgen/types.rs  (新規)
```

**`crates/hw_world/src/mapgen.rs`** — 既存の `generate_base_terrain_tiles` をそのまま維持し、以下を追加:

```rust
/// `crates/hw_world/src/lib.rs` から `pub use mapgen::types::{...}` するには **`pub mod types`** が必須。
/// `mod types` のみだと親モジュール経由の再エクスポートが煩雑になるため、本計画では `pub mod types` を採用する。
pub mod types;

// 既存（変更なし）
pub fn generate_base_terrain_tiles(
    map_width: i32,
    map_height: i32,
    sand_width: i32,
) -> Vec<TerrainType> { /* ... 変更なし */ }

// 新規追加（実装は MS-WFC-2b で行う）
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    types::GeneratedWorldLayout::stub(master_seed)
}
```

> **補足**: `src/mapgen.rs` と `src/mapgen/types.rs` の共存は Rust のモジュール解決で問題ない。`mapgen/mod.rs` への移動は、`wfc_adapter.rs` や `validate.rs` のような子モジュールが増えてから判断してよい。

---

### 4.5 `crates/hw_world/src/lib.rs` への追加

```rust
pub mod anchor;
pub mod world_masks;
// mapgen 内は pub use で必要なものだけ公開
pub use anchor::{AnchorLayout, GridRect};
pub use world_masks::{BitGrid, WorldMasks};
pub use mapgen::generate_world_layout;
pub use mapgen::types::{
    GeneratedWorldLayout, ResourceSpawnCandidates, WfcForestZone,
};
```

---

### 4.6 廃止コメント追加（この MS で実施、削除は MS-WFC-4）

**`crates/hw_world/src/layout.rs`**:
```rust
/// 初期配置の木材アイテムの座標リスト
/// # Deprecated
/// WFC 移行後は `AnchorLayout::fixed().initial_wood_positions` を使用すること。
/// この定数は MS-WFC-4 で削除される（全座標が Site 内に誤配置されている）。
pub const INITIAL_WOOD_POSITIONS: &[(i32, i32)] = &[...]; // 変更なし

/// 全岩の座標リスト
/// # Deprecated (一部座標)
/// クラスター1 (75-79, y:50-54) の 25 点は Yard 内 (x:70-89, y:40-59) に存在し、
/// WFC 移行後の invariant「Yard 内に岩を生成しない」と矛盾する。
/// WFC 移行後は `GeneratedWorldLayout::initial_rock_positions` を使用すること。
/// この定数は MS-WFC-4 で削除される。
pub const ROCK_POSITIONS: &[(i32, i32)] = &[...]; // 変更なし

/// 全木の座標リスト
/// # Deprecated (一部座標)
/// (45,55), (55,45), (65,55), (38,58), (42,42) は Site 内、(70,45) は Yard 内に存在し、
/// WFC 移行後の invariant「Site/Yard 内に木を生成しない」と矛盾する。
/// WFC 移行後は `GeneratedWorldLayout::initial_tree_positions` を使用すること。
/// この定数は MS-WFC-4 で削除される。
pub const TREE_POSITIONS: &[(i32, i32)] = &[...]; // 変更なし
```

**`crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`**:
```rust
// TODO(MS-WFC-4): AnchorLayout::fixed().wheelbarrow_parking に置き換える。
// (58, 58) は Site 内 (x:30-69, y:40-59) に位置しており Yard 内固定条件と矛盾する。
const INITIAL_WHEELBARROW_PARKING_GRID: (i32, i32) = (58, 58);
```

---

## 5. 実装手順（推奨順序）

1. `anchor.rs` を新規作成し、`GridRect` と `AnchorLayout::fixed()` を実装する
2. テスト (`anchor::tests`) を通す（`cargo test -p hw_world anchor`）
3. `world_masks.rs` を新規作成し、`BitGrid` と `WorldMasks::from_anchor()` を実装する
4. `mapgen/types.rs` を新規作成し、`GeneratedWorldLayout::stub()` を実装する
5. `mapgen.rs` に `pub mod types;` と `generate_world_layout()` スタブを追加する
6. `lib.rs` に pub export を追加する
7. `bevy_app::initial_spawn::layout.rs` の `compute_site_yard_layout()` を `AnchorLayout::try_fixed()` 由来へ寄せる
8. `layout.rs` に `INITIAL_WOOD_POSITIONS` / `ROCK_POSITIONS` / `TREE_POSITIONS` の廃止コメントを追加する
9. `initial_spawn/mod.rs` に `INITIAL_WHEELBARROW_PARKING_GRID` の廃止コメントを追加する
10. `cargo check --workspace` と `cargo clippy --workspace` を通す

---

## 6. 変更ファイル一覧

| ファイル | 変更種別 | 内容 |
| --- | --- | --- |
| `crates/hw_world/src/anchor.rs` | **新規** | `GridRect` / `AnchorLayout` / `AnchorLayout::fixed()` / テスト |
| `crates/hw_world/src/world_masks.rs` | **新規** | `BitGrid` / `WorldMasks` / `WorldMasks::from_anchor()` / `combined_protection_band()` |
| `crates/hw_world/src/mapgen.rs` | **変更** | `pub mod types;` と `generate_world_layout()` スタブを追加 |
| `crates/hw_world/src/mapgen/types.rs` | **新規** | `GeneratedWorldLayout` / `ResourceSpawnCandidates` / `WfcForestZone` / `used_fallback` |
| `crates/hw_world/src/lib.rs` | **変更** | `anchor` / `world_masks` を pub に追加、`generate_world_layout` / `GeneratedWorldLayout` 等を pub use |
| `crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs` | **変更** | `compute_site_yard_layout()` を `AnchorLayout::try_fixed()` ベースへ変更、整合テスト追加 |
| `crates/hw_world/src/layout.rs` | **変更** | `INITIAL_WOOD_POSITIONS` / `ROCK_POSITIONS` / `TREE_POSITIONS` に廃止コメント追加 |
| `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs` | **変更** | `INITIAL_WHEELBARROW_PARKING_GRID` に廃止コメント追加 |

---

## 7. 完了条件チェックリスト

- [ ] `AnchorLayout::fixed()` が Site/Yard 固定領域を pure に返す
- [ ] `anchor::tests` が通る（wood が Yard 内、parking footprint が Yard 内）
- [ ] `AnchorLayout` が parking の 2×2 footprint を型で表現している
- [ ] `WorldMasks::from_anchor()` が site/yard/anchor マスクを正しく設定する
- [ ] `WorldMasks` が River/岩/高密度木の保護帯を別 field で保持できる
- [ ] `GeneratedWorldLayout::stub()` が `cargo check` を通る
- [ ] `GeneratedWorldLayout` が `used_fallback` を保持している
- [ ] `compute_site_yard_layout()` が `AnchorLayout::try_fixed()` を単一ソースとして使っている
- [ ] `generate_world_layout()` のシグネチャが `lib.rs` から pub に公開されている
- [ ] `WfcForestZone` が定義され、既存 `ForestZone` との関係と geometry がコメントで明記されている
- [ ] `INITIAL_WOOD_POSITIONS` に廃止コメントが入っている
- [ ] `ROCK_POSITIONS` に廃止コメント（Yard 内 25 点の問題を明記）が入っている
- [ ] `TREE_POSITIONS` に廃止コメント（Site/Yard 内 6 点の問題を明記）が入っている
- [ ] `INITIAL_WHEELBARROW_PARKING_GRID` に廃止コメント（誤配置の説明含む）が入っている
- [ ] `mapgen.rs` から `mapgen::types` が正しく解決できる
- [ ] `cargo check --workspace` がゼロエラー
- [ ] `cargo clippy --workspace` がゼロ警告

---

## 8. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world anchor
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace
```

---

## 9. 注意事項・落とし穴

### `SITE_WIDTH_TILES` は `f32`

`hw_core::constants` の `SITE_WIDTH_TILES` 等は `f32` で定義されている。
`anchor.rs` 内で `as i32` キャストを集中管理し、小数点以下の切り捨てが意図通りであることを確認する。
（現在も `compute_site_yard_layout()` で `as i32` しているので挙動は同じ）
`MAP_WIDTH` / `MAP_HEIGHT` は `i32` なので `BitGrid::new(MAP_WIDTH, MAP_HEIGHT)` はそのまま使える。

### `mapgen.rs` と `mapgen/types.rs` の共存

`src/mapgen.rs` の中で `pub mod types;` を宣言すれば、
`src/mapgen/types.rs` はその子モジュールとして解決できる。
この MS で `mapgen/mod.rs` へ移動する必然性はない。

`mapgen/mod.rs` への移行は、`wfc_adapter.rs` や `validate.rs` のような
追加子モジュールが増えて、フラットファイル維持より見通しが悪くなった時点で判断する。

### `BitGrid` の `BitOrAssign` / `combined_protection_band`

`combined_protection_band` で `|=` する各 `BitGrid` は **同一 `width` / `height` / `data.len()`** であること。`debug_assert_eq!` で検知する前提（異なるサイズのマスクを合成しない）。

### `Site` / `Yard` 矩形の単一ソース

§3.4 を参照。`compute_site_yard_layout()` は `AnchorLayout::try_fixed()` の戻り値を変換するだけにし、
矩形計算ロジック自体を再実装しない。

### `ROCK_POSITIONS` クラスター1 と将来の wheelbarrow_parking の非干渉

提案中の `wheelbarrow_parking: (82, 52)-(83, 53)` と、現在の `ROCK_POSITIONS` クラスター1 `(75-79, 50-54)` は **直接重複しない**（x=75-79 と x=82-83 は別範囲）。
ただし、WFC 移行前の現行コードでは Yard 内に岩が存在し続ける。
MS-WFC-4 で `ROCK_POSITIONS` を廃止するまでは、startup 時に Yard 内の岩がスポーンされることに注意。

### `bevy_app` 側は壊さない

この MS で `bevy_app` に加える変更はコメント追加のみ。
`INITIAL_WHEELBARROW_PARKING_GRID` や `INITIAL_WOOD_POSITIONS` を削除しない。
startup システムは MS-WFC-4 まで旧パスのまま動く必要がある。

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
| `2026-04-02` | `Copilot` | コードベース調査（layout.rs / regrowth.rs / initial_spawn）に基づき全面ブラッシュアップ。具体的な座標値、型規約、ForestZone 命名衝突対処、mapgen 分割タイミング、実装手順を追加 |
| `2026-04-04` | `Codex` | レビュー反映。`used_fallback` 追加、parking footprint の `GridRect` 化、保護帯の要素別分離、`WfcForestZone` geometry の確定、完了条件更新 |
| `2026-04-05` | `Copilot` | コードベース再調査。`ROCK_POSITIONS` Yard 内25点問題・`TREE_POSITIONS` Site/Yard 内6点問題を §2.2 に追加。§3.2廃止予定・§4.6廃止コメント・§5手順・§6ファイル一覧・§7完了条件・§9注意事項を更新。`MAP_WIDTH/HEIGHT` が `i32` であることを明記。 |
| `2026-04-05` | `Codex` | レビュー反映。`mapgen.rs`→`mapgen/mod.rs` 必須という誤りを修正し、`mapgen.rs` + `mapgen/types.rs` 共存前提へ戻した。MS0/親計画と合わせて `combined_protection_band()` が `protection_band` 契約に相当することを明記。 |
| `2026-04-05` | `Codex` | 実装同期。`AnchorLayout::try_fixed()` / `AnchorLayoutError` を追加し、`compute_site_yard_layout()` を `hw_world` 側の単一ソースへ寄せた。 |
| `2026-03-29` | — | レビュー反映: §3.4 単一ソース、`stub` と MS-0 到達・空フィールドの範囲、§4.2 保護帯と MS-0 参照、§4.4 `pub mod types` 理由、§9 BitGrid 合成・二重定義 |
