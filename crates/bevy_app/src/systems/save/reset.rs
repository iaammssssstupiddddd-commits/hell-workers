//! Fixed-phase reset coordination for persistent world replacement.
//!
//! The root crate owns the registry because leaf crates must not depend on
//! `bevy_app`. A leaf exposes a `reset_for_world_replace(&mut World)` function
//! for its own state, and its root plugin facade registers that function here.

use bevy::prelude::*;

use hw_core::WorldEpoch;
use hw_core::game_state::PlayMode;
use hw_core::selection::{HoveredEntity, SelectedEntity};
use hw_logistics::resource_cache::SharedResourceCache;
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::transport_request::TransportRequestMetrics;
use hw_logistics::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards, CachedStockpileGroups,
};
use hw_logistics::transport_request::producer::tile_wait_cache::{
    FloorTileWaitingCache, WallTileWaitingCache,
};
use hw_spatial::blueprint::BlueprintSpatialGrid;
use hw_spatial::designation::DesignationSpatialGrid;
use hw_spatial::familiar::FamiliarSpatialGrid;
use hw_spatial::floor_construction::FloorConstructionSpatialGrid;
use hw_spatial::gathering::GatheringSpotSpatialGrid;
use hw_spatial::resource::ResourceSpatialGrid;
use hw_spatial::soul::SpatialGrid;
use hw_spatial::stockpile::StockpileSpatialGrid;
use hw_spatial::transport_request::TransportRequestSpatialGrid;
use hw_world::room_detection::{RoomDetectionState, RoomTileLookup, RoomValidationState};
use hw_world::{ObstaclePositionIndex, RuntimePathSearchBudget, WalkabilityConnectivityCache};

use crate::app_contexts::{
    BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
    ZoneContext,
};
use crate::systems::energy::grid_recalc::EnergyUpdateDirty;
use crate::systems::familiar_ai::perceive::resource_sync::{
    ReservationSignatureCache, ReservationSyncTimer,
};
use crate::systems::logistics::{ResourceCountDisplayTimer, ResourceCountLabel, ResourceLabels};
use crate::world::map::GeneratedWorldLayoutResource;
use crate::world::regrowth::{RegrowthManager, configure_regrowth_from_generated_layout};

#[derive(Clone, Copy)]
struct LoadResetHook {
    name: &'static str,
    reset: fn(&mut World),
}

/// Explicit list of state owners that must drop stale entity references before
/// a persisted world replacement.
#[derive(Resource, Default)]
pub(crate) struct LoadResetRegistry {
    hooks: Vec<LoadResetHook>,
}

impl LoadResetRegistry {
    fn register(&mut self, name: &'static str, reset: fn(&mut World)) {
        assert!(
            self.hooks.iter().all(|hook| hook.name != name),
            "load reset hook '{name}' was registered more than once"
        );
        self.hooks.push(LoadResetHook { name, reset });
    }

    #[cfg(test)]
    fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.hooks.iter().map(|hook| hook.name)
    }
}

/// Registers a root or leaf reset function during plugin construction.
///
/// This is intentionally a root-only API. Leaf crates expose their reset
/// function without importing this type; their `bevy_app` facade connects it.
pub(crate) fn register_load_reset_hook(app: &mut App, name: &'static str, reset: fn(&mut World)) {
    if !app.world().contains_resource::<LoadResetRegistry>() {
        app.init_resource::<LoadResetRegistry>();
    }
    app.world_mut()
        .resource_mut::<LoadResetRegistry>()
        .register(name, reset);
}

/// Runs all registered reset hooks without retaining a borrow of the registry.
pub(super) fn run_load_resets(world: &mut World) {
    let hooks = world
        .get_resource::<LoadResetRegistry>()
        .map(|registry| registry.hooks.clone())
        .unwrap_or_default();
    for hook in hooks {
        (hook.reset)(world);
    }
}

/// Resets root-owned selection and input contexts.
pub(crate) fn reset_root_interaction_state(world: &mut World) {
    reset_existing_resource::<SelectedEntity>(world);
    reset_existing_resource::<HoveredEntity>(world);
    reset_existing_resource::<BuildContext>(world);
    reset_existing_resource::<MoveContext>(world);
    reset_existing_resource::<MovePlacementState>(world);
    reset_existing_resource::<ZoneContext>(world);
    reset_existing_resource::<TaskContext>(world);
    reset_existing_resource::<CompanionPlacementState>(world);

    if let Some(mut next_play_mode) = world.get_resource_mut::<NextState<PlayMode>>() {
        next_play_mode.set(PlayMode::Normal);
    }
}

