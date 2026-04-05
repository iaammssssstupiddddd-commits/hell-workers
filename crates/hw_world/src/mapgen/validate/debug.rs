//! Debug diagnostic validators — compiled only in test/debug builds.

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

use crate::mapgen::types::GeneratedWorldLayout;
use crate::mapgen::wfc_adapter::CARDINAL_DIRS;
use crate::river::{RIVER_TOTAL_TILES_TARGET_MAX, RIVER_TOTAL_TILES_TARGET_MIN};
use crate::terrain::TerrainType;

use super::{ValidationWarning, ValidationWarningKind};

pub fn debug_validate(layout: &GeneratedWorldLayout) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    check_protection_band_clean(layout, &mut warnings);
    check_river_tile_count(layout, &mut warnings);
    check_no_fallback_reached(layout, &mut warnings);
    check_forbidden_diagonal_patterns(layout, &mut warnings);
    check_final_sand_mask_applied(layout, &mut warnings);
    check_no_stray_sand_outside_mask(layout, &mut warnings);
    check_sand_mask_not_in_anchor_or_band(layout, &mut warnings);
    warnings
}

/// River タイルがアンカー外周保護帯（river_protection_band）に侵入していないか確認する。
fn check_protection_band_clean(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.river_mask.get((x, y)) && layout.masks.river_protection_band.get((x, y))
            {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::ProtectionBandViolation,
                    message: format!("River at ({x},{y}) is inside river_protection_band"),
                });
            }
        }
    }
}

/// river_mask のセル数が RIVER_TOTAL_TILES_TARGET_MIN/MAX の範囲外なら警告する。
fn check_river_tile_count(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    let count = layout.masks.river_mask.count_set();
    if !(RIVER_TOTAL_TILES_TARGET_MIN..=RIVER_TOTAL_TILES_TARGET_MAX).contains(&count) {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::RiverTileCountOutOfRange,
            message: format!(
                "River tile count {count} outside [{RIVER_TOTAL_TILES_TARGET_MIN}, {RIVER_TOTAL_TILES_TARGET_MAX}]"
            ),
        });
    }
}

/// fallback 地形が使われた場合に警告する（debug_assert! は使わない）。
fn check_no_fallback_reached(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    if layout.used_fallback {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::FallbackReached,
            message: format!(
                "WFC fallback terrain used (master_seed={}, attempt={})",
                layout.master_seed, layout.generation_attempt
            ),
        });
    }
}

/// 4 近傍に River 隣接がない孤立 River タイルを検出する（F2 斜め整合診断の一部）。
fn check_forbidden_diagonal_patterns(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if layout.terrain_tiles[idx] != TerrainType::River {
                continue;
            }
            let has_river_neighbor = CARDINAL_DIRS.iter().any(|(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                    return false;
                }
                layout.terrain_tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::River
            });
            if !has_river_neighbor {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::ForbiddenPattern,
                    message: format!(
                        "Isolated River tile (no cardinal River neighbor) at ({x},{y})"
                    ),
                });
            }
        }
    }
}

/// final_sand_mask == true のセルがすべて TerrainType::Sand になっているか確認する。
fn check_final_sand_mask_applied(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.final_sand_mask.get((x, y)) {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] != TerrainType::Sand {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::SandMaskMismatch,
                        message: format!(
                            "final_sand_mask=true but terrain != Sand at ({x},{y}): {:?}",
                            layout.terrain_tiles[idx]
                        ),
                    });
                }
            }
        }
    }
}

/// final_sand_mask または inland_sand_mask の外に TerrainType::Sand が残っていないか確認する。
fn check_no_stray_sand_outside_mask(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            // final_sand_mask と inland_sand_mask の合法領域を合わせて判定
            let in_legal_sand =
                layout.masks.final_sand_mask.get(pos) || layout.masks.inland_sand_mask.get(pos);
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

/// final_sand_mask が anchor_mask / river_protection_band と交差しないか確認する。
fn check_sand_mask_not_in_anchor_or_band(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !layout.masks.final_sand_mask.get((x, y)) {
                continue;
            }
            if layout.masks.anchor_mask.get((x, y)) {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::SandMaskMismatch,
                    message: format!("final_sand_mask overlaps anchor_mask at ({x},{y})"),
                });
            }
            if layout.masks.river_protection_band.get((x, y)) {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::SandMaskMismatch,
                    message: format!("final_sand_mask overlaps river_protection_band at ({x},{y})"),
                });
            }
        }
    }
}
