# MS-WFC-2c: 生成後バリデータ（lightweight + debug）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2c-validator` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
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

---

## 2. モジュール構成

```
crates/hw_world/src/mapgen/
├── validate.rs          ← 本 MS で新規作成
│   ├── lightweight_validate()
│   └── debug_validate()        (#[cfg(debug_assertions)])
└── ...
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
- layout.anchors.site_rect の全セルが River / Sand でないことを確認
- layout.anchors.yard_rect の全セルが River / Sand でないことを確認
```

#### `check_site_yard_reachable`

```
- Site の代表セル（中心等）から Yard の代表セルへ walkable 経路が存在するか
- hw_world::pathfinding の walkable 判定（現行と同一の関数）を使う
- 実装: BFS / flood fill でよい（A* 不要）
```

#### `check_required_resources_reachable`

```
- Yard の代表セルから以下それぞれへの到達可能性を確認:
  - 水源: layout.masks.river_mask のいずれか 1 セル
  - 砂源: terrain_tiles に Sand が 1 セル以上あり、Yard から到達可能
  - 岩源: layout.resource_spawn_candidates.rock_positions のいずれか 1 個
            (MS-WFC-3 で設定される。この MS 時点では空なら SKIP でよい)
- 「最低 1 つへ到達可能」を満たせば OK
```

#### `check_yard_anchors_present`

```
- layout.anchors.initial_wood_grid の全座標が Yard 内に存在する
- layout.anchors.wheelbarrow_parking_grid が Yard 内に存在する
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
- layout.masks.protection_band の各セルを確認
- River / 岩 オブジェクトが保護帯内に存在しないことを警告
```

#### `check_sand_river_adjacency_ratio`

```
- Sand タイル総数のうち、河川タイルに辺接するものの割合を計算
- 80% を下回ったら ValidationWarning を追加
```

#### `check_river_tile_count`

```
- river_mask のセル数が RIVER_TOTAL_TILES_TARGET の許容範囲内かを確認
```

#### `check_no_fallback_reached`

```
- layout.generation_attempt > MAX_WFC_RETRIES なら「fallback に到達した」として warn
- debug_assert!(false, ...) も発火させる
```

#### `check_forbidden_diagonal_patterns`

```
- 2×2 以上の禁止パターン（例: River の孤立点、Dirt の孤立点）を検出
- F2: 斜め整合は WFC 後の validator で扱う方針
```

---

## 5. ValidationError / ValidationWarning 型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Site/Yard contains River or Sand at {0:?}")]
    ForbiddenTileInAnchorZone(IVec2),
    #[error("Site to Yard is not reachable")]
    SiteYardNotReachable,
    #[error("No required resource reachable from Yard")]
    RequiredResourceNotReachable,
    #[error("Yard anchor not in Yard bounds: {0:?}")]
    YardAnchorOutOfBounds(IVec2),
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
    RiverTileCountOutOfRange,
    FallbackReached,
    ForbiddenPattern,
}
```

`thiserror` は既存 crate の依存確認が必要。なければ手動 `impl std::fmt::Display`。

---

## 6. generate_world_layout への統合

```rust
pub fn generate_world_layout(master_seed: u64) -> GeneratedWorldLayout {
    // ... (MS-WFC-2b で実装済み)

    let layout = GeneratedWorldLayout { ... };

    // lightweight validate: 失敗は retry 判断に使う（startup で panic）
    if let Err(e) = lightweight_validate(&layout) {
        // retry ループの外ではここに来ないはずだが念のため panic
        panic!("Generated world failed lightweight validation: {e}");
    }

    #[cfg(debug_assertions)]
    {
        let warnings = debug_validate(&layout);
        for w in &warnings {
            log::warn!("[WFC debug] {:?}: {}", w.kind, w.message);
        }
    }

    layout
}
```

---

## 7. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/validate.rs` (新規) | `lightweight_validate` / `debug_validate` / ValidationError / ValidationWarning |
| `crates/hw_world/src/mapgen/mod.rs` | `validate` モジュール追加 |
| `crates/hw_world/src/mapgen.rs` | `generate_world_layout()` に validate 呼び出し組み込み |
| `crates/hw_world/Cargo.toml` | `thiserror` 追加（未追加の場合） |

---

## 8. 完了条件チェックリスト

- [ ] `lightweight_validate()` が 4 チェックを実装している
- [ ] `debug_validate()` が 5 チェック以上を実装している（`#[cfg(debug_assertions)]`）
- [ ] `ValidationError` / `ValidationWarning` が定義されている
- [ ] Site/Yard 内に River/Sand がある場合、`lightweight_validate` が Err を返す
- [ ] Site ↔ Yard が非連結の場合、`lightweight_validate` が Err を返す
- [ ] `generate_world_layout()` が lightweight_validate を通過した layout のみを返す
- [ ] fallback に到達した場合、`debug_validate` が警告を出す
- [ ] `cargo test -p hw_world` の golden seed テストが全て通る
- [ ] `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 9. テスト

```rust
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
    // Site の最初のセルを River に書き換える
    let site_pos = layout.anchors.site_rect.min;
    let idx = site_pos.y as usize * MAP_WIDTH + site_pos.x as usize;
    layout.terrain_tiles[idx] = TerrainType::River;
    assert!(lightweight_validate(&layout).is_err());
}
```

---

## 10. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
cargo check --workspace
cargo clippy --workspace
```

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
