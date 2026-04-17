//! ビジュアル関連のプラグイン

use crate::entities::familiar::{familiar_animation_system, update_familiar_range_indicator};
use crate::plugins::startup::{
    Camera3dRtt, Camera3dSoulMaskRtt, RttCompositeSprite, RttDirectionalLight,
    RttExtraDirectionalLight,
};
use crate::systems::GameSystemSet;
use crate::systems::command::{
    area_edit_handles_visual_system, area_selection_indicator_system,
    dream_tree_planting_preview_system, sync_designation_indicator_system,
    update_designation_indicator_system,
};
use crate::systems::jobs::building_completion_system;
use crate::systems::logistics::resource_count_display_system;
use crate::systems::visual::building3d_cleanup::{
    cleanup_building_3d_visuals_system, sync_provisional_wall_material_system,
};
use crate::systems::visual::camera_sync::{
    sync_camera3d_system, sync_world_foreground_2d_camera_system,
};
use crate::systems::visual::character_proxy_3d::{
    apply_soul_gltf_render_layers_on_ready, apply_soul_mask_gltf_render_layers_on_ready,
    apply_soul_shadow_gltf_render_layers_on_ready, cleanup_familiar_proxy_3d_system,
    cleanup_soul_mask_proxy_3d_system, cleanup_soul_proxy_3d_system,
    cleanup_soul_shadow_proxy_3d_system, register_familiar_proxy_3d_system,
    register_soul_mask_proxy_3d_system, register_soul_proxy_3d_system,
    register_soul_shadow_proxy_3d_system, sync_familiar_proxy_3d_system,
    sync_soul_mask_proxy_3d_system, sync_soul_proxy_3d_system, sync_soul_shadow_proxy_3d_system,
};
use crate::systems::visual::elevation_view::{ElevationViewState, elevation_view_input_system};
use crate::systems::visual::section_cut::sync_section_cut_normal_system;
use crate::systems::visual::soul_animation::{
    SoulAnimationLibrary, initialize_soul_animation_players_system,
    prepare_soul_animation_library_system, sync_soul_anim_visual_state_system,
    sync_soul_body_animation_system, sync_soul_face_expression_system,
};
use crate::systems::visual::soul_shadow_projector::sync_soul_shadow_projectors_system;
use crate::systems::visual::task_area_visual::update_task_area_material_system;
use crate::systems::visual::terrain_lod::{
    TerrainLodMetrics, TerrainLodState, terrain_lod_switch_system,
    update_terrain_lod_metrics_system,
};
use crate::systems::visual::terrain_material::terrain_id_map_sync_system;
use crate::world::map::TerrainChunk;
use hw_core::game_state::PlayMode;
use hw_visual::HwVisualPlugin;
use hw_visual::SectionCut;
use hw_visual::soul::task_link_system;
use hw_visual::SoulProxyOwnerCache;
use hw_visual::visual3d::{Building3dVisual, FamiliarProxy3d, SoulProxy3d};
use hw_world::{TerrainChangedEvent, sync_room_overlay_tiles_system};

use bevy::prelude::*;

type MainRttCameraQuery<'w, 's> = Query<'w, 's, &'static mut Camera, With<Camera3dRtt>>;
type SoulMaskRttCameraQuery<'w, 's> =
    Query<'w, 's, &'static mut Camera, (With<Camera3dSoulMaskRtt>, Without<Camera3dRtt>)>;
type RttDirectionalLightQuery<'w, 's> =
    Query<'w, 's, &'static mut DirectionalLight, With<RttDirectionalLight>>;
