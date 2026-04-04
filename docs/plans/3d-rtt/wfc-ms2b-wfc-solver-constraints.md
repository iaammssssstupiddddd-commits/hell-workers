# MS-WFC-2b: WFC ソルバー統合と制約マスキング

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2b-wfc-solver-constraints` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2a-crate-adapter-river-mask.md`](wfc-ms2a-crate-adapter-river-mask.md) |
| 次MS | [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md) |
| 前提 | アダプタ骨格・川マスク生成が完成（MS-WFC-2a 完了） |

---

## 1. 目的

`run_wfc()` の `todo!()` を実装し、**実際に WFC ソルバーで地形グリッドを生成する**。

- hard constraint（川マスク・Site/Yard マスク）を WFC ソルバーに正しく渡す
- カーディナル 4 近傍の隣接ルールを適用して地形を収束させる
- 砂の重みを F4 方針で反映する
- 収束失敗時の **deterministic retry + fallback** を実装する
- 生成結果を `GeneratedWorldLayout::terrain_tiles` に格納する

---

## 2. 実装詳細

### 2.1 WFC ソルバーの呼び出しフロー

```
1. AnchorLayout::fixed() から Site/Yard マスクを取得
2. generate_river_mask() から川マスクを取得
3. HardConstraints::from_masks() でソルバー入力マスクを構築
4. AdjacencyRules::default_terrain_rules() で隣接ルールを構築
5. タイル重み（砂の F4 重みを含む）を設定
6. run_wfc(constraints, rules, sub_seed, attempt) を呼び出す
7. 失敗時は attempt += 1 して sub_seed を再導出し、最大 MAX_WFC_RETRIES まで繰り返す
8. 全試行失敗時は fallback_terrain() を呼び出す
9. 結果を TerrainType グリッドに変換して GeneratedWorldLayout へ格納
```

### 2.2 deterministic retry

```rust
/// master_seed と attempt から sub_seed を導出する（可逆・deterministic）
fn derive_sub_seed(master_seed: u64, attempt: u32) -> u64 {
    // 例: wyrand / splitmix64 の 1 ステップを使って分散させる
    master_seed.wrapping_add((attempt as u64).wrapping_mul(0x9e3779b97f4a7c15))
}
```

- `master_seed` は変更しない（F6 方針）
- `attempt` は 0 から始まり `MAX_WFC_RETRIES` まで（例: 64）

### 2.3 fallback

```rust
/// 失敗時の安全マップ。未決定セルを Grass で埋める。
fn fallback_terrain(
    constraints: &HardConstraints,
    mapping: &TerrainTileMapping,
) -> Vec<TerrainType> {
    // hard constraint（川・Site/Yard）はそのまま維持
    // それ以外のセルは全て Grass
}
```

`debug_assert!(false, "WFC fallback reached for master_seed={}", master_seed)` を fallback 冒頭に置く。

### 2.4 隣接ルール（AdjacencyRules の実装）

F2 方針: **カーディナル 4 近傍のみ**。斜め制約は生成後の validator（MS-WFC-2c）で対応。

| 許可する隣接 | 方向 |
| --- | --- |
| Grass ↔ Grass | 全方向 |
| Grass ↔ Dirt | 全方向 |
| Dirt ↔ Dirt | 全方向 |
| Dirt ↔ Sand | 全方向 |
| Sand ↔ Sand | 全方向 |
| River ↔ River | 全方向 |
| River ↔ Sand | 全方向 |
| River ↔ Grass | 全方向（River を Grass で囲む場合） |
| Grass ↔ Sand | 全方向（低頻度だが禁止しない） |

**禁止する隣接:**
- Site/Yard 内セルへの River / Sand（hard constraint で解決するため ルール不要だが念のためチェック）

### 2.5 タイル重み設定

```rust
pub struct TileWeights {
    pub grass: f32,   // 例: 5.0
    pub dirt: f32,    // 例: 2.0
    pub sand_adjacent_to_river: f32,      // SAND_ADJACENT_TO_RIVER_WEIGHT
    pub sand_non_adjacent: f32,           // SAND_NON_ADJACENT_WEIGHT
    pub river: f32,   // 0.0（hard constraint で固定、WFC ソルバーには渡さない）
}
```

