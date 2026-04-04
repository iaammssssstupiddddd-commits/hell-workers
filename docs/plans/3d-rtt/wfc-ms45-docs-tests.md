# MS-WFC-4.5: ドキュメントと検証整備

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms45-docs-tests` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms4-startup-integration.md`](wfc-ms4-startup-integration.md) |
| 前提 | MS-WFC-0〜4 が完了し、WFC 地形生成が実動している |

---

## 1. 目的

実装完了後の **品質担保と保守性の確立**。

- `docs/world_layout.md` を「固定アンカー付き自動生成」仕様へ更新する
- golden seeds を使った回帰確認の運用フローを確立する
- debug レポート出力を整備する
- WFC 関連の全変更が `milestone-roadmap.md` と `ms-3-6-terrain-surface-plan-2026-03-31.md` と矛盾しないよう整合を確認する
- この MS 完了をもって `wfc-terrain-generation-plan-2026-04-01.md` のステータスを `Done` に更新する

---

## 2. ドキュメント更新

### 2.1 `docs/world_layout.md`

更新方針:

| 項目 | Before | After |
| --- | --- | --- |
| 地形生成 | 固定パターン（`(x+y)%30==0` 等） | WFC 自動生成（seed ベース） |
| 木・岩 | 固定座標テーブル（廃止済み） | procedural 配置（ForestZone 参照） |
| 初期木材 | 固定絶対座標 | Yard 内固定オフセット |
| 猫車置き場 | `(58, 58)` 絶対座標（廃止済み） | Yard 内固定オフセット |
| 不変条件 | 記述なし | lightweight validator の 4 チェックを列挙 |
| 再生エリア | 記述なし | `ForestZone` データ構造と `regrowth` システムとの関係を記述 |

記述すべき境界の明確化:
- **固定（変更不可）**: `Site` 位置・`Yard` 位置・Yard 内木材オフセット・Yard 内猫車置き場オフセット
- **自動生成（seed 依存）**: 地形・木・岩・川・砂の分布
- **禁止**: Site/Yard 内への River/Sand/木/岩

### 2.2 `docs/plans/3d-rtt/milestone-roadmap.md`

- 並行トラック B（WFC 地形生成）のステータスを `Done` に更新
- サブ計画ファイル群（wfc-ms0〜ms4.5）への参照を追加

### 2.3 `docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md`

- WFC 完了後に S0（受入基準スクリーンショット）が撮影可能になったことを記録
- 隣接ブレンド（B 方針）の実施判断を更新

---

## 3. golden seeds 回帰テストの確立

### 3.1 テストファイル

`crates/hw_world/src/mapgen/tests/golden_seeds.rs`（新規または既存 tests ファイルに追加）

```rust
#[cfg(test)]
mod golden_seed_tests {
    use super::*;
    use crate::golden_seeds::*;  // 定数定義モジュール

    #[test]
    fn golden_seed_standard_passes_validate() {
        let layout = generate_world_layout(GOLDEN_SEED_STANDARD);
        assert!(lightweight_validate(&layout).is_ok());
    }

    #[test]
    fn golden_seed_winding_river_passes_validate() {
        let layout = generate_world_layout(GOLDEN_SEED_WINDING_RIVER);
        assert!(lightweight_validate(&layout).is_ok());
    }

    #[test]
    fn golden_seed_tight_band_passes_validate() {
        let layout = generate_world_layout(GOLDEN_SEED_TIGHT_BAND);
        assert!(lightweight_validate(&layout).is_ok());
    }

    #[test]
    fn all_golden_seeds_are_deterministic() {
        for seed in [GOLDEN_SEED_STANDARD, GOLDEN_SEED_WINDING_RIVER, GOLDEN_SEED_TIGHT_BAND] {
            let a = generate_world_layout(seed);
            let b = generate_world_layout(seed);
            assert_eq!(a.terrain_tiles, b.terrain_tiles, "non-deterministic at seed={seed}");
        }
    }
}
```

### 3.2 golden seeds 定数ファイル

`crates/hw_world/src/golden_seeds.rs`（新規）

```rust
/// ゲームプレイに近い標準状態
pub const GOLDEN_SEED_STANDARD: u64 = 12345;
/// 川が大きく曲がり砂帯が広いケース
pub const GOLDEN_SEED_WINDING_RIVER: u64 = 99999;
/// 保護帯ぎりぎりに資源が生成されるケース
pub const GOLDEN_SEED_TIGHT_BAND: u64 = 42042;
```

