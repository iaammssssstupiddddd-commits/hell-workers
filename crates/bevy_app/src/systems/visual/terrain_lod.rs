//! 地形 LOD 観測基盤と状態管理
//!
//! ## 設計
//! - `TerrainLodMetrics`: 毎フレーム更新される観測値（`tile_rtt_px` / `tile_screen_px`）
//! - `TerrainLodState`: LOD 切替状態（`level` / `applied_level`）
//!   - `metric` の変化が毎フレーム `state.is_changed()` を立てるのを防ぐため、2 つを分離する。
//!   - 切替システムは `level != applied_level` の時のみ実際の差し替えを行う。
//! - `tile_rtt_px` が LOD 判定の正本。`tile_screen_px` はデバッグ補助用。
//! - 矢視（Elevation）時は LOD2 を禁止する（`resolve_lod_level` で保証）。

use crate::plugins::startup::{Camera3dRtt, Camera3dSoulMaskRtt, RttRuntime, composite_logical_size};
use crate::systems::visual::elevation_view::{ElevationDirection, ElevationViewState};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::TILE_SIZE;

// ── 型エイリアス ──────────────────────────────────────────────────────────────

type Rtt3dCameraQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Camera, &'static GlobalTransform),
    (With<Camera3dRtt>, Without<Camera3dSoulMaskRtt>),
>;

// ── LOD レベル定義 ────────────────────────────────────────────────────────────

/// 地形 LOD レベル。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LodLevel {
    /// 近景: 現行 `TerrainSurfaceMaterial` をそのまま使用。
    #[default]
    Lod0,
    /// 中景〜遠景: 49 chunk 維持、shader の重い経路を省略。
    Lod1,
    /// TopDown far のみ: chunk を隠し overview/impostor を表示（条件付き導入）。
    Lod2,
}

// ── hysteresis 閾値（tile_rtt_px 基準） ──────────────────────────────────────

/// LOD0 → LOD1 に遷移する RtT タイルサイズ閾値（px）。
pub const LOD0_TO_LOD1_ENTER_PX: f32 = 10.0;
/// LOD1 → LOD0 に復帰する RtT タイルサイズ閾値（px）。
pub const LOD1_TO_LOD0_EXIT_PX: f32 = 14.0;
/// LOD1 → LOD2 に遷移する RtT タイルサイズ閾値（px）。TopDown far のみ発動。
pub const LOD1_TO_LOD2_ENTER_PX: f32 = 4.0;
/// LOD2 → LOD1 に復帰する RtT タイルサイズ閾値（px）。
pub const LOD2_TO_LOD1_EXIT_PX: f32 = 6.0;

// ── Resources ────────────────────────────────────────────────────────────────

/// 毎フレーム更新される地形 LOD 観測値。
///
/// LOD 判定は `TerrainLodState` が持ち、こちらは metric 専用にする。
/// これにより metric 更新だけで `TerrainLodState::is_changed()` が立つのを防ぐ。
#[derive(Resource, Default)]
pub struct TerrainLodMetrics {
    /// RtT 上での 1 タイル見かけサイズ（px）。LOD 判定の正本。
    pub tile_rtt_px: f32,
    /// スクリーン表示上での 1 タイル見かけサイズ（px）。デバッグ表示用。
    pub tile_screen_px: f32,
}

/// 地形 LOD の切替状態。
///
/// `level`: `update_terrain_lod_metrics_system` が更新する目標 LOD。
/// `applied_level`: 切替システムが最後に適用したときの LOD。
/// `level != applied_level` の時だけ切替システムが実際の差し替えを行う。
#[derive(Resource, Default)]
pub struct TerrainLodState {
    pub level: LodLevel,
    pub applied_level: LodLevel,
}

// ── LOD 更新システム ──────────────────────────────────────────────────────────