重みは定数化し、調整しやすくする（`mapgen/weights.rs` 等）。

### 2.6 `generate_world_layout()` の実装

```rust
pub fn generate_world_layout(master_seed: u64) -> GeneratedWorldLayout {
    let anchors = AnchorLayout::fixed();
    let (river_mask, river_centerline) = generate_river_mask(master_seed, &anchors, ...);
    let masks = WorldMasks {
        site_mask: anchors.site_mask(),
        yard_mask: anchors.yard_mask(),
        anchor_mask: anchors.combined_mask(),
        protection_band: build_protection_band(&anchors, PROTECTION_BAND_WIDTH),
        river_mask,
        river_centerline,
    };
    let mapping = TerrainTileMapping::new();
    let rules = AdjacencyRules::default_terrain_rules();
    let constraints = HardConstraints::from_masks(&masks, &mapping);

    let (terrain_tiles, attempt) = (0..=MAX_WFC_RETRIES)
        .find_map(|attempt| {
            let sub_seed = derive_sub_seed(master_seed, attempt);
            run_wfc(&constraints, &rules, sub_seed, attempt)
                .ok()
                .map(|tiles| (tiles, attempt))
        })
        .unwrap_or_else(|| {
            debug_assert!(false, "WFC fallback for seed={}", master_seed);
            (fallback_terrain(&constraints, &mapping), MAX_WFC_RETRIES + 1)
        });

    GeneratedWorldLayout {
        terrain_tiles,
        anchors,
        masks,
        master_seed,
        generation_attempt: attempt,
        // resource_spawn_candidates / tree / rock は MS-WFC-3 で設定
        ..GeneratedWorldLayout::default()
    }
}
```

---

## 3. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | `run_wfc()` の実装・retry ロジック・fallback |
| `crates/hw_world/src/mapgen/weights.rs` (新規) | タイル重み定数・`TileWeights` 構造体 |
| `crates/hw_world/src/mapgen.rs` | `generate_world_layout()` の実装 |
| `crates/hw_world/src/mapgen/mod.rs` | `weights` モジュール追加 |

---

## 4. 完了条件チェックリスト

- [ ] `run_wfc()` が外部 WFC crate を呼び出して地形グリッドを生成する
- [ ] Site/Yard 内に River / Sand が生成されない
- [ ] 同一 master seed で同一マップが生成される（テストで確認）
- [ ] 別 master seed で River / Dirt / Sand の分布が変化する（テストで確認）
- [ ] fallback に到達した場合 `debug_assert` が発火する
- [ ] 同一 master seed で fallback を経ても最終マップが再現する
- [ ] タイル重みが定数化されて調整しやすい
- [ ] `MAX_WFC_RETRIES` が定数化されている
- [ ] `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 5. テスト

```rust
#[test]
fn test_wfc_determinism() {
    let seed = GOLDEN_SEED_STANDARD;
    let layout1 = generate_world_layout(seed);
    let layout2 = generate_world_layout(seed);
    assert_eq!(layout1.terrain_tiles, layout2.terrain_tiles);
}

#[test]
fn test_wfc_different_seeds_differ() {
    let layout_a = generate_world_layout(GOLDEN_SEED_STANDARD);
    let layout_b = generate_world_layout(GOLDEN_SEED_WINDING_RIVER);
    assert_ne!(layout_a.terrain_tiles, layout_b.terrain_tiles);
}

#[test]
fn test_site_yard_no_river_sand() {
    let layout = generate_world_layout(GOLDEN_SEED_STANDARD);
    for pos in layout.anchors.site_rect.iter() {
        let tile = layout.terrain_tiles[pos.y as usize * MAP_WIDTH + pos.x as usize];
        assert!(!matches!(tile, TerrainType::River | TerrainType::Sand));
    }
    // Yard も同様
}
```

---

## 6. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
cargo check --workspace
cargo clippy --workspace
```

---

## 7. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