/// Drops all caches that contain simulation entity ids and rebuilds only the
/// configuration-derived portion that does not come from the save payload.
///
/// This hook is intentionally idempotent: the finalizer repeats it before
/// rehydration so a fallible partial finalizer cannot leak cache state into a
/// rollback recovery.
pub(crate) fn reset_runtime_caches(world: &mut World) {
    clear_message::<hw_world::TerrainChangedEvent>(world);
    world.insert_resource(SharedResourceCache::default());
    world.insert_resource(ReservationSignatureCache::default());
    world.insert_resource(ReservationSyncTimer::default());
    world.insert_resource(TileSiteIndex::default());
    world.insert_resource(TransportRequestMetrics::default());
    world.insert_resource(
        hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics::default(),
    );
    world.insert_resource(
        hw_logistics::transport_request::arbitration::WheelbarrowArbitrationRuntime::default(),
    );
    world.insert_resource(CachedActiveFamiliars::default());
    world.insert_resource(CachedActiveYards::default());
    world.insert_resource(CachedStockpileGroups::default());
    world.insert_resource(FloorTileWaitingCache::default());
    world.insert_resource(WallTileWaitingCache::default());
    world.insert_resource(RoomDetectionState::default());
    world.insert_resource(RoomTileLookup::default());
    world.insert_resource(RoomValidationState::default());
    world.insert_resource(ObstaclePositionIndex::default());
    world.insert_resource(GatheringSpotSpatialGrid::default());
    world.insert_resource(BlueprintSpatialGrid::default());
    world.insert_resource(DesignationSpatialGrid::default());
    world.insert_resource(FamiliarSpatialGrid::default());
    world.insert_resource(FloorConstructionSpatialGrid::default());
    world.insert_resource(ResourceSpatialGrid::default());
    world.insert_resource(SpatialGrid::default());
    world.insert_resource(StockpileSpatialGrid::default());
    world.insert_resource(TransportRequestSpatialGrid::default());
    world.insert_resource(WalkabilityConnectivityCache::default());
    world.insert_resource(RuntimePathSearchBudget::default());
    world.init_resource::<EnergyUpdateDirty>();
    world
        .resource_mut::<EnergyUpdateDirty>()
        .request_full_rebuild();

    let mut regrowth = RegrowthManager::default();
    if let Some(generated_layout) = world.get_resource::<GeneratedWorldLayoutResource>() {
        configure_regrowth_from_generated_layout(&mut regrowth, &generated_layout.layout);
    }
    world.insert_resource(regrowth);

    clear_resource_count_labels(world);
}

/// Advances the epoch after old persisted entities are gone. System-local
/// entity caches observe the new value before their next use.
pub(super) fn advance_world_epoch(world: &mut World) {
    if !world.contains_resource::<WorldEpoch>() {
        world.init_resource::<WorldEpoch>();
    }
    world.resource_mut::<WorldEpoch>().advance();
}

/// Clears both Bevy 0.19 removed-component message buffers.
///
/// `World::clear_trackers()` calls `Messages::update()`, which swaps its two
/// buffers and clears only the previously inactive one. Calling it twice after
/// `flush()` discards removals produced while deleting the old world. Never
/// call this after writing the replacement: its Added/Changed observations
/// must remain visible to the next frame.
pub(super) fn discard_old_removed_components(world: &mut World) {
    world.clear_trackers();
    world.clear_trackers();
}

fn reset_existing_resource<T: Resource + Default>(world: &mut World) {
    if world.contains_resource::<T>() {
        world.insert_resource(T::default());
    }
}

fn clear_message<T: Message>(world: &mut World) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<T>>() {
        messages.clear();
    }
}