/// `Camera3dRtt` の `world_to_viewport` で `tile_rtt_px` を算出し、
/// `TerrainLodMetrics` と `TerrainLodState.level` を更新する。
///
/// このシステムは `sync_camera3d_system` の後に実行される必要がある（投影が更新済みであるため）。
pub fn update_terrain_lod_metrics_system(
    q_cam3d: Rtt3dCameraQuery,
    runtime: Res<RttRuntime>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut metrics: ResMut<TerrainLodMetrics>,
    mut state: ResMut<TerrainLodState>,
    elevation: Res<ElevationViewState>,
) {
    let Ok((cam, gtf)) = q_cam3d.single() else {
        return;
    };

    // map center を基準点に（カメラ位置依存にしない）
    let p = Vec3::ZERO;
    let px = p + Vec3::new(TILE_SIZE, 0.0, 0.0);
    let pz = p + Vec3::new(0.0, 0.0, TILE_SIZE);

    let to_vp = |world: Vec3| -> Option<Vec2> { cam.world_to_viewport(gtf, world).ok() };

    if let (Some(v0), Some(vx), Some(vz)) = (to_vp(p), to_vp(px), to_vp(pz)) {
        let dx = (vx - v0).length();
        let dz = (vz - v0).length();
        // East/West 矢視で world X 辺が視線方向へ潰れるため、2 軸の大きい方を採用する。
        metrics.tile_rtt_px = dx.max(dz);
    }

    // tile_screen_px: tile_rtt_px × composite 表示倍率（デバッグ補助用）
    if let Ok(window) = q_window.single() {
        let logical = composite_logical_size(window);
        let screen_scale_x = logical.x / runtime.viewport.width.max(1) as f32;
        let screen_scale_y = logical.y / runtime.viewport.height.max(1) as f32;
        metrics.tile_screen_px = metrics.tile_rtt_px * screen_scale_x.max(screen_scale_y);
    }

    // LOD 状態遷移（hysteresis）
    let new_level = resolve_lod_level(state.level, metrics.tile_rtt_px, elevation.direction);
    if new_level != state.level {
        state.level = new_level;
    }
}

/// hysteresis 付き LOD 遷移ロジック。矢視中は LOD2 を禁止する。
pub fn resolve_lod_level(
    current: LodLevel,
    tile_rtt_px: f32,
    direction: ElevationDirection,
) -> LodLevel {
    match current {
        LodLevel::Lod0 => {
            if tile_rtt_px < LOD0_TO_LOD1_ENTER_PX {
                LodLevel::Lod1
            } else {
                LodLevel::Lod0
            }
        }
        LodLevel::Lod1 => {
            if tile_rtt_px > LOD1_TO_LOD0_EXIT_PX {
                LodLevel::Lod0
            } else if direction.is_top_down() && tile_rtt_px < LOD1_TO_LOD2_ENTER_PX {
                LodLevel::Lod2
            } else {
                LodLevel::Lod1
            }
        }
        LodLevel::Lod2 => {
            if !direction.is_top_down() || tile_rtt_px > LOD2_TO_LOD1_EXIT_PX {
                LodLevel::Lod1
            } else {
                LodLevel::Lod2
            }
        }
    }
}

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod0_enters_lod1_when_small() {
        let result = resolve_lod_level(LodLevel::Lod0, 9.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1);
    }

    #[test]
    fn lod0_stays_when_large() {
        let result = resolve_lod_level(LodLevel::Lod0, 15.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod0);
    }

    #[test]
    fn lod1_exits_to_lod0_when_large() {
        let result = resolve_lod_level(LodLevel::Lod1, 15.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod0);
    }

    #[test]
    fn lod1_enters_lod2_topdown_only() {
        let result = resolve_lod_level(LodLevel::Lod1, 3.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod2);
    }

    #[test]
    fn lod1_does_not_enter_lod2_in_elevation() {
        for dir in [
            ElevationDirection::North,
            ElevationDirection::East,
            ElevationDirection::South,
            ElevationDirection::West,
        ] {
            let result = resolve_lod_level(LodLevel::Lod1, 3.0, dir);
            assert_eq!(
                result,
                LodLevel::Lod1,
                "LOD2 should be forbidden in {:?}",
                dir
            );
        }
    }

    #[test]
    fn lod2_exits_to_lod1_when_large() {
        let result = resolve_lod_level(LodLevel::Lod2, 7.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1);
    }

    #[test]
    fn lod2_exits_immediately_when_leaving_topdown() {
        for dir in [
            ElevationDirection::North,
            ElevationDirection::East,
            ElevationDirection::South,
            ElevationDirection::West,
        ] {
            let result = resolve_lod_level(LodLevel::Lod2, 1.0, dir);
            assert_eq!(
                result,
                LodLevel::Lod1,
                "LOD2 should be forbidden in {:?}",
                dir
            );
        }
    }

    #[test]
    fn hysteresis_lod0_does_not_exit_at_enter_threshold() {
        // LOD0→LOD1 の enter は 10 px。LOD1→LOD0 の exit は 14 px。
        // LOD1 で 11 px（enter < 11 < exit）のとき LOD1 を維持する。
        let result = resolve_lod_level(LodLevel::Lod1, 11.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1);
    }
}
