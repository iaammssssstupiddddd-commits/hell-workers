//! 地形 LOD 観測基盤と状態管理
//!
//! ## 設計
//! - `TerrainLodMetrics`: 毎フレーム更新される観測値（`tile_rtt_px` / `tile_screen_px`）
//! - `TerrainLodState`: LOD 切替状態（`level` / `applied_level`）
//!   - `metric` の変化が毎フレーム `state.is_changed()` を立てるのを防ぐため、2 つを分離する。
//!   - 切替システムは `level != applied_level` の時のみ実際の差し替えを行う。
//! - `tile_rtt_px` が LOD 判定の正本。`tile_screen_px` はデバッグ補助用。
//! - runtime は現状 `Lod1`（現行フル品質）/ `Lod1Lite`（中景簡略版）/ `Lod2`（軽量 variant）を使用する。
//! - `Lod0` は将来のリッチビジュアル実装用に予約し、現在の runtime では選択しない。

use crate::plugins::startup::Terrain3dHandles;
use crate::plugins::startup::{
    Camera3dRtt, Camera3dSoulMaskRtt, RttRuntime, composite_logical_size,
};
use crate::systems::visual::elevation_view::{ElevationDirection, ElevationViewState};
use crate::world::map::TerrainChunk;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::TILE_SIZE;
use hw_visual::{
    TerrainSurfaceMaterial, TerrainSurfaceMaterialLod1Lite, TerrainSurfaceMaterialLod2,
};

// ── 型エイリアス ──────────────────────────────────────────────────────────────

type Rtt3dCameraQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Camera, &'static GlobalTransform),
    (With<Camera3dRtt>, Without<Camera3dSoulMaskRtt>),
>;

type ChunkLod1Query<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<TerrainChunk>,
        With<MeshMaterial3d<TerrainSurfaceMaterial>>,
    ),
>;

type ChunkLod2Query<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<TerrainChunk>,
        With<MeshMaterial3d<TerrainSurfaceMaterialLod2>>,
    ),
>;

type ChunkLod1LiteQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<TerrainChunk>,
        With<MeshMaterial3d<TerrainSurfaceMaterialLod1Lite>>,
    ),
>;

// ── LOD レベル定義 ────────────────────────────────────────────────────────────

/// 地形 LOD レベル。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LodLevel {
    /// 将来のリッチビジュアル用に予約。現 runtime では未使用。
    Lod0,
    /// 現行フル品質。現在の近景〜通常表示で使用する。
    #[default]
    Lod1,
    /// 中景の簡略版。曲線境界は維持しつつ surface detail を落とす。
    Lod1Lite,
    /// 軽量 variant。現在の遠景表示で使用する。
    Lod2,
}

// ── hysteresis 閾値（tile_rtt_px 基準） ──────────────────────────────────────

/// LOD1 → LOD1Lite に遷移する RtT タイルサイズ閾値（px）。
pub const LOD1_TO_LOD1LITE_ENTER_PX: f32 = 22.0;
/// LOD1Lite → LOD1 に復帰する RtT タイルサイズ閾値（px）。
pub const LOD1LITE_TO_LOD1_EXIT_PX: f32 = 25.0;
/// LOD1Lite → LOD2 に遷移する RtT タイルサイズ閾値（px）。
pub const LOD1LITE_TO_LOD2_ENTER_PX: f32 = 14.0;
/// LOD2 → LOD1Lite に復帰する RtT タイルサイズ閾値（px）。
pub const LOD2_TO_LOD1LITE_EXIT_PX: f32 = 16.0;

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

/// hysteresis 付き LOD 遷移ロジック。
///
/// `Lod0` は将来用の予約スロットのため、現在の runtime では `Lod1` に寄せる。
pub fn resolve_lod_level(
    current: LodLevel,
    tile_rtt_px: f32,
    _direction: ElevationDirection,
) -> LodLevel {
    match current {
        LodLevel::Lod0 => LodLevel::Lod1,
        LodLevel::Lod1 => {
            if tile_rtt_px < LOD1_TO_LOD1LITE_ENTER_PX {
                LodLevel::Lod1Lite
            } else {
                LodLevel::Lod1
            }
        }
        LodLevel::Lod1Lite => {
            if tile_rtt_px > LOD1LITE_TO_LOD1_EXIT_PX {
                LodLevel::Lod1
            } else if tile_rtt_px < LOD1LITE_TO_LOD2_ENTER_PX {
                LodLevel::Lod2
            } else {
                LodLevel::Lod1Lite
            }
        }
        LodLevel::Lod2 => {
            if tile_rtt_px > LOD2_TO_LOD1LITE_EXIT_PX {
                LodLevel::Lod1Lite
            } else {
                LodLevel::Lod2
            }
        }
    }
}

