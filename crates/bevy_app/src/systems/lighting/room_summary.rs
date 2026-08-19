use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::WorldEpoch;
use hw_infra::lighting::{
    LightGridPos, RoomIlluminationSummary, RoomSummaryError, summarize_room_illumination,
};
use hw_world::{Room, RoomTileLookup, RoomTileSignature};

use super::{IndoorLightRuntime, IndoorLightingLifecycleProbe, read_indoor_light_snapshot};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoulLightRecoverySet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomIlluminationSummarySet;

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct IndoorLightConsumerMetrics {
    pub recovery_updates: u64,
    pub unavailable_recovery_updates: u64,
    pub recovery_steps: u64,
    pub soul_samples: u64,
    pub recovery_effects: u64,
    pub old_epoch_recovery_effects: u64,
    pub dark_samples: u64,
    pub out_of_bounds_samples: u64,
    pub room_summary_updates: u64,
    pub room_summary_recomputes: u64,
    pub room_summary_cache_hits: u64,
    pub room_sampled_cells: u64,
    pub invalid_room_summaries: u64,
    pub removed_stale_room_states: u64,
    pub old_epoch_room_read_attempts: u64,
    pub old_epoch_room_reads: u64,
}

#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IndoorLightCrossConsumerObservation {
    world_epoch: Option<u64>,
    field_revision: Option<u64>,
    recovery_steps: u32,
    soul_count: u32,
    sample_count: u64,
    effect_count: u64,
    mask_or_stale_effects: u64,
}

#[cfg(feature = "profiling")]
impl IndoorLightCrossConsumerObservation {
    pub const fn world_epoch(self) -> Option<u64> {
        self.world_epoch
    }

    pub const fn field_revision(self) -> Option<u64> {
        self.field_revision
    }

    pub const fn recovery_steps(self) -> u32 {
        self.recovery_steps
    }

    pub const fn soul_count(self) -> u32 {
        self.soul_count
    }

    pub const fn sample_count(self) -> u64 {
        self.sample_count
    }

    pub const fn effect_count(self) -> u64 {
        self.effect_count
    }

    pub const fn mask_or_stale_effects(self) -> u64 {
        self.mask_or_stale_effects
    }

