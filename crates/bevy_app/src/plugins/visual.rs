//! ビジュアル関連のプラグイン

use crate::entities::familiar::{familiar_animation_system, update_familiar_range_indicator};
use crate::plugins::startup::{
    Camera3dRtt, RttCompositeSprite, RttDirectionalLight, RttExtraDirectionalLight,
};
use crate::systems::GameSystemSet;
use crate::systems::command::{
    AreaEditHandleVisual, AreaSelectionIndicator, DesignationIndicator, DreamTreePreviewIndicator,
    TaskAreaIndicator, area_edit_handles_visual_system, area_selection_indicator_system,
    dream_tree_planting_preview_system, sync_designation_indicator_system,
    update_designation_indicator_system,
};
use crate::systems::lighting::IndoorLightingRebuildSet;
use crate::systems::logistics::resource_count_display_system;
use crate::systems::visual::actor_billboard::{
    cleanup_actor_billboard_system, register_actor_billboard_system, sync_actor_billboard_system,
};
use crate::systems::visual::building3d_cleanup::{
    DoorPresentationSyncSet, cleanup_building_3d_visuals_system, sync_building_3d_transform_system,
    sync_door_presentation_system, sync_provisional_wall_material_system,
    sync_structural_presentation_state_system,
};
use crate::systems::visual::camera_sync::sync_camera3d_system;
use crate::systems::visual::indoor_light_texture::{
    IndoorLightUploadSet, reset_indoor_light_texture_for_world_replace,
    upload_indoor_light_texture_system,
};
use crate::systems::visual::soul_animation::sync_soul_anim_visual_state_system;
use crate::systems::visual::task_area_visual::update_task_area_material_system;
use crate::systems::visual::terrain_lod::{
    TerrainLodMetrics, TerrainLodState, terrain_lod_switch_system,
    update_terrain_lod_metrics_system,
};
use crate::systems::visual::terrain_material::terrain_id_map_sync_system;
use crate::world::map::TerrainChunk;
use hw_core::game_state::PlayMode;
use hw_visual::ActorBillboardOwnerCache;
use hw_visual::HwVisualPlugin;
use hw_visual::soul::task_link_system;
use hw_visual::visual3d::{ActorBillboard3d, Building3dVisual};
use hw_world::{TerrainChangedEvent, sync_room_overlay_tiles_system};

use bevy::prelude::*;

type MainRttCameraQuery<'w, 's> = Query<'w, 's, &'static mut Camera, With<Camera3dRtt>>;
type RttDirectionalLightQuery<'w, 's> =
    Query<'w, 's, &'static mut DirectionalLight, With<RttDirectionalLight>>;
type RttExtraDirectionalLightQuery<'w, 's> =
    Query<'w, 's, &'static mut DirectionalLight, With<RttExtraDirectionalLight>>;

pub struct VisualPlugin;

fn configure_indoor_light_visual_schedule(app: &mut App) {
    app.configure_sets(
        Update,
        (
            IndoorLightUploadSet
                .in_set(GameSystemSet::Visual)
                .after(IndoorLightingRebuildSet),
            DoorPresentationSyncSet
                .in_set(GameSystemSet::Visual)
                .after(IndoorLightUploadSet),
        )
            .chain(),
    );
}

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        crate::systems::save::register_visual_rehydrate_pipeline(app);
        app.add_plugins(HwVisualPlugin);
        crate::systems::save::register_load_reset_hook(
            app,
            "hw-visual",
            hw_visual::reset_for_world_replace,
        );
        crate::systems::save::register_load_reset_hook(
            app,
            "root-command-visuals",
            reset_root_command_visuals,
        );
        crate::systems::save::register_load_reset_hook(
            app,
            "indoor-light-texture",
            reset_indoor_light_texture_for_world_replace,
        );

        app.init_resource::<ActorBillboardOwnerCache>();
        app.init_resource::<TerrainLodMetrics>();
        app.init_resource::<TerrainLodState>();

        app.add_message::<TerrainChangedEvent>();

        // Door mutations and the CPU Light Field are final before presentation.
        // Behavior observers remain after this stable consumer boundary.
        configure_indoor_light_visual_schedule(app);
        app.add_systems(
            Update,
            upload_indoor_light_texture_system.in_set(IndoorLightUploadSet),
        );

        app.add_systems(Update, sync_camera3d_system.in_set(GameSystemSet::Visual));
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
            sync_room_overlay_tiles_system.in_set(GameSystemSet::Visual),
        );

        // Area indicators (app_contexts 依存のため root 残留)
        app.add_systems(
            Update,
            hw_ui::selection::clear_live_placement_feedback_system
                .before(crate::systems::visual::placement_ghost::placement_ghost_system)
                .in_set(GameSystemSet::Visual),
        );

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

        // Building3dVisual クリーンアップ・マテリアル遷移
        app.add_systems(
            Update,
            (
                cleanup_building_3d_visuals_system,
                sync_provisional_wall_material_system,
                sync_building_3d_transform_system
                    .after(hw_visual::blueprint::building_bounce_animation_system),
                sync_structural_presentation_state_system,
            )
                .in_set(GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            sync_door_presentation_system
                .after(hw_spatial::door_auto_open_nearby_system)
                .after(hw_spatial::door_auto_close_nearby_system)
                .after(crate::systems::lighting::consume_door_lock_toggle_requests_system)
                .in_set(DoorPresentationSyncSet),
        );

        // terrain id map 更新（障害物除去後）
        app.add_systems(
            Update,
            terrain_id_map_sync_system.in_set(GameSystemSet::Visual),
        );

        // Shared-pool Soul billboard resolver/sync/lifecycle.
        app.add_systems(
            Update,
            (
                sync_soul_anim_visual_state_system,
                sync_actor_billboard_system
                    .after(sync_soul_anim_visual_state_system)
                    .run_if(render3d_sync_enabled),
                cleanup_actor_billboard_system,
                register_actor_billboard_system,
            )
                .in_set(GameSystemSet::Visual),
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
    }
}

