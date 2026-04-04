# MS-WFC-2c: 生成後バリデータ（lightweight + debug）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2c-validator` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2b-wfc-solver-constraints.md`](wfc-ms2b-wfc-solver-constraints.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `generate_world_layout()` が地形グリッドを返せる（MS-WFC-2b 完了） |

---

## 1. 目的

WFC で生成された地形が **ゲーム上の invariant** を満たしていることを、**コードで検証する**。

- MS-WFC-0 で設計した 2 段 validator（lightweight / debug）を実装する
- `hw_world::pathfinding` と同一の walkable 判定を使い、到達可能性を確認する
- validator は `GeneratedWorldLayout` を受け取る pure 関数として実装し、startup との結合を最小にする
- この MS で **生成後局所修正** は行わない（validator が失敗したら retry または fallback）
- `Sand` の斜め-only River 接触は、WFC 本体ではなく debug validator で診断する

---

## 2. モジュール構成

本リポジトリでは `mapgen` は **`src/mapgen.rs`** が親モジュールで、サブモジュールは **`src/mapgen/*.rs`** に置く（`src/mapgen/mod.rs` は使わない）。

```
crates/hw_world/src/
├── mapgen.rs                 ← `pub mod validate;` を追加
└── mapgen/
    ├── types.rs
    ├── wfc_adapter.rs
    └── validate.rs           ← 本 MS で新規作成
        ├── lightweight_validate()
        └── debug_validate()   (#[cfg(debug_assertions)])
```

---

## 3. lightweight_validate()

```rust
/// 起動時必須チェック。失敗時は Err を返す（呼び出し側が panic するか retry に誘導する）。
pub fn lightweight_validate(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    check_site_yard_no_river_sand(layout)?;
    check_site_yard_reachable(layout)?;
    check_required_resources_reachable(layout)?;
    check_yard_anchors_present(layout)?;
    Ok(())
}
```

### チェック関数の責務

#### `check_site_yard_no_river_sand`

```
- layout.anchors.site（GridRect）の全セルが River / Sand でないことを確認
- layout.anchors.yard（GridRect）の全セルが River / Sand でないことを確認
- 走査は GridRect::iter_cells() と terrain_tiles の row-major インデックスで対応付ける
```

#### `check_site_yard_reachable`

```
- Site の 1 代表セルから Yard の 1 代表セルへ walkable 経路が存在するか
  （連結性チェックなので、代表点は固定でよい。例: site の min 角 1 点、yard の min 角 1 点、
   または各矩形の「中心に最も近い整数セル」1 点）
- hw_world::pathfinding の walkable 判定（現行と同一の関数）を使う
- 実装: BFS / flood fill でよい（A* 不要）
```

#### `check_required_resources_reachable`

```
- Yard の代表セルから以下それぞれへの到達可能性を確認:
  - 水源: layout.masks.river_mask のいずれか 1 セル
  - 砂源: terrain_tiles に Sand が 1 セル以上あり、Yard から到達可能
  - 岩源: layout.resource_spawn_candidates.rock_candidates のいずれか 1 座標
            (MS-WFC-3 で設定される。この MS 時点では Vec が空なら SKIP でよい)
- 「最低 1 つへ到達可能」を満たせば OK
```

#### `check_yard_anchors_present`

```
- layout.anchors.initial_wood_positions の全座標が layout.anchors.yard（GridRect）内にある
- layout.anchors.wheelbarrow_parking（GridRect）が layout.anchors.yard に包含される
  （猫車置き場は Yard 内 2×2 想定。ms1 データモデルに合わせる）
```

---

## 4. debug_validate()

```rust
/// 開発時のみ有効な追加診断。
/// #[cfg(debug_assertions)] または cargo test のみで実行する。
#[cfg(debug_assertions)]
pub fn debug_validate(layout: &GeneratedWorldLayout) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    check_protection_band_clean(layout, &mut warnings);
    check_sand_river_adjacency_ratio(layout, &mut warnings);
    check_sand_diagonal_only_contacts(layout, &mut warnings);
    check_river_tile_count(layout, &mut warnings);
    check_no_fallback_reached(layout, &mut warnings);
    // 斜め・2×2 禁止パターンのチェック（F2）
    check_forbidden_diagonal_patterns(layout, &mut warnings);
    warnings
}
```

