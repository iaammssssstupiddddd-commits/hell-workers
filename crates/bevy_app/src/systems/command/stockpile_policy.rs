use std::cmp::Ordering;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::TILE_SIZE;
use hw_core::game_state::PlayMode;
use hw_logistics::StockpilePolicyPatch;
use hw_spatial::StockpileSpatialGrid;
use hw_ui::UiIntent;
use hw_ui::camera::{MainCamera, world_cursor_pos};
use hw_ui::components::UiInputState;
use hw_ui::intents::StockpilePolicyEditTarget;

use crate::app_contexts::TaskContext;
use crate::world::map::WorldMap;

use super::TaskMode;

/// Root-owned session state for the one-shot rectangular policy gesture.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockpilePolicyRangeEditState {
    pub patch: Option<StockpilePolicyPatch>,
}

/// Resolves a copyable UI target into a stable, deduplicated entity list.
///
/// Area resolution deliberately keeps special storages in the request. The domain handler owns
/// the positive `Stockpile + StockpilePolicy` boundary and reports mixed-selection skips.
#[must_use]
pub fn resolve_stockpile_policy_targets(
    target: StockpilePolicyEditTarget,
    stockpile_grid: &StockpileSpatialGrid,
) -> Vec<Entity> {
    match target {
        StockpilePolicyEditTarget::Single(entity) => vec![entity],
        StockpilePolicyEditTarget::Area { min, max } => {
            let area_min = min.min(max);
            let area_max = min.max(max);
            let mut targets = stockpile_grid.get_in_area(area_min, area_max);
            let positions = &stockpile_grid.data().positions;
            targets.sort_unstable_by(|left, right| {
                match (positions.get(left), positions.get(right)) {
                    (Some(left_pos), Some(right_pos)) => left_pos
                        .y
                        .total_cmp(&right_pos.y)
                        .then_with(|| left_pos.x.total_cmp(&right_pos.x)),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
                .then_with(|| {
                    (left.index_u32(), left.generation().to_bits())
                        .cmp(&(right.index_u32(), right.generation().to_bits()))
                })
            });
            targets.dedup();
            targets
        }
    }
}

/// Owns pointer press/release while `TaskMode::StockpilePolicyEdit` is active.
///
/// The release emits the same `UiIntent::ApplyStockpilePolicy` used by a single-cell editor;
/// concrete target resolution and domain mutation happen later in the root intent adapter.
#[derive(SystemParam)]
pub struct StockpilePolicyRangeSelectionParams<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
    task_context: ResMut<'w, TaskContext>,
    next_play_mode: ResMut<'w, NextState<PlayMode>>,
    range_state: ResMut<'w, StockpilePolicyRangeEditState>,
    ui_intents: MessageWriter<'w, UiIntent>,
}

pub fn stockpile_policy_range_selection_system(mut params: StockpilePolicyRangeSelectionParams) {
    let TaskMode::StockpilePolicyEdit(start) = params.task_context.0 else {
        params.range_state.patch = None;
        return;
    };
    let Some(patch) = params.range_state.patch else {
        params.task_context.0 = TaskMode::None;
        params.next_play_mode.set(PlayMode::Normal);
        return;
    };
    if params.ui_input_state.world_input_blocked() {
        return;
    }

    if start.is_none() && params.buttons.just_pressed(MouseButton::Left) {
        if let Some(world_pos) = world_cursor_pos(&params.q_window, &params.q_camera) {
            params.task_context.0 =
                TaskMode::StockpilePolicyEdit(Some(WorldMap::snap_to_grid_edge(world_pos)));
        }
        return;
    }

    let Some(start_pos) = start else {
        return;
    };
    if !params.buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some(world_pos) = world_cursor_pos(&params.q_window, &params.q_camera) else {
        return;
    };
    let end_pos = WorldMap::snap_to_grid_edge(world_pos);
    let (min, max) = if start_pos.distance_squared(end_pos) <= f32::EPSILON {
        let center = WorldMap::snap_to_grid_center(world_pos);
        let half = Vec2::splat(TILE_SIZE * 0.5);
        (center - half, center + half)
    } else {
        (start_pos.min(end_pos), start_pos.max(end_pos))
    };

    params.ui_intents.write(UiIntent::ApplyStockpilePolicy {
        target: StockpilePolicyEditTarget::Area { min, max },
        patch,
    });
    params.range_state.patch = None;
    params.task_context.0 = TaskMode::None;
    params.next_play_mode.set(PlayMode::Normal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_spatial::SpatialGridOps;

    #[test]
    fn stockpile_policy_area_targets_are_spatially_stable_and_deduplicated() {
        let mut world = World::new();
        let upper_right = world.spawn_empty().id();
        let lower_right = world.spawn_empty().id();
        let lower_left = world.spawn_empty().id();
        let outside = world.spawn_empty().id();
        let mut grid = StockpileSpatialGrid::default();
        grid.insert(upper_right, Vec2::new(16.0, 16.0));
        grid.insert(lower_right, Vec2::new(16.0, 0.0));
        grid.insert(outside, Vec2::new(64.0, 64.0));
        grid.insert(lower_left, Vec2::ZERO);

        let targets = resolve_stockpile_policy_targets(
            StockpilePolicyEditTarget::Area {
                min: Vec2::splat(32.0),
                max: Vec2::splat(-1.0),
            },
            &grid,
        );

        assert_eq!(targets, vec![lower_left, lower_right, upper_right]);
    }

    #[test]
    fn stockpile_policy_single_target_is_preserved_for_stale_validation() {
        let entity = Entity::from_raw_u32(41).expect("valid entity");
        assert_eq!(
            resolve_stockpile_policy_targets(
                StockpilePolicyEditTarget::Single(entity),
                &StockpileSpatialGrid::default(),
            ),
            vec![entity]
        );
    }
}
