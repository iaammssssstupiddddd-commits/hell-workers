//! ゲームロジック関連のプラグイン

use crate::entities::familiar::{familiar_movement, familiar_spawning_system};
use crate::systems::GameSystemSet;
use crate::systems::command::{
    AreaEditClipboard, AreaEditHistory, AreaEditPresets, AreaEditSession, ZoneRemovalPreviewState,
    assign_task_system, blueprint_cancel_cleanup_system, familiar_command_input_system,
    task_area_edit_history_shortcuts_system, task_area_selection_system, zone_placement_system,
    zone_removal_system,
};
use crate::systems::dream_tree_planting::dream_tree_planting_system;
use crate::systems::energy::grid_lifecycle::{
    on_power_consumer_added, on_yard_added, on_yard_removed,
};
use crate::systems::energy::grid_recalc::{
    EnergyUpdateDirty, detect_energy_update_dirty_system, energy_grid_recalc_should_run,
    energy_power_output_should_run, grid_recalc_system,
};
use crate::systems::energy::lamp_buff::lamp_buff_system;
use crate::systems::energy::power_output::soul_spa_power_output_system;
use crate::systems::familiar_ai::FamiliarAiPlugin;
use crate::systems::jobs::floor_construction::{
    floor_construction_cancellation_system, floor_construction_completion_system,
    floor_construction_phase_transition_system,
};
use crate::systems::jobs::soul_spa_construction::{
    soul_spa_auto_haul_system, soul_spa_delivery_sync_system, soul_spa_tile_activate_system,
};
use crate::systems::jobs::wall_construction::{
    wall_construction_cancellation_system, wall_construction_completion_system,
    wall_construction_phase_transition_system, wall_framed_tile_spawn_system,
};
use crate::systems::jobs::{
    BuildingCompletionSet, TaskOwnerCancellationSet, blueprint_cancellation_system,
    building_completion_system, door_auto_close_nearby_system, door_auto_open_nearby_system,
};
use crate::systems::logistics::item_lifetime::despawn_expired_items_system;
use crate::systems::logistics::transport_request::{TransportRequestPlugin, TransportRequestSet};
use crate::systems::soul_ai::SoulAiPlugin;
use crate::world::regrowth::{RegrowthManager, tree_regrowth_system};
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_core::system_sets::{FamiliarAiSystemSet, ObstacleSyncSet, SoulAiSystemSet};
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaPhase, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};
use hw_jobs::visual_sync::{
    on_building_added_sync_visual, on_designation_added, on_designation_removed,
    on_mud_mixer_storage_added, on_power_consumer_visual_added, on_rest_area_added,
    on_unpowered_added, on_unpowered_removed, sync_blueprint_visual_system,
    sync_building_visual_system, sync_floor_site_visual_system, sync_floor_tile_visual_system,
    sync_mud_mixer_active_system, sync_soul_task_visual_system, sync_wall_site_visual_system,
    sync_wall_tile_visual_system,
};
use hw_jobs::{GeneratePowerData, GeneratePowerPhase, TargetSoulSpaSite};
use hw_logistics::visual_sync::{
    on_stockpile_added_sync_visual, on_wheelbarrow_added, sync_inventory_item_visual_system,
    sync_stockpile_visual_system,
};
use hw_world::{
    ObstaclePositionIndex, RoomDetectionState, RoomTileLookup, RoomValidationState,
    detect_rooms_system, mark_room_dirty_from_building_changes_system, obstacle_sync_system,
    on_building_added, on_building_removed, on_door_added, on_door_removed, validate_rooms_system,
};