### debug チェックの内容

#### `check_protection_band_clean`

```
- WorldMasks に単一フィールド protection_band はない。
  debug レポート相当の合成は combined_protection_band()、要素別は
  river_protection_band / rock_protection_band / tree_dense_protection_band（wfc-ms0 §3.1.1）
- 検査内容（ms0）: 禁止対象（River タイル・岩占有・高密度木）が各種保護帯「内」に入っていないこと
- MS-WFC-2c 時点で procedural 岩・木が未配置なら、terrain 上で判定できる River などに絞ってよい
```

#### `check_sand_river_adjacency_ratio`

```
- Sand タイル総数のうち、河川タイルに辺接するものの割合を計算
- 80% を下回ったら ValidationWarning を追加
```

#### `check_sand_diagonal_only_contacts`

```
- Sand セルごとに、River との 4 近傍接触と斜め 4 マス接触を別々に数える
- 「River に斜めでは接しているが、4 近傍では接していない」Sand を
  diagonal-only 接触として扱う
- diagonal-only 接触は初版では **warning** 扱い
  - error にはしない
  - retry 必須条件にも含めない
- 理由:
  - 親計画 F2 は斜め整合を validator 側で扱う方針
  - 親計画 F4 は「砂の 8 割程度が River に辺接」を目標としており、
    斜め-only は見た目品質の診断対象だが、ただちに不正マップとはみなさない
- 初期閾値:
  - 0 件が理想
  - 総 Sand 数に対して 10% 超なら warning を追加
```

#### `check_river_tile_count`

```
- river_mask のセル数が、river.rs の RIVER_TOTAL_TILES_TARGET_MIN / MAX（または計画で置いた目安）の範囲内か
```

#### `check_no_fallback_reached`

```
- layout.used_fallback == true なら「WFC fallback に到達した」として ValidationWarning を追加
- MS-WFC-2b ではフォールバック時に debug でパニックしない方針のため、
  **debug_assert!(false, ...) は使わない**（警告・ログのみ）
```

#### `check_forbidden_diagonal_patterns`

```
- 2×2 以上の禁止パターン（例: River の孤立点、Dirt の孤立点）を検出
- F2: 斜め整合は WFC 後の validator で扱う方針
- Sand の diagonal-only 接触はここでまとめず、`check_sand_diagonal_only_contacts` に分離する
```

---

## 5. ValidationError / ValidationWarning 型

座標は **`hw_core::world::GridPos`（`(i32, i32)`）** を使う。`glam::IVec2` はマップ生成コードと揃える必要がなければ導入しない。

```rust
use hw_core::world::GridPos;

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Site/Yard contains River or Sand at {0:?}")]
    ForbiddenTileInAnchorZone(GridPos),
    #[error("Site to Yard is not reachable")]
    SiteYardNotReachable,
    #[error("No required resource reachable from Yard")]
    RequiredResourceNotReachable,
    #[error("Yard anchor not in Yard bounds: {0:?}")]
    YardAnchorOutOfBounds(GridPos),
}

#[derive(Debug)]
pub struct ValidationWarning {
    pub kind: ValidationWarningKind,
    pub message: String,
}

#[derive(Debug)]
pub enum ValidationWarningKind {
    ProtectionBandViolation,
    SandRiverAdjacencyLow,
    SandDiagonalOnlyContact,
    RiverTileCountOutOfRange,
    FallbackReached,
    ForbiddenPattern,
}
```

`thiserror` は `hw_world/Cargo.toml` に未追加なら workspace 経由で追加するか、手動 `impl std::fmt::Display` とする。

---

## 6. generate_world_layout への統合

`hw_world` に **`log` crate は現状ない**。debug 診断の出力は **`eprintln!`** でよい（または Bevy 利用箇所のみ `tracing` するなら別計画）。下記は `eprintln` 例。