type RttExtraDirectionalLightQuery<'w, 's> =
    Query<'w, 's, &'static mut DirectionalLight, With<RttExtraDirectionalLight>>;

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HwVisualPlugin);

        app.init_resource::<ElevationViewState>();
        app.init_resource::<SectionCut>();
        app.init_resource::<SoulAnimationLibrary>();
        app.init_resource::<SoulProxyOwnerCache>();
        app.init_resource::<TerrainLodMetrics>();
        app.init_resource::<TerrainLodState>();

        app.add_message::<TerrainChangedEvent>();

        app.add_systems(
            Update,
            (sync_camera3d_system, sync_world_foreground_2d_camera_system)
                .chain()
                .in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            update_terrain_lod_metrics_system
                .after(sync_camera3d_system)
                .in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            terrain_lod_switch_system
                .after(update_terrain_lod_metrics_system)
                .in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            sync_section_cut_normal_system.in_set(GameSystemSet::Visual),
        );

        app.add_systems(
            Update,
            sync_room_overlay_tiles_system.in_set(GameSystemSet::Visual),
        );

        // Area indicators (app_contexts 依存のため root 残留)
        app.add_systems(
            Update,
            (
                crate::systems::command::task_area_indicator_system,
                area_edit_handles_visual_system,
                crate::systems::command::designation_visual_system,
                crate::systems::command::familiar_command_visual_system,
                crate::systems::visual::placement_ghost::placement_ghost_system,
            )
                .in_set(GameSystemSet::Visual)
                .run_if(|state: Res<State<hw_core::game_state::PlayMode>>| {
                    matches!(
                        state.get(),
                        PlayMode::Normal | PlayMode::BuildingPlace | PlayMode::TaskDesignation
                    )
                }),
        );

        app.add_systems(
            Update,
            dream_tree_planting_preview_system.in_set(GameSystemSet::Visual),
        );

        // task_link は DebugVisible（root 専有リソース）で条件付き実行
        app.add_systems(
            Update,
            task_link_system
                .run_if(|debug: Res<crate::DebugVisible>| debug.0)
                .in_set(GameSystemSet::Visual),
        );

        // root 残留の visual systems（jobs / logistics / soul_ai / familiar 由来）
        app.add_systems(
            Update,
            (
                building_completion_system,
                area_selection_indicator_system.run_if(|play_mode: Res<State<PlayMode>>| {
                    matches!(
                        play_mode.get(),
                        PlayMode::TaskDesignation | PlayMode::FloorPlace
                    )
                }),
                update_designation_indicator_system,
                sync_designation_indicator_system,
                resource_count_display_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        app.add_systems(
            Update,
            (familiar_animation_system, update_familiar_range_indicator)
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // task area visual（root 残留型に依存）
        app.add_systems(
            Update,
            update_task_area_material_system.in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            sync_soul_shadow_projectors_system
                .after(sync_camera3d_system)
                .in_set(GameSystemSet::Visual),
        );

        // Building3dVisual クリーンアップ・マテリアル遷移
        app.add_systems(
            Update,
            (
                cleanup_building_3d_visuals_system,
                sync_provisional_wall_material_system,
            )
                .in_set(GameSystemSet::Visual),
        );

        // terrain id map 更新（障害物除去後）
        app.add_systems(
            Update,
            terrain_id_map_sync_system.in_set(GameSystemSet::Visual),
        );

        // キャラクター3Dプロキシ同期・クリーンアップ
        app.add_systems(
            Update,
            (
                sync_soul_proxy_3d_system,
                sync_soul_mask_proxy_3d_system,
                sync_soul_shadow_proxy_3d_system,
                sync_familiar_proxy_3d_system,
                prepare_soul_animation_library_system,
                (
                    sync_soul_anim_visual_state_system,
                    initialize_soul_animation_players_system,
                    sync_soul_body_animation_system,
                    sync_soul_face_expression_system,
                )
                    .chain(),
                cleanup_soul_proxy_3d_system,
                cleanup_soul_mask_proxy_3d_system,
                cleanup_soul_shadow_proxy_3d_system,
                cleanup_familiar_proxy_3d_system,
                register_soul_proxy_3d_system,
                register_soul_mask_proxy_3d_system,
                register_soul_shadow_proxy_3d_system,
                register_familiar_proxy_3d_system,
            )
                .in_set(GameSystemSet::Visual),
        );

        // 矢視モード入力
        app.add_systems(
            Update,
            elevation_view_input_system.in_set(GameSystemSet::Input),
        );

        // Render3dVisible の変更を Camera3dRtt と RttCompositeSprite に反映
        app.add_systems(
            Update,
            apply_render3d_visibility_system.in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            apply_rtt_directional_light_toggle_system.in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            apply_rtt_extra_directional_light_toggle_system.in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            apply_rtt_scene_content_toggle_system.in_set(GameSystemSet::Visual),
        );
        app.add_observer(apply_soul_gltf_render_layers_on_ready);
        app.add_observer(apply_soul_mask_gltf_render_layers_on_ready);
        app.add_observer(apply_soul_shadow_gltf_render_layers_on_ready);
    }
}

/// Render3dVisible の変更を Camera3dRtt と RttCompositeSprite の可視性に反映する
fn apply_render3d_visibility_system(
    render3d: Res<crate::Render3dVisible>,
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_main_camera: MainRttCameraQuery,
    mut q_soul_mask_camera: SoulMaskRttCameraQuery,
    mut q_sprite: Query<&mut Visibility, With<RttCompositeSprite>>,
) {
    if !render3d.is_changed() && !perf_toggles.is_changed() {
        return;
    }

    for mut camera in &mut q_main_camera {
        camera.is_active = render3d.0;
    }
    for mut camera in &mut q_soul_mask_camera {
        camera.is_active = render3d.0 && perf_toggles.soul_mask_enabled;
    }
    if let Ok(mut visibility) = q_sprite.single_mut() {
        *visibility = if render3d.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// RtT 用 DirectionalLight の固定費を個別比較できるようにする。
fn apply_rtt_directional_light_toggle_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_lights: RttDirectionalLightQuery,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for mut light in &mut q_lights {
        light.shadows_enabled = perf_toggles.directional_light_enabled;
        light.illuminance = if perf_toggles.directional_light_enabled {
            12_000.0
        } else {
            0.0
        };
    }
}

/// 追加テスト用 DirectionalLight の ON/OFF を反映する。
fn apply_rtt_extra_directional_light_toggle_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_lights: RttExtraDirectionalLightQuery,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for mut light in &mut q_lights {
        light.shadows_enabled = perf_toggles.extra_directional_light_enabled;
        light.illuminance = if perf_toggles.extra_directional_light_enabled {
            8_000.0
        } else {
            0.0
        };
    }
}

type SceneObjectQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    Or<(With<Building3dVisual>, With<SoulProxy3d>, With<FamiliarProxy3d>)>,
>;

/// 地形と main scene object を個別に隠して、RtT 固定費の内訳を切り分ける。
fn apply_rtt_scene_content_toggle_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    q_terrain: Query<Entity, With<TerrainChunk>>,
    q_scene_objects: SceneObjectQuery,
    mut commands: Commands,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    let terrain_visibility = if perf_toggles.terrain_enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for entity in &q_terrain {
        commands.entity(entity).insert(terrain_visibility);
    }

    let scene_object_visibility = if perf_toggles.scene_objects_enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for entity in &q_scene_objects {
        commands.entity(entity).insert(scene_object_visibility);
    }
}
