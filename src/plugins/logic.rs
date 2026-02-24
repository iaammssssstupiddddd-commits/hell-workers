//! ゲームロジック関連のプラグイン

use crate::entities::familiar::{familiar_movement, familiar_spawning_system};
use crate::game_state::PlayMode;
use crate::systems::GameSystemSet;
use crate::systems::command::{
    AreaEditClipboard, AreaEditHistory, AreaEditPresets, AreaEditSession, assign_task_system,
    blueprint_cancel_cleanup_system, familiar_command_input_system,
    task_area_edit_history_shortcuts_system, task_area_selection_system, zone_placement_system,
    zone_removal_system,
};
use crate::systems::dream_tree_planting::dream_tree_planting_system;
use crate::systems::jobs::door::{door_auto_close_system, door_auto_open_system};
use crate::systems::jobs::floor_construction::{
    floor_construction_cancellation_system, floor_construction_completion_system,
    floor_construction_phase_transition_system,
};
use crate::systems::jobs::wall_construction::{
    wall_construction_cancellation_system, wall_construction_completion_system,
    wall_construction_phase_transition_system, wall_framed_tile_spawn_system,
};
use crate::systems::logistics::item_lifetime::despawn_expired_items_system;
use crate::systems::logistics::transport_request::TransportRequestPlugin;
use crate::systems::obstacle::obstacle_cleanup_system;
use crate::systems::room::{
    RoomDetectionState, RoomTileLookup, RoomValidationState, detect_rooms_system,
    mark_room_dirty_from_building_changes_system, mark_room_dirty_from_world_map_diff_system,
    validate_rooms_system,
};
use crate::systems::soul_ai::SoulAiPlugin;
use crate::world::regrowth::{RegrowthManager, tree_regrowth_system};
use bevy::prelude::*;

pub struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SoulAiPlugin);
        app.add_plugins(TransportRequestPlugin);

        // パスファインディング用の作業メモリを登録
        app.init_resource::<RegrowthManager>();
        app.init_resource::<AreaEditSession>();
        app.init_resource::<AreaEditHistory>();
        app.init_resource::<AreaEditClipboard>();
        app.init_resource::<AreaEditPresets>();
        app.init_resource::<crate::entities::familiar::FamiliarColorAllocator>();
        app.init_resource::<RoomDetectionState>();
        app.init_resource::<RoomTileLookup>();
        app.init_resource::<RoomValidationState>();

        app.add_systems(
            Update,
            (
                assign_task_system.run_if(in_state(PlayMode::TaskDesignation)),
                familiar_command_input_system.run_if(
                    |selected: Res<crate::interface::selection::SelectedEntity>| {
                        selected.0.is_some()
                    },
                ),
                task_area_selection_system.run_if(in_state(PlayMode::TaskDesignation)),
                zone_placement_system.run_if(in_state(PlayMode::TaskDesignation)),
                zone_removal_system.run_if(in_state(PlayMode::TaskDesignation)),
                task_area_edit_history_shortcuts_system.run_if(in_state(PlayMode::TaskDesignation)),
                familiar_spawning_system,
                tree_regrowth_system,
                obstacle_cleanup_system,
                blueprint_cancel_cleanup_system,
                floor_construction_cancellation_system,
                floor_construction_phase_transition_system,
                floor_construction_completion_system,
                wall_construction_cancellation_system,
                wall_framed_tile_spawn_system,
                wall_construction_phase_transition_system,
                wall_construction_completion_system,
                despawn_expired_items_system,
                dream_tree_planting_system,
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (
                mark_room_dirty_from_building_changes_system,
                mark_room_dirty_from_world_map_diff_system,
                validate_rooms_system,
                detect_rooms_system,
            )
                .chain()
                .after(dream_tree_planting_system)
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (
                door_auto_open_system.before(crate::entities::damned_soul::movement::soul_movement),
                familiar_movement,
                door_auto_close_system.after(crate::entities::damned_soul::movement::soul_movement),
            )
                .in_set(GameSystemSet::Actor),
        );
    }
}
