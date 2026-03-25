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
use bevy::ecs::system::SystemParam;
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

#[derive(SystemParam)]
pub struct BuildMoveInput<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
}

#[derive(SystemParam)]
pub struct BuildMoveState<'w> {
    pub move_context: ResMut<'w, MoveContext>,
    pub move_placement_state: ResMut<'w, MovePlacementState>,
    pub companion_state: ResMut<'w, CompanionPlacementState>,
    pub next_play_mode: ResMut<'w, NextState<PlayMode>>,
}

#[derive(SystemParam)]
pub struct BuildMoveQueries<'w, 's> {
    pub q_buildings: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
    pub q_bucket_storages: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::systems::logistics::BelongsTo,
        ),
        With<crate::systems::logistics::BucketStorage>,
    >,
    pub q_transport_requests: Query<'w, 's, (Entity, &'static TransportRequest)>,
    pub q_souls: SoulTaskQuery<'w, 's>,
    pub task_queries: TaskUnassignQueries<'w, 's>,
}

struct MoveStateCtx<'a> {
    companion_state: &'a mut CompanionPlacementState,
    move_placement_state: &'a mut MovePlacementState,
    move_context: &'a mut MoveContext,
    next_play_mode: &'a mut NextState<PlayMode>,
}

struct MoveOpCtx<'a, 'wc, 'sc, 'wm, 'wq, 'sq> {
    commands: &'a mut Commands<'wc, 'sc>,
    world_map: &'a mut WorldMapWrite<'wm>,
    q_transport_requests: &'a Query<'wq, 'sq, (Entity, &'static TransportRequest)>,
    q_souls: &'a mut SoulTaskQuery<'wq, 'sq>,
    task_queries: &'a mut TaskUnassignQueries<'wq, 'sq>,
    game_assets: &'a crate::assets::GameAssets,
}

pub fn building_move_system(
    input: BuildMoveInput,
    mut state: BuildMoveState,
    mut queries: BuildMoveQueries,
    mut world_map: WorldMapWrite,
    game_assets: Res<crate::assets::GameAssets>,
    mut commands: Commands,
) {
    // --- Phase 1: 入力と早期 return ---
    if input.ui_input_state.pointer_over_ui {
        return;
    }
    if input.buttons.just_pressed(MouseButton::Right) {
        clear_move_states(
            &mut state.move_context,
            &mut state.move_placement_state,
            &mut state.companion_state,
        );
        state.next_play_mode.set(PlayMode::Normal);
        return;
    }
    if !input.buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let destination_grid = WorldMap::world_to_grid(world_pos);
    let Some(target_entity) = state.move_context.0 else {
        return;
    };

    // --- Phase 2: 配置対象の検証 ---
    let Ok((_, building, transform)) = queries.q_buildings.get(target_entity) else {
        clear_move_states(
            &mut state.move_context,
            &mut state.move_placement_state,
            &mut state.companion_state,
        );
        state.next_play_mode.set(PlayMode::Normal);
        return;
    };
    if !matches!(building.kind, BuildingType::Tank | BuildingType::MudMixer) {
        clear_move_states(
            &mut state.move_context,
            &mut state.move_placement_state,
            &mut state.companion_state,
        );
        state.next_play_mode.set(PlayMode::Normal);
        return;
    }

    let mut op = MoveOpCtx {
        commands: &mut commands,
        world_map: &mut world_map,
        q_transport_requests: &queries.q_transport_requests,
        q_souls: &mut queries.q_souls,
        task_queries: &mut queries.task_queries,
        game_assets: &game_assets,
    };
    let mut st = MoveStateCtx {
        companion_state: &mut state.companion_state,
        move_placement_state: &mut state.move_placement_state,
        move_context: &mut state.move_context,
        next_play_mode: &mut state.next_play_mode,
    };

    // --- Phase 3a: Companion 配置確認クリック ---
    if st.companion_state.0.is_some() {
        handle_companion_click(
            &mut op,
            &mut st,
            destination_grid,
            target_entity,
            building,
            transform,
            &queries.q_bucket_storages,
        );
        return;
    }

    // --- Phase 3b: 初回配置クリック ---
    handle_initial_click(
        &mut op,
        &mut st,
        destination_grid,
        target_entity,
        building,
        transform,
    );
}

