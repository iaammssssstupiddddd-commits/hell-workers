//! WFC 回帰・診断テスト用 seed 定数。本番ビルドには含めない。
//!
//! ## Seed 一覧
//!
//! | 定数名 | 値 | 用途 |
//! | --- | --- | --- |
//! | `GOLDEN_SEED_PRIMARY` | `10_182_272_928_891_625_829` | 主回帰（旧 `GOLDEN_SEED_STANDARD` / `TEST_SEED_A` と同値）。validate・resources・pipeline テストのほぼ全てで使用 |
//! | `GOLDEN_SEED_SECONDARY` | `12_345_678` | 異種確認（旧 `TEST_SEED_B`）。異なる地形が生成されることの検証 |
//! | `SEED_SUITE_ROCK_REGRESSION` | `[0, 42, 99, 12_345_678]` | `rock_fields` 代表スイート |
//! | `SEED_SUITE_DIAG_PRINT` | `[0, 42, 99, 12345]` | `tile_dist_sim` 診断プリント用（4 番目は `12345`・`ROCK_REGRESSION` と別値） |
//! | `TERRAIN_ZONE_DETERMINISM_SEED` | `12345` | `terrain_zones` の deterministic 確認で使う固定 seed |
//! | `SEED_SUITE_TERRAIN_ZONE_CANDIDATES` | `[42, 12345, 99, 0]` | `terrain_zones` の dirt ゾーン存在確認スイート |
//!
//! ## 注意
//! `SEED_SUITE_DIAG_PRINT` と `SEED_SUITE_ROCK_REGRESSION` は**値を揃えていない**。
//! 意図的に分けており、まとめると診断出力・分布が変わるため統合しないこと。

/// 主回帰 seed（旧 `GOLDEN_SEED_STANDARD` / `TEST_SEED_A` と同値）。
/// validate・resources・pipeline のほぼ全テストがこれを使う。
pub(crate) const GOLDEN_SEED_PRIMARY: u64 = 10_182_272_928_891_625_829;

/// 異種確認用 seed（旧 `TEST_SEED_B`）。
/// `GOLDEN_SEED_PRIMARY` と異なる地形が生成されることを保証する。
pub(crate) const GOLDEN_SEED_SECONDARY: u64 = 12_345_678;

/// `rock_fields` 回帰テスト用代表スイート。4 番目は `12_345_678`。
pub(crate) const SEED_SUITE_ROCK_REGRESSION: &[u64] = &[0, 42, 99, 12_345_678];

/// `tile_dist_sim` 診断プリント用スイート。4 番目は `12345`（`ROCK_REGRESSION` と別値）。
/// 分布の連続性を保つため値を変えないこと。
pub(crate) const SEED_SUITE_DIAG_PRINT: &[u64] = &[0, 42, 99, 12345];

/// `terrain_zones` の deterministic 確認に使う固定 seed。
pub(crate) const TERRAIN_ZONE_DETERMINISM_SEED: u64 = 12345;

/// `terrain_zones` の dirt ゾーン存在確認に使う候補スイート。
pub(crate) const SEED_SUITE_TERRAIN_ZONE_CANDIDATES: &[u64] = &[42, 12345, 99, 0];
