use crate::app_contexts::{
    CompanionParentKind, CompanionPlacement, CompanionPlacementKind, CompanionPlacementState,
    MoveContext, MovePlacementState, PendingMovePlacement,
};
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Building, BuildingType, Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::TransportRequest;
use crate::systems::soul_ai::execute::task_execution::context::TaskUnassignQueries;
use crate::systems::soul_ai::execute::task_execution::move_plant::{
    MovePlanned, MovePlantReservation,
};
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, MovePlantTask};
use crate::world::map::{WorldMap, WorldMapRef, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::game_state::PlayMode;
use hw_soul_ai::unassign_task;
use hw_ui::camera::MainCamera;
use hw_ui::selection::{
    can_place_moved_building, move_anchor_grid, move_occupied_grids, move_spawn_pos,
};

use super::placement::validate_tank_companion_for_move;

type SoulTaskQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut AssignedTask,
        &'static mut crate::entities::damned_soul::Path,
        Option<&'static mut crate::systems::logistics::Inventory>,
    ),
    With<crate::entities::damned_soul::DamnedSoul>,
>;

const COMPANION_PLACEMENT_RADIUS_TILES: f32 = 5.0;

#[allow(clippy::too_many_arguments)]
pub fn building_move_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut world_map: WorldMapWrite,
    mut move_context: ResMut<MoveContext>,
    mut move_placement_state: ResMut<MovePlacementState>,
    mut companion_state: ResMut<CompanionPlacementState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    q_buildings: Query<(Entity, &Building, &Transform)>,
    q_bucket_storages: Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
    q_transport_requests: Query<(Entity, &TransportRequest)>,
    mut q_souls: SoulTaskQuery,
    mut task_queries: TaskUnassignQueries,
    game_assets: Res<crate::assets::GameAssets>,
    mut commands: Commands,
) {
    // --- Phase 1: 入力と早期 return ---
    if ui_input_state.pointer_over_ui {
        return;
    }
    if buttons.just_pressed(MouseButton::Right) {
        clear_move_states(
            &mut move_context,
            &mut move_placement_state,
            &mut companion_state,
        );
        next_play_mode.set(PlayMode::Normal);
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let destination_grid = WorldMap::world_to_grid(world_pos);
    let Some(target_entity) = move_context.0 else {
        return;
    };

    // --- Phase 2: 配置対象の検証 ---
    let Ok((_, building, transform)) = q_buildings.get(target_entity) else {
        clear_move_states(
            &mut move_context,
            &mut move_placement_state,
            &mut companion_state,
        );
        next_play_mode.set(PlayMode::Normal);
        return;
    };
    if !matches!(building.kind, BuildingType::Tank | BuildingType::MudMixer) {
        clear_move_states(
            &mut move_context,
            &mut move_placement_state,
            &mut companion_state,
        );
        next_play_mode.set(PlayMode::Normal);
        return;
    }

    // --- Phase 3a: Companion 配置確認クリック ---
    if companion_state.0.is_some() {
        handle_companion_click(
            &mut commands,
            &mut world_map,
            &q_transport_requests,
            &mut q_souls,
            &mut task_queries,
            &game_assets,
            &mut companion_state,
            &mut move_placement_state,
            &mut move_context,
            &mut next_play_mode,
            destination_grid,
            target_entity,
            building,
            transform,
            &q_bucket_storages,
        );
        return;
    }

    // --- Phase 3b: 初回配置クリック ---
    handle_initial_click(
        &mut commands,
        &mut world_map,
        &q_transport_requests,
        &mut q_souls,
        &mut task_queries,
        &game_assets,
        &mut companion_state,
        &mut move_placement_state,
        &mut move_context,
        &mut next_play_mode,
        destination_grid,
        target_entity,
        building,
        transform,
    );
}

/// Companion（BucketStorage）配置確認ステップのクリック処理。
#[allow(clippy::too_many_arguments)]
fn handle_companion_click(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    q_transport_requests: &Query<(Entity, &TransportRequest)>,
    q_souls: &mut SoulTaskQuery,
    task_queries: &mut TaskUnassignQueries,
    game_assets: &crate::assets::GameAssets,
    companion_state: &mut CompanionPlacementState,
    move_placement_state: &mut MovePlacementState,
    move_context: &mut MoveContext,
    next_play_mode: &mut NextState<PlayMode>,
    destination_grid: (i32, i32),
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
    q_bucket_storages: &Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
) {
    let Some(active_companion) = companion_state.0.clone() else {
        return;
    };
    if active_companion.kind != CompanionPlacementKind::BucketStorage
        || active_companion.parent_kind != CompanionParentKind::Tank
    {
        companion_state.0 = None;
        move_placement_state.0 = None;
        return;
    }
    let Some(pending) = move_placement_state.0 else {
        companion_state.0 = None;
        return;
    };
    if pending.building != target_entity {
        companion_state.0 = None;
        move_placement_state.0 = None;
        return;
    }
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, pending.destination_grid);
    if !can_place_moved_building(
        &WorldMapRef(world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    ) {
        return;
    }
    if !validate_tank_companion_for_move(
        world_map,
        target_entity,
        pending.destination_grid,
        destination_grid,
        &old_occupied,
        q_bucket_storages,
    )
    .can_place
    {
        return;
    }
    finalize_move_request(
        commands,
        world_map,
        q_transport_requests,
        q_souls,
        task_queries,
        game_assets,
        target_entity,
        building,
        transform,
        pending.destination_grid,
        Some(destination_grid),
    );
    clear_move_states(move_context, move_placement_state, companion_state);
    next_play_mode.set(PlayMode::Normal);
}

/// 初回クリック時の配置検証・移動確定処理。
/// Tank は companion 配置フローへ移行、MudMixer は即確定。
#[allow(clippy::too_many_arguments)]
fn handle_initial_click(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    q_transport_requests: &Query<(Entity, &TransportRequest)>,
    q_souls: &mut SoulTaskQuery,
    task_queries: &mut TaskUnassignQueries,
    game_assets: &crate::assets::GameAssets,
    companion_state: &mut CompanionPlacementState,
    move_placement_state: &mut MovePlacementState,
    move_context: &mut MoveContext,
    next_play_mode: &mut NextState<PlayMode>,
    destination_grid: (i32, i32),
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
) {
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    if !can_place_moved_building(
        &WorldMapRef(world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    ) {
        return;
    }
    if building.kind == BuildingType::Tank {
        let center = move_spawn_pos(BuildingType::Tank, destination_grid);
        move_placement_state.0 = Some(PendingMovePlacement {
            building: target_entity,
            destination_grid,
        });
        companion_state.0 = Some(CompanionPlacement {
            parent_kind: CompanionParentKind::Tank,
            parent_anchor: destination_grid,
            kind: CompanionPlacementKind::BucketStorage,
            center,
            radius: TILE_SIZE * COMPANION_PLACEMENT_RADIUS_TILES,
            required: true,
        });
        return;
    }
    finalize_move_request(
        commands,
        world_map,
        q_transport_requests,
        q_souls,
        task_queries,
        game_assets,
        target_entity,
        building,
        transform,
        destination_grid,
        None,
    );
    clear_move_states(move_context, move_placement_state, companion_state);
    next_play_mode.set(PlayMode::Normal);
}

fn clear_move_states(
    move_context: &mut MoveContext,
    move_placement_state: &mut MovePlacementState,
    companion_state: &mut CompanionPlacementState,
) {
    move_context.0 = None;
    move_placement_state.0 = None;
    companion_state.0 = None;
}

#[allow(clippy::too_many_arguments)]
fn finalize_move_request(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    q_transport_requests: &Query<(Entity, &TransportRequest)>,
    q_souls: &mut SoulTaskQuery,
    task_queries: &mut TaskUnassignQueries,
    game_assets: &crate::assets::GameAssets,
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
    destination_grid: (i32, i32),
    companion_anchor: Option<(i32, i32)>,
) {
    cancel_tasks_and_requests_for_moved_building(
        commands,
        target_entity,
        q_transport_requests,
        q_souls,
        task_queries,
        world_map,
    );

    let destination_pos = move_spawn_pos(building.kind, destination_grid);
    let (texture, size) = match building.kind {
        BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        _ => return,
    };
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    let companion_occupied = companion_anchor
        .map(|anchor| vec![anchor, (anchor.0 + 1, anchor.1)])
        .unwrap_or_default();
    let mut reserved_occupied = destination_occupied.clone();
    reserved_occupied.extend(companion_occupied.iter().copied());
    let task_entity = commands.spawn_empty().id();
    world_map.add_grid_obstacles(reserved_occupied.iter().copied());

    commands.entity(task_entity).with_children(|parent| {
        for &(gx, gy) in &reserved_occupied {
            parent.spawn((
                crate::systems::jobs::ObstaclePosition(gx, gy),
                Name::new("Move Reservation Obstacle"),
            ));
        }
    });
    commands.entity(task_entity).insert((
        Designation {
            work_type: WorkType::Move,
        },
        TaskSlots::new(1),
        Priority(10),
        MovePlantReservation {
            occupied: reserved_occupied,
        },
        MovePlantTask {
            building: target_entity,
            destination_grid,
            destination_pos,
            companion_anchor,
        },
        Sprite {
            image: texture,
            color: Color::srgba(1.0, 1.0, 1.0, 0.35),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(
            destination_pos.x,
            destination_pos.y,
            transform.translation.z,
        ),
        Name::new("Move Plant Task"),
    ));
    commands
        .entity(target_entity)
        .insert(MovePlanned { task_entity });
}

fn cancel_tasks_and_requests_for_moved_building(
    commands: &mut Commands,
    building_entity: Entity,
    q_transport_requests: &Query<(Entity, &TransportRequest)>,
    q_souls: &mut SoulTaskQuery,
    task_queries: &mut TaskUnassignQueries,
    world_map: &WorldMap,
) {
    commands.entity(building_entity).remove::<(
        Designation,
        TaskSlots,
        Priority,
        hw_core::relationships::ManagedBy,
    )>();

    for (request_entity, request) in q_transport_requests.iter() {
        if request.anchor == building_entity {
            commands.entity(request_entity).despawn();
        }
    }

    for (soul_entity, transform, mut task, mut path, mut inventory) in q_souls.iter_mut() {
        if task_targets_building(&task, building_entity) {
            unassign_task(
                commands,
                soul_entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                inventory.as_deref_mut(),
                None,
                task_queries,
                world_map,
                false,
            );
        }
    }
}

fn task_targets_building(task: &AssignedTask, building_entity: Entity) -> bool {
    if let Some(data) = task.bucket_transport_data() {
        if matches!(
            data.source,
            crate::systems::soul_ai::execute::task_execution::types::BucketTransportSource::Tank {
                tank,
                ..
            } if tank == building_entity
        ) {
            return true;
        }

        match data.destination {
            crate::systems::soul_ai::execute::task_execution::types::BucketTransportDestination::Tank(tank) => {
                if tank == building_entity {
                    return true;
                }
            }
            crate::systems::soul_ai::execute::task_execution::types::BucketTransportDestination::Mixer(mixer) => {
                if mixer == building_entity {
                    return true;
                }
            }
        }
    }

    match task {
        AssignedTask::Refine(data) => data.mixer == building_entity,
        AssignedTask::HaulToMixer(data) => data.mixer == building_entity,
        AssignedTask::MovePlant(data) => data.building == building_entity,
        _ => false,
    }
}