/// Companion（BucketStorage）配置確認ステップのクリック処理。
fn handle_companion_click(
    op: &mut MoveOpCtx<'_, '_, '_, '_, '_, '_>,
    st: &mut MoveStateCtx<'_>,
    destination_grid: (i32, i32),
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
    q_bucket_storages: &Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
) {
    let Some(active_companion) = st.companion_state.0.clone() else {
        return;
    };
    if active_companion.kind != CompanionPlacementKind::BucketStorage
        || active_companion.parent_kind != CompanionParentKind::Tank
    {
        st.companion_state.0 = None;
        st.move_placement_state.0 = None;
        return;
    }
    let Some(pending) = st.move_placement_state.0 else {
        st.companion_state.0 = None;
        return;
    };
    if pending.building != target_entity {
        st.companion_state.0 = None;
        st.move_placement_state.0 = None;
        return;
    }
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, pending.destination_grid);
    if !can_place_moved_building(
        &WorldMapRef(op.world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    ) {
        return;
    }
    if !validate_tank_companion_for_move(
        op.world_map,
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
        op,
        target_entity,
        building,
        transform,
        pending.destination_grid,
        Some(destination_grid),
    );
    clear_move_states(st.move_context, st.move_placement_state, st.companion_state);
    st.next_play_mode.set(PlayMode::Normal);
}

/// 初回クリック時の配置検証・移動確定処理。
/// Tank は companion 配置フローへ移行、MudMixer は即確定。
fn handle_initial_click(
    op: &mut MoveOpCtx<'_, '_, '_, '_, '_, '_>,
    st: &mut MoveStateCtx<'_>,
    destination_grid: (i32, i32),
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
) {
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    if !can_place_moved_building(
        &WorldMapRef(op.world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    ) {
        return;
    }
    if building.kind == BuildingType::Tank {
        let center = move_spawn_pos(BuildingType::Tank, destination_grid);
        st.move_placement_state.0 = Some(PendingMovePlacement {
            building: target_entity,
            destination_grid,
        });
        st.companion_state.0 = Some(CompanionPlacement {
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
        op,
        target_entity,
        building,
        transform,
        destination_grid,
        None,
    );
    clear_move_states(st.move_context, st.move_placement_state, st.companion_state);
    st.next_play_mode.set(PlayMode::Normal);
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

fn finalize_move_request(
    op: &mut MoveOpCtx<'_, '_, '_, '_, '_, '_>,
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
    destination_grid: (i32, i32),
    companion_anchor: Option<(i32, i32)>,
) {
    cancel_tasks_and_requests_for_moved_building(
        op.commands,
        target_entity,
        op.q_transport_requests,
        op.q_souls,
        op.task_queries,
        &*op.world_map,
    );

    let destination_pos = move_spawn_pos(building.kind, destination_grid);
    let (texture, size) = match building.kind {
        BuildingType::Tank => (op.game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::MudMixer => (op.game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        _ => return,
    };
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    let companion_occupied = companion_anchor
        .map(|anchor| vec![anchor, (anchor.0 + 1, anchor.1)])
        .unwrap_or_default();
    let mut reserved_occupied = destination_occupied.clone();
    reserved_occupied.extend(companion_occupied.iter().copied());
    let task_entity = op.commands.spawn_empty().id();
    op.world_map.add_grid_obstacles(reserved_occupied.iter().copied());

    op.commands.entity(task_entity).with_children(|parent| {
        for &(gx, gy) in &reserved_occupied {
            parent.spawn((
                crate::systems::jobs::ObstaclePosition(gx, gy),
                Name::new("Move Reservation Obstacle"),
            ));
        }
    });
    op.commands.entity(task_entity).insert((
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
    op.commands
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
                hw_soul_ai::SoulDropCtx {
                    soul_entity,
                    drop_pos: transform.translation.truncate(),
                    inventory: inventory.as_deref_mut(),
                    dropped_item_res: None,
                },
                &mut task,
                &mut path,
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