// ── LOD 切替システム ──────────────────────────────────────────────────────────

/// `TerrainLodState.level != applied_level` の時に限り、49 chunk の
/// `MeshMaterial3d` を LOD1 / LOD1Lite / LOD2 間で差し替える。
pub fn terrain_lod_switch_system(
    mut commands: Commands,
    mut lod: ResMut<TerrainLodState>,
    handles: Res<Terrain3dHandles>,
    q_lod1: ChunkLod1Query,
    q_lod1_lite: ChunkLod1LiteQuery,
    q_lod2: ChunkLod2Query,
) {
    if lod.level == lod.applied_level {
        return;
    }
    match lod.level {
        LodLevel::Lod2 => {
            for entity in &q_lod1 {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterial>>()
                    .insert(MeshMaterial3d(handles.lod2.clone()));
            }
            for entity in &q_lod1_lite {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterialLod1Lite>>()
                    .insert(MeshMaterial3d(handles.lod2.clone()));
            }
        }
        LodLevel::Lod1Lite => {
            for entity in &q_lod1 {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterial>>()
                    .insert(MeshMaterial3d(handles.lod1_lite.clone()));
            }
            for entity in &q_lod2 {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterialLod2>>()
                    .insert(MeshMaterial3d(handles.lod1_lite.clone()));
            }
        }
        // LOD0 は未実装のため、当面は LOD1 material へフォールバックする。
        LodLevel::Lod0 | LodLevel::Lod1 => {
            for entity in &q_lod1_lite {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterialLod1Lite>>()
                    .insert(MeshMaterial3d(handles.lod1.clone()));
            }
            for entity in &q_lod2 {
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterialLod2>>()
                    .insert(MeshMaterial3d(handles.lod1.clone()));
            }
        }
    }
    lod.applied_level = lod.level;
}

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod0_falls_back_to_lod1_while_reserved() {
        let result = resolve_lod_level(LodLevel::Lod0, 20.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1);
    }

    #[test]
    fn lod1_enters_lod2_when_small() {
        let result = resolve_lod_level(LodLevel::Lod1, 13.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1Lite);
    }

    #[test]
    fn lod1_stays_when_large() {
        let result = resolve_lod_level(LodLevel::Lod1, 26.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1);
    }

    #[test]
    fn lod1lite_enters_lod2_when_small() {
        let result = resolve_lod_level(LodLevel::Lod1Lite, 13.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod2);
    }

    #[test]
    fn lod1lite_returns_to_lod1_when_large() {
        let result = resolve_lod_level(LodLevel::Lod1Lite, 26.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1);
    }

    #[test]
    fn lod1lite_stays_within_band() {
        let result = resolve_lod_level(LodLevel::Lod1Lite, 20.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1Lite);
    }

    #[test]
    fn lod2_exits_to_lod1lite_when_large() {
        let result = resolve_lod_level(LodLevel::Lod2, 17.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1Lite);
    }

    #[test]
    fn lod2_stays_when_within_hysteresis_band() {
        let result = resolve_lod_level(LodLevel::Lod2, 15.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod2);
    }

    #[test]
    fn lod2_is_allowed_in_elevation_views() {
        for dir in [
            ElevationDirection::North,
            ElevationDirection::East,
            ElevationDirection::South,
            ElevationDirection::West,
        ] {
            let result = resolve_lod_level(LodLevel::Lod1, 13.0, dir);
            assert_eq!(
                result,
                LodLevel::Lod1Lite,
                "LOD1Lite should remain available in {:?}",
                dir
            );
        }
    }

    #[test]
    fn hysteresis_lod2_does_not_exit_at_enter_threshold() {
        let result = resolve_lod_level(LodLevel::Lod2, 15.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod2);
    }

    #[test]
    fn hysteresis_lod1lite_does_not_exit_to_lod1_below_exit_threshold() {
        let result = resolve_lod_level(LodLevel::Lod1Lite, 24.0, ElevationDirection::TopDown);
        assert_eq!(result, LodLevel::Lod1Lite);
    }
}