    pub(crate) fn record_recovery_step(
        &mut self,
        world_epoch: u64,
        field_revision: u64,
        recovery_steps: u32,
        sample_count: u64,
        effect_count: u64,
        mask_or_stale_effects: u64,
    ) {
        let Some(soul_count) = sample_count.checked_div(u64::from(recovery_steps)) else {
            *self = Self::default();
            return;
        };
        self.world_epoch = Some(world_epoch);
        self.field_revision = Some(field_revision);
        self.recovery_steps = recovery_steps;
        self.soul_count = u32::try_from(soul_count).unwrap_or(u32::MAX);
        self.sample_count = sample_count;
        self.effect_count = effect_count;
        self.mask_or_stale_effects = mask_or_stale_effects;
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RoomIlluminationState {
    world_epoch: u64,
    field_revision: u64,
    room_topology_revision: u64,
    room_tile_signature: RoomTileSignature,
    summary: RoomIlluminationSummary,
}

impl RoomIlluminationState {
    pub const fn world_epoch(&self) -> u64 {
        self.world_epoch
    }

    pub const fn field_revision(&self) -> u64 {
        self.field_revision
    }

    pub const fn room_topology_revision(&self) -> u64 {
        self.room_topology_revision
    }

    pub const fn room_tile_signature(&self) -> &RoomTileSignature {
        &self.room_tile_signature
    }

    pub const fn summary(&self) -> &RoomIlluminationSummary {
        &self.summary
    }

    fn matches(
        &self,
        world_epoch: WorldEpoch,
        field_revision: u64,
        room_topology_revision: u64,
        room_tile_signature: &RoomTileSignature,
    ) -> bool {
        self.world_epoch == world_epoch.get()
            && self.field_revision == field_revision
            && self.room_topology_revision == room_topology_revision
            && &self.room_tile_signature == room_tile_signature
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RoomIlluminationCacheKey {
    world_epoch: u64,
    field_revision: u64,
    room_topology_revision: u64,
    room_tile_signature: RoomTileSignature,
}

#[derive(Resource, Debug, Default)]
pub struct RoomIlluminationCache {
    entries: HashMap<RoomIlluminationCacheKey, RoomIlluminationSummary>,
}

impl RoomIlluminationCache {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub fn read_room_illumination_state<'a>(
    state: Option<&'a RoomIlluminationState>,
    requested_epoch: WorldEpoch,
    current_epoch: WorldEpoch,
    current_field_revision: u64,
    current_topology_revision: u64,
    room_tile_signature: &RoomTileSignature,
    metrics: &mut IndoorLightConsumerMetrics,
) -> Option<&'a RoomIlluminationState> {
    if requested_epoch != current_epoch {
        metrics.old_epoch_room_read_attempts =
            metrics.old_epoch_room_read_attempts.saturating_add(1);
        return None;
    }
    state.filter(|state| {
        state.matches(
            current_epoch,
            current_field_revision,
            current_topology_revision,
            room_tile_signature,
        )
    })
}

type RoomSummaryQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Room,
        Option<&'static RoomIlluminationState>,
    ),
>;

#[derive(SystemParam)]
pub struct RoomIlluminationSummaryContext<'w> {
    runtime: Res<'w, IndoorLightRuntime>,
    world_epoch: Res<'w, WorldEpoch>,
    room_lookup: Res<'w, RoomTileLookup>,
    lifecycle_probe: ResMut<'w, IndoorLightingLifecycleProbe>,
    cache: ResMut<'w, RoomIlluminationCache>,
    metrics: ResMut<'w, IndoorLightConsumerMetrics>,
}

pub fn update_room_illumination_summaries_system(
    mut commands: Commands,
    mut context: RoomIlluminationSummaryContext,
    rooms: RoomSummaryQuery,
) {
    let RoomIlluminationSummaryContext {
        runtime,
        world_epoch,
        room_lookup,
        lifecycle_probe,
        cache,
        metrics,
    } = &mut context;
    metrics.room_summary_updates = metrics.room_summary_updates.saturating_add(1);
    let Some(snapshot) =
        read_indoor_light_snapshot(runtime, **world_epoch, **world_epoch, lifecycle_probe)
    else {
        cache.entries.clear();
        for (entity, _, state) in &rooms {
            if state.is_some() {
                commands.entity(entity).remove::<RoomIlluminationState>();
                metrics.removed_stale_room_states =
                    metrics.removed_stale_room_states.saturating_add(1);
            }
        }
        return;
    };

    let field_revision = snapshot.field_revision();
    let topology_revision = room_lookup.topology_signature().revision();
    cache.entries.retain(|key, _| {
        key.world_epoch == world_epoch.get()
            && key.field_revision == field_revision
            && key.room_topology_revision == topology_revision
    });

    for (entity, room, current_state) in &rooms {
        let key = RoomIlluminationCacheKey {
            world_epoch: world_epoch.get(),
            field_revision,
            room_topology_revision: topology_revision,
            room_tile_signature: room.tile_signature.clone(),
        };
        if current_state.is_some_and(|state| {
            state.matches(
                **world_epoch,
                field_revision,
                topology_revision,
                &room.tile_signature,
            )
        }) {
            continue;
        }

        let summary = if let Some(summary) = cache.entries.get(&key).copied() {
            metrics.room_summary_cache_hits = metrics.room_summary_cache_hits.saturating_add(1);
            summary
        } else {
            let result = summarize_room_illumination(
                room.tiles
                    .iter()
                    .map(|&(x, y)| snapshot.sample_room_cell(LightGridPos::new(x, y))),
            );
            let summary = match result {
                Ok(summary) => summary,
                Err(
                    RoomSummaryError::Empty
                    | RoomSummaryError::InvalidSample(_)
                    | RoomSummaryError::TooManySamples,
                ) => {
                    metrics.invalid_room_summaries =
                        metrics.invalid_room_summaries.saturating_add(1);
                    commands.entity(entity).remove::<RoomIlluminationState>();
                    continue;
                }
            };
            metrics.room_summary_recomputes = metrics.room_summary_recomputes.saturating_add(1);
            metrics.room_sampled_cells = metrics
                .room_sampled_cells
                .saturating_add(u64::from(summary.sample_count));
            cache.entries.insert(key.clone(), summary);
            summary
        };

        commands.entity(entity).insert(RoomIlluminationState {
            world_epoch: key.world_epoch,
            field_revision: key.field_revision,
            room_topology_revision: key.room_topology_revision,
            room_tile_signature: key.room_tile_signature,
            summary,
        });
    }
}

pub(crate) fn reset_indoor_light_consumers_for_world_replace(world: &mut World) {
    let state_entities = {
        let mut query = world.query_filtered::<Entity, With<RoomIlluminationState>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for entity in state_entities {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.remove::<RoomIlluminationState>();
        }
    }
    world.insert_resource(RoomIlluminationCache::default());
    world.insert_resource(IndoorLightConsumerMetrics::default());
    #[cfg(feature = "profiling")]
    world.insert_resource(IndoorLightCrossConsumerObservation::default());
}