```rust
pub fn generate_world_layout(master_seed: u64) -> GeneratedWorldLayout {
    // ... (MS-WFC-2b で実装済み)

    let layout = GeneratedWorldLayout { ... };

    // lightweight validate: 失敗は retry 判断に使う（startup で panic）
    if let Err(e) = lightweight_validate(&layout) {
        panic!("Generated world failed lightweight validation: {e}");
    }

    #[cfg(debug_assertions)]
    {
        let warnings = debug_validate(&layout);
        for w in &warnings {
            eprintln!("[WFC debug] {:?}: {}", w.kind, w.message);
        }
    }

    layout
}
```

---

## 7. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/validate.rs` (新規) | `lightweight_validate` / `debug_validate` / `ValidationError` / `ValidationWarning` |
| `crates/hw_world/src/mapgen.rs` | `pub mod validate;` を追加し、`generate_world_layout()` に validate 呼び出しを組み込み |
| `crates/hw_world/Cargo.toml` | `thiserror` を追加する場合のみ（手動 `Display` なら不要） |

---

## 8. Golden seed 定数（テスト用）

[wfc-ms0-invariant-spec.md](wfc-ms0-invariant-spec.md) §3.0 の方針（4 本: `STANDARD`, `WINDING_RIVER`, `TIGHT_BAND`, `RETRY`）に合わせ、`mapgen` の `#[cfg(test)]` または `validate.rs` のテスト用モジュールに **`u64` 定数**を定義する。初版の具体値は実装時に決定し、**`lightweight_validate` が通ること**を CI で保証する。

例（名前のみ・値はプレースホルダ）:

```rust
pub const GOLDEN_SEED_STANDARD: u64 = 42;
pub const GOLDEN_SEED_WINDING_RIVER: u64 = 0;      // 実装時に確定
pub const GOLDEN_SEED_TIGHT_BAND: u64 = 0;
// RETRY 用 seed は必要に応じて追加
```

---

## 9. 完了条件チェックリスト

- [ ] `lightweight_validate()` が 4 チェックを実装している
- [ ] `debug_validate()` が 5 チェック以上を実装している（`#[cfg(debug_assertions)]`）
- [ ] `ValidationError` / `ValidationWarning` が定義されている
- [ ] Site/Yard 内に River/Sand がある場合、`lightweight_validate` が Err を返す
- [ ] Site ↔ Yard が非連結の場合、`lightweight_validate` が Err を返す
- [ ] `generate_world_layout()` が lightweight_validate を通過した layout のみを返す
- [ ] fallback に到達した場合（`used_fallback == true`）、`debug_validate` が警告を出す
- [ ] Sand の diagonal-only River 接触が多すぎる場合、`debug_validate` が warning を出す
- [ ] `cargo test -p hw_world` の golden seed テストが全て通る
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 10. テスト

```rust
use crate::anchor::GridRect;
use crate::terrain::TerrainType;
use hw_core::constants::MAP_WIDTH;

#[test]
fn test_golden_seeds_pass_lightweight_validate() {
    for seed in [GOLDEN_SEED_STANDARD, GOLDEN_SEED_WINDING_RIVER, GOLDEN_SEED_TIGHT_BAND] {
        let layout = generate_world_layout(seed);
        assert!(lightweight_validate(&layout).is_ok(), "seed={seed}");
    }
}

#[test]
fn test_fake_invalid_layout_fails_validate() {
    let mut layout = generate_world_layout(GOLDEN_SEED_STANDARD);
    // Site の一角（例: min 角）を River に書き換える
    let site: &GridRect = &layout.anchors.site;
    let x = site.min_x;
    let y = site.min_y;
    let idx = (y * MAP_WIDTH + x) as usize;
    layout.terrain_tiles[idx] = TerrainType::River;
    assert!(lightweight_validate(&layout).is_err());
}
```

`GridRect` は `crate::anchor::GridRect` をテストで import する。

---

## 11. 検証コマンド

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

---

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
| `2026-04-04` | — | レビュー反映: `AnchorLayout` / `ResourceSpawnCandidates` の実フィールド名、`mapgen.rs`+`mapgen/validate.rs` 構成、保護帯は `combined_protection_band` 等、`used_fallback` と `debug_assert` 非使用、`GridPos`・`eprintln`・検証コマンドの `CARGO_HOME` 統一、golden seed 節・テストの `GridRect` 修正。 |