pub struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SoulAiPlugin);
        app.add_plugins(FamiliarAiPlugin);
        app.add_plugins(TransportRequestPlugin);
        app.add_plugins(hw_logistics::LogisticsPlugin);

        // パスファインディング用の作業メモリを登録
        app.init_resource::<RegrowthManager>();
        app.init_resource::<AreaEditSession>();
        app.init_resource::<AreaEditHistory>();
        app.init_resource::<AreaEditClipboard>();
        app.init_resource::<AreaEditPresets>();
        app.init_resource::<ZoneRemovalPreviewState>();
        app.init_resource::<crate::entities::familiar::FamiliarColorAllocator>();
        app.init_resource::<RoomDetectionState>();
        app.init_resource::<RoomTileLookup>();
        app.init_resource::<RoomValidationState>();
        app.init_resource::<ObstaclePositionIndex>();
        app.init_resource::<EnergyUpdateDirty>();
        #[cfg(feature = "profiling")]
        app.init_resource::<crate::systems::jobs::DoorPerfMetrics>()
            .init_resource::<crate::systems::jobs::ConstructionPerfMetrics>()
            .init_resource::<crate::systems::energy::grid_recalc::EnergyPerfMetrics>();

        app.configure_sets(
            Update,
            (
                TaskOwnerCancellationSet::Cancel,
                TaskOwnerCancellationSet::Flush,
            )
                .chain()
                .before(FamiliarAiSystemSet::Perceive)
                .before(SoulAiSystemSet::Perceive)
                .before(TransportRequestSet::Perceive)
                .in_set(GameSystemSet::Logic),
        );
        app.add_systems(
            Update,
            (
                blueprint_cancellation_system,
                floor_construction_cancellation_system,
                wall_construction_cancellation_system,
            )
                .in_set(TaskOwnerCancellationSet::Cancel),
        )
        .add_systems(
            Update,
            ApplyDeferred.in_set(TaskOwnerCancellationSet::Flush),
        );

        // Soul Energy 型登録
        app.register_type::<PowerGrid>()
            .register_type::<PowerGenerator>()
            .register_type::<PowerConsumer>()
            .register_type::<Unpowered>()
            .register_type::<YardPowerGrid>()
            .register_type::<GeneratesFor>()
            .register_type::<GridGenerators>()
            .register_type::<ConsumesFrom>()
            .register_type::<GridConsumers>()
            .register_type::<SoulSpaSite>()
            .register_type::<SoulSpaTile>()
            .register_type::<SoulSpaPhase>()
            .register_type::<GeneratePowerData>()
            .register_type::<GeneratePowerPhase>()
            .register_type::<TargetSoulSpaSite>();

        // グループA: command 系（直列維持 — TaskContext / AreaEdit / WorldMapWrite が競合）
        app.add_systems(
            Update,
            (
                assign_task_system.run_if(in_state(PlayMode::TaskDesignation)),
                familiar_command_input_system,
                task_area_selection_system,
                zone_placement_system.run_if(in_state(PlayMode::TaskDesignation)),
                zone_removal_system.run_if(in_state(PlayMode::TaskDesignation)),
                task_area_edit_history_shortcuts_system.run_if(in_state(PlayMode::TaskDesignation)),
            )
                .chain()
                .before(SoulAiSystemSet::Perceive)
                .in_set(GameSystemSet::Logic),
        )
        // User cancellation writes SoulTaskUnassignRequest through Commands.
        // Apply it before Perceive so cleanup precedes task execution.
        .add_systems(
            Update,
            bevy::ecs::schedule::ApplyDeferred
                .after(task_area_selection_system)
                .before(SoulAiSystemSet::Perceive)
                .in_set(GameSystemSet::Logic),
        )
        // グループB: maintenance / spawn 系（独立。Bevy scheduler が競合を自動調停）
        .add_systems(
            Update,
            (
                tree_regrowth_system,
                blueprint_cancel_cleanup_system,
                despawn_expired_items_system,
                dream_tree_planting_system,
            )
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            building_completion_system
                .after(SoulAiSystemSet::Execute)
                .in_set(BuildingCompletionSet)
                .in_set(GameSystemSet::Logic),
        )
        // グループC: floor construction（フェーズ順序が必要）
        .add_systems(
            Update,
            (
                floor_construction_phase_transition_system,
                floor_construction_completion_system,
            )
                .chain()
                .after(TaskOwnerCancellationSet::Flush)
                .in_set(GameSystemSet::Logic),
        )
        // グループD: wall construction（フェーズ順序が必要）
        .add_systems(
            Update,
            (
                crate::plugins::interface_debug::debug_instant_complete_walls_system
                    .run_if(|d: Res<crate::DebugInstantBuild>| d.0),
                wall_framed_tile_spawn_system,
                wall_construction_phase_transition_system,
                wall_construction_completion_system,
            )
                .chain()
                .after(TaskOwnerCancellationSet::Flush)
                .in_set(GameSystemSet::Logic),
        )
        // グループE: Soul Spa construction + energy pipeline.
        // Commands that attach workers/children are visible before dirty
        // detection. A changed generator then propagates through grid state
        // and `Unpowered` before the 10 Hz lamp effect reads it.
        .add_systems(
            Update,
            (
                soul_spa_auto_haul_system,
                soul_spa_delivery_sync_system,
                soul_spa_tile_activate_system,
                bevy::ecs::schedule::ApplyDeferred,
                detect_energy_update_dirty_system,
                soul_spa_power_output_system.run_if(energy_power_output_should_run),
                grid_recalc_system.run_if(energy_grid_recalc_should_run),
                bevy::ecs::schedule::ApplyDeferred,
                lamp_buff_system,
            )
                .chain()
                .after(SoulAiSystemSet::Update)
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (
                sync_inventory_item_visual_system,
                sync_soul_task_visual_system,
                sync_blueprint_visual_system,
                sync_floor_tile_visual_system,
                sync_wall_tile_visual_system,
                sync_floor_site_visual_system,
                sync_wall_site_visual_system,
                sync_building_visual_system,
                sync_stockpile_visual_system,
                sync_mud_mixer_active_system,
            )
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (
                mark_room_dirty_from_building_changes_system,
                validate_rooms_system,
                detect_rooms_system,
            )
                .chain()
                .after(dream_tree_planting_system)
                .in_set(GameSystemSet::Logic),
        )
        .add_observer(on_building_added)
        .add_observer(on_building_removed)
        .add_observer(on_door_added)
        .add_observer(on_door_removed)
        .add_observer(on_designation_added)
        .add_observer(on_designation_removed)
        .add_observer(on_rest_area_added)
        .add_observer(on_wheelbarrow_added)
        .add_observer(on_building_added_sync_visual)
        .add_observer(on_mud_mixer_storage_added)
        .add_observer(on_stockpile_added_sync_visual)
        .add_observer(on_yard_added)
        .add_observer(on_yard_removed)
        .add_observer(on_power_consumer_added)
        .add_observer(on_power_consumer_visual_added)
        .add_observer(on_unpowered_added)
        .add_observer(on_unpowered_removed);

        configure_obstacle_sync_schedule(app);

        app.add_systems(
            Update,
            (
                door_auto_open_nearby_system
                    .before(crate::entities::damned_soul::movement::soul_movement),
                familiar_movement,
                door_auto_close_nearby_system
                    .after(crate::entities::damned_soul::movement::soul_movement),
            )
                .in_set(GameSystemSet::Actor),
        );

        #[cfg(feature = "profiling")]
        app.add_systems(
            Update,
            familiar_spawning_system
                .in_set(GameSystemSet::Logic)
                .run_if(crate::plugins::startup::is_not_fixed_step_audit),
        );

        #[cfg(not(feature = "profiling"))]
        app.add_systems(
            Update,
            familiar_spawning_system.in_set(GameSystemSet::Logic),
        );
    }
}