fn clear_resource_count_labels(world: &mut World) {
    let labels: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<ResourceCountLabel>>();
        query.iter(world).collect()
    };
    for entity in labels {
        world.despawn(entity);
    }
    reset_existing_resource::<ResourceLabels>(world);
    reset_existing_resource::<ResourceCountDisplayTimer>(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_contexts::PendingMovePlacement;
    use hw_core::constants::{MAP_HEIGHT, MAX_PATHFINDS_PER_FRAME};
    use hw_core::game_state::TaskMode;
    use hw_world::WorldMap;

    #[derive(Resource, Default)]
    struct ResetCount(u32);

    fn count_reset(world: &mut World) {
        world.resource_mut::<ResetCount>().0 += 1;
    }

    #[test]
    fn registry_runs_each_registered_hook_once() {
        let mut app = App::new();
        app.init_resource::<ResetCount>();
        register_load_reset_hook(&mut app, "test-count", count_reset);

        run_load_resets(app.world_mut());

        assert_eq!(app.world().resource::<ResetCount>().0, 1);
        assert_eq!(
            app.world()
                .resource::<LoadResetRegistry>()
                .names()
                .collect::<Vec<_>>(),
            vec!["test-count"]
        );
    }

    #[test]
    #[should_panic(expected = "registered more than once")]
    fn registry_rejects_duplicate_hook_names() {
        let mut app = App::new();
        register_load_reset_hook(&mut app, "duplicate", count_reset);
        register_load_reset_hook(&mut app, "duplicate", count_reset);
    }

    #[test]
    fn world_epoch_advances_once_per_replace_boundary() {
        let mut world = World::new();
        advance_world_epoch(&mut world);
        assert_eq!(world.resource::<WorldEpoch>().get(), 1);
    }

    #[test]
    fn root_interaction_reset_drops_stale_entity_references() {
        let mut world = World::new();
        let stale = world.spawn_empty().id();
        world.insert_resource(SelectedEntity(Some(stale)));
        world.insert_resource(HoveredEntity(Some(stale)));
        world.insert_resource(MoveContext(Some(stale)));
        world.insert_resource(MovePlacementState(Some(PendingMovePlacement {
            building: stale,
            destination_grid: (4, 5),
        })));
        world.insert_resource(TaskContext::default());
        world.insert_resource(NextState::<PlayMode>::Pending(PlayMode::BuildingMove));

        reset_root_interaction_state(&mut world);

        assert!(world.resource::<SelectedEntity>().0.is_none());
        assert!(world.resource::<HoveredEntity>().0.is_none());
        assert!(world.resource::<MoveContext>().0.is_none());
        assert!(world.resource::<MovePlacementState>().0.is_none());
        assert!(matches!(
            world.resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::Normal)
        ));
    }

    #[test]
    fn world_replace_clears_zone_placement_mode() {
        let mut world = World::new();
        world.insert_resource(ZoneContext(Some(
            crate::systems::logistics::ZoneType::Stockpile,
        )));
        world.insert_resource(TaskContext(TaskMode::ZonePlacement(
            hw_core::game_state::TaskModeZoneType::Stockpile,
            Some(Vec2::ZERO),
        )));
        world.insert_resource(NextState::<PlayMode>::Pending(PlayMode::TaskDesignation));

        reset_root_interaction_state(&mut world);

        assert!(world.resource::<ZoneContext>().0.is_none());
        assert_eq!(world.resource::<TaskContext>().0, TaskMode::None);
        assert!(matches!(
            world.resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::Normal) | NextState::PendingIfNeq(PlayMode::Normal)
        ));
    }

    #[test]
    fn runtime_cache_reset_drops_connectivity_and_path_budget_for_a_reused_map_version() {
        let mut blocked_map = WorldMap::default();
        for y in 0..MAP_HEIGHT {
            blocked_map.add_grid_obstacle((50, y));
        }

        let start = (25, 50);
        let target = (75, 50);
        let mut world = World::new();
        world.insert_resource(blocked_map);
        world.insert_resource(WalkabilityConnectivityCache::default());
        world.insert_resource(RuntimePathSearchBudget::new(1));
        assert!(world.resource_mut::<RuntimePathSearchBudget>().try_claim());
        world.resource_scope(|world, mut cache: Mut<WalkabilityConnectivityCache>| {
            let map = world.resource::<WorldMap>();
            assert!(!cache.can_reach_target(map, start, target, true));
        });

        let obstacle_version = world.resource::<WorldMap>().obstacle_version;
        let loaded_map = WorldMap {
            obstacle_version,
            ..Default::default()
        };
        world.insert_resource(loaded_map);

        reset_runtime_caches(&mut world);

        let budget = world.resource::<RuntimePathSearchBudget>();
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.hard_limit(), MAX_PATHFINDS_PER_FRAME);

        world.resource_scope(|world, mut cache: Mut<WalkabilityConnectivityCache>| {
            let map = world.resource::<WorldMap>();
            assert!(cache.can_reach_target(map, start, target, true));
        });
    }

    #[test]
    fn runtime_cache_reset_requests_one_energy_rebuild_after_load() {
        let mut world = World::new();

        reset_runtime_caches(&mut world);

        let dirty = world.resource::<EnergyUpdateDirty>();
        assert!(dirty.power_output_due);
        assert!(dirty.grid_recalc_due);
    }
}
