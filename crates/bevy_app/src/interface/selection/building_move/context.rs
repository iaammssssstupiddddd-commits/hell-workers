use crate::app_contexts::{CompanionPlacementState, MoveContext, MovePlacementState};
use crate::interface::ui::UiInputState;
use crate::systems::jobs::Building;
use crate::systems::logistics::transport_request::TransportRequest;
use crate::systems::soul_ai::execute::task_execution::context::TaskUnassignQueries;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;

pub(super) type SoulTaskQuery<'w, 's> = Query<
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

pub(super) const COMPANION_PLACEMENT_RADIUS_TILES: f32 = 5.0;

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
        (Entity, &'static crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
    pub q_transport_requests: Query<'w, 's, (Entity, &'static TransportRequest)>,
    pub q_souls: SoulTaskQuery<'w, 's>,
    pub task_queries: TaskUnassignQueries<'w, 's>,
}

pub(super) struct MoveStateCtx<'a> {
    pub companion_state: &'a mut CompanionPlacementState,
    pub move_placement_state: &'a mut MovePlacementState,
    pub move_context: &'a mut MoveContext,
    pub next_play_mode: &'a mut NextState<PlayMode>,
}

pub(super) struct MoveOpCtx<'a, 'wc, 'sc, 'wm, 'wq, 'sq> {
    pub commands: &'a mut Commands<'wc, 'sc>,
    pub world_map: &'a mut crate::world::map::WorldMapWrite<'wm>,
    pub q_transport_requests: &'a Query<'wq, 'sq, (Entity, &'static TransportRequest)>,
    pub q_souls: &'a mut SoulTaskQuery<'wq, 'sq>,
    pub task_queries: &'a mut TaskUnassignQueries<'wq, 'sq>,
    pub game_assets: &'a crate::assets::GameAssets,
}