fn configure_obstacle_sync_schedule(app: &mut App) {
    app.configure_sets(
        Update,
        ObstacleSyncSet
            .in_set(GameSystemSet::Actor)
            .before(SoulAiSystemSet::Actor),
    )
    .add_systems(
        Update,
        bevy::ecs::schedule::ApplyDeferred
            .after(SoulAiSystemSet::Execute)
            .after(BuildingCompletionSet)
            .before(ObstacleSyncSet)
            .in_set(GameSystemSet::Actor),
    )
    .add_systems(Update, obstacle_sync_system.in_set(ObstacleSyncSet));
}

#[cfg(test)]
mod tests {
    use super::configure_obstacle_sync_schedule;
    use crate::systems::GameSystemSet;
    use bevy::prelude::*;
    use hw_core::system_sets::SoulAiSystemSet;
    use hw_jobs::{ObstaclePosition, ObstacleSourceKind};
    use hw_world::{
        ObstaclePositionIndex, TerrainChangedEvent, WorldMap, seed_obstacle_position_index,
    };

    #[derive(Resource)]
    struct PendingMarkerRemoval(Option<Entity>);

    #[derive(Resource, Default)]
    struct PathfindingProbe(Option<bool>);

    fn remove_marker_in_execute(mut commands: Commands, mut pending: ResMut<PendingMarkerRemoval>) {
        if let Some(marker) = pending.0.take() {
            commands.entity(marker).remove::<ObstaclePosition>();
        }
    }

    fn record_pathfinding_input(world_map: Res<WorldMap>, mut probe: ResMut<PathfindingProbe>) {
        probe.0 = Some(world_map.is_walkable(21, 22));
    }

    #[test]
    fn obstacle_sync_applies_execute_removal_before_actor_pathfinding() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldMap::default())
            .init_resource::<ObstaclePositionIndex>()
            .init_resource::<PathfindingProbe>()
            .add_message::<TerrainChangedEvent>()
            .configure_sets(Update, (GameSystemSet::Logic, GameSystemSet::Actor).chain())
            .configure_sets(
                Update,
                SoulAiSystemSet::Execute.in_set(GameSystemSet::Logic),
            )
            .configure_sets(Update, SoulAiSystemSet::Actor.in_set(GameSystemSet::Actor))
            .add_systems(
                Update,
                remove_marker_in_execute.in_set(SoulAiSystemSet::Execute),
            )
            .add_systems(
                Update,
                record_pathfinding_input.in_set(SoulAiSystemSet::Actor),
            );
        configure_obstacle_sync_schedule(&mut app);

        let marker = app
            .world_mut()
            .spawn((
                ObstaclePosition(21, 22),
                ObstacleSourceKind::NaturalTerrainClearing,
            ))
            .id();
        seed_obstacle_position_index(app.world_mut());
        app.world_mut()
            .resource_mut::<WorldMap>()
            .add_grid_obstacle((21, 22));
        app.insert_resource(PendingMarkerRemoval(Some(marker)));

        app.update();

        assert_eq!(app.world().resource::<PathfindingProbe>().0, Some(true));
    }
}