> 実際の seed 値は最初に `generate_world_layout()` が動いた段階で調整する。
> 各 seed の「意図する特性」を満たすことを目視で確認してから定数化する。

---

## 4. debug レポート出力の整備

### 4.1 実装

`crates/hw_world/src/mapgen/debug_report.rs`（新規）

```rust
/// debug レポートを端末に ASCII ダンプする
pub fn print_ascii_report(layout: &GeneratedWorldLayout) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let ch = match layout.terrain_tiles[y * MAP_WIDTH + x] {
                TerrainType::Grass => '.',
                TerrainType::Dirt  => 'D',
                TerrainType::River => '~',
                TerrainType::Sand  => 's',
            };
            // Site/Yard/保護帯を上書き表示
            let ch = if layout.masks.site_mask.get(y * MAP_WIDTH + x) { 'S' }
                     else if layout.masks.yard_mask.get(y * MAP_WIDTH + x) { 'Y' }
                     else if layout.masks.protection_band.get(y * MAP_WIDTH + x) { 'P' }
                     else { ch };
            print!("{ch}");
        }
        println!();
    }
}

/// PNG カラーマップを target/debug_reports/ に書き出す（dev ビルドのみ）
#[cfg(debug_assertions)]
pub fn save_png_report(layout: &GeneratedWorldLayout, master_seed: u64) { ... }
```

### 4.2 有効化トリガー

- CLI 引数 `--debug-worldgen` または環境変数 `HELL_WORKERS_DEBUG_WORLDGEN=1` で有効化
- `cargo test -p hw_world -- --nocapture` でも出力確認可能

---

## 5. 整合性確認チェックリスト

### RtT 整合（`ms-3-6-terrain-surface-plan-2026-03-31.md` との確認）

- [ ] WFC が生成する Dirt タイルが孤立点でなく連続領域として現れることを確認（B 方針の前提）
- [ ] WFC の隣接制約により、境界のジグザグが自然な形になっていることを確認
- [ ] `SectionMaterial` 側の想定（`TerrainType` の種類と隣接パターン）と WFC 生成結果が矛盾しないことを確認

### milestone-roadmap との整合

- [ ] 並行トラック B の完了条件がすべてチェックされている
- [ ] 後続 MS への影響（RtT 側の S0 撮影可能状態）が記録されている

---

## 6. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `docs/world_layout.md` | 自動生成前提へ全面更新（§2.1 参照） |
| `docs/plans/3d-rtt/milestone-roadmap.md` | トラック B 完了・サブ計画参照追加 |
| `docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md` | S0 撮影可能状態の記録・B 方針更新 |
| `crates/hw_world/src/golden_seeds.rs` (新規) | golden seed 定数 |
| `crates/hw_world/src/mapgen/tests/golden_seeds.rs` (新規) | 回帰テスト群 |
| `crates/hw_world/src/mapgen/debug_report.rs` (新規) | ASCII ダンプ / PNG レポート |
| `crates/hw_world/src/mapgen/mod.rs` | `debug_report` モジュール追加 |
| `docs/plans/3d-rtt/wfc-terrain-generation-plan-2026-04-01.md` | ステータスを `Done` に更新 |

---

## 7. 完了条件チェックリスト

- [ ] `docs/world_layout.md` が自動生成前提に更新されている
- [ ] `Site/Yard` / Yard 内固定オブジェクト / 自動生成対象の境界が docs に明記されている
- [ ] 全 golden seeds が `cargo test -p hw_world` を通過する
- [ ] debug レポート（ASCII ダンプ）が `--debug-worldgen` で出力される
- [ ] `milestone-roadmap.md` の並行トラック B が Done になっている
- [ ] `ms-3-6-terrain-surface-plan-2026-03-31.md` との整合が確認されている
- [ ] `wfc-terrain-generation-plan-2026-04-01.md` のステータスが `Done` になっている
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 8. Definition of Done（プロジェクト全体）

この MS 完了をもって、以下がすべて満たされていることを確認する:

- [ ] WFC が地形自動生成の中核として実装されている
- [ ] `Site/Yard` と Yard 内固定オブジェクトの制約が守られている
- [ ] 木・岩が procedural 配置に置き換わっている
- [ ] 木の再生可能エリア（ForestZone）が生成結果と整合している
- [ ] golden seeds と生成レポートを含む回帰運用が成立している
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が成功している

---

## 9. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
cargo check --workspace
cargo clippy --workspace
```

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