/// Removes root-owned transient command visuals before their target world is
/// replaced. `DesignationIndicator` cannot rely on its normal
/// `RemovedComponents<Designation>` cleanup because the replacement boundary
/// intentionally discards old removal buffers before the next frame.
fn reset_root_command_visuals(world: &mut World) {
    let transient_entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, Or<(
            With<DesignationIndicator>,
            With<TaskAreaIndicator>,
            With<AreaEditHandleVisual>,
            With<AreaSelectionIndicator>,
            With<DreamTreePreviewIndicator>,
        )>>();
        query.iter(world).collect()
    };

    for entity in transient_entities {
        world.despawn(entity);
    }
}

fn render3d_sync_enabled(render3d: Res<crate::Render3dVisible>) -> bool {
    render3d.0
}

/// Render3dVisible の変更を Camera3dRtt と RttCompositeSprite の可視性に反映する
fn apply_render3d_visibility_system(
    render3d: Res<crate::Render3dVisible>,
    mut q_main_camera: MainRttCameraQuery,
    mut q_sprite: Query<&mut Visibility, With<RttCompositeSprite>>,
) {
    if !render3d.is_changed() {
        return;
    }

    for mut camera in &mut q_main_camera {
        camera.is_active = render3d.0;
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
        light.shadow_maps_enabled = perf_toggles.directional_light_enabled;
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
        light.shadow_maps_enabled = perf_toggles.extra_directional_light_enabled;
        light.illuminance = if perf_toggles.extra_directional_light_enabled {
            8_000.0
        } else {
            0.0
        };
    }
}

type SceneObjectQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Building3dVisual>, With<ActorBillboard3d>)>>;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::command::AreaEditHandleKind;

    #[derive(Resource, Default)]
    struct ScheduleTrace(Vec<&'static str>);

    fn trace_rebuild(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("rebuild");
    }

    fn trace_upload(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("upload");
    }

    fn trace_door_presentation(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("door-presentation");
    }

    #[test]
    fn world_replace_reset_removes_root_command_visuals() {
        let mut world = World::new();
        let stale = world.spawn_empty().id();
        let designation = world.spawn(DesignationIndicator(stale)).id();
        let task_area = world.spawn(TaskAreaIndicator(stale)).id();
        let handle = world
            .spawn(AreaEditHandleVisual {
                owner: stale,
                kind: AreaEditHandleKind::Center,
            })
            .id();
        let area_selection = world.spawn(AreaSelectionIndicator).id();
        let dream_preview = world.spawn(DreamTreePreviewIndicator).id();

        reset_root_command_visuals(&mut world);

        for entity in [
            designation,
            task_area,
            handle,
            area_selection,
            dream_preview,
        ] {
            assert!(world.get_entity(entity).is_err());
        }
    }

    #[test]
    fn visual_sets_upload_the_current_field_before_door_presentation() {
        let mut app = App::new();
        app.init_resource::<ScheduleTrace>().add_systems(
            Update,
            (
                trace_rebuild.in_set(IndoorLightingRebuildSet),
                trace_upload.in_set(IndoorLightUploadSet),
                trace_door_presentation.in_set(DoorPresentationSyncSet),
            ),
        );
        configure_indoor_light_visual_schedule(&mut app);

        app.update();

        assert_eq!(
            app.world().resource::<ScheduleTrace>().0,
            ["rebuild", "upload", "door-presentation"]
        );
    }
}
