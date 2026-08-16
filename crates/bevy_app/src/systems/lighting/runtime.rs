use std::fmt;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::WorldEpoch;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::DoorState;
use hw_energy::PowerSupplyState;
use hw_infra::lighting::{
    CardinalDirection, FieldSnapshot, FixtureMount, GridDimensions, IndoorMask, LightFieldError,
    LightFieldInput, LightGridPos, LightOcclusionGrid, OcclusionCell, RadialLightEmitterSnapshot,
    Sha256Digest, digest_hex, rebuild_field,
};
use hw_jobs::{Building, BuildingType, Door, ProvisionalWall};
use hw_world::{DoorLockToggleRequest, RoomTileLookup, WorldMap, apply_door_state};

use super::{LightingFixtureMount, RadialLightEmitter};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DoorManualMutationSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndoorLightingEmitterSyncSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndoorLightingDirtyCollectSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndoorLightingCollectSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndoorLightingRebuildSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IndoorLightAvailability {
    #[default]
    Initializing,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndoorLightingMetrics {
    pub topology_dirty_updates: u64,
    pub emitter_dirty_updates: u64,
    pub room_mask_dirty_updates: u64,
    pub snapshot_collect_count: u64,
    pub full_snapshot_scan_count: u64,
    pub field_rebuild_count: u64,
    pub output_revision_increment_count: u64,
    pub failed_update_count: u64,
    pub max_rebuilds_per_update: u32,
    pub last_changed_cell_count: u32,
}

#[derive(Resource, Debug, Default)]
pub struct IndoorLightingAllocationProbe {
    enabled: bool,
    emitter_collect_events: Option<u64>,
    emitter_collect_bytes: Option<u64>,
}

impl IndoorLightingAllocationProbe {
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub const fn emitter_collect_allocation_events(&self) -> Option<u64> {
        self.emitter_collect_events
    }

    pub const fn emitter_collect_allocation_bytes(&self) -> Option<u64> {
        self.emitter_collect_bytes
    }
}

#[derive(Resource, Debug, Clone)]
pub struct IndoorLightRuntime {
    availability: IndoorLightAvailability,
    snapshot: Option<FieldSnapshot>,
    pending_input: Option<LightFieldInput>,
    last_input_checksum: Option<Sha256Digest>,
    input_revision: u64,
    published_epoch: Option<u64>,
    last_error: Option<String>,
    typed_emitter_components: u32,
    eligible_supplied_emitters: u32,
    metrics: IndoorLightingMetrics,
}

impl Default for IndoorLightRuntime {
    fn default() -> Self {
        Self {
            availability: IndoorLightAvailability::Initializing,
            snapshot: None,
            pending_input: None,
            last_input_checksum: None,
            input_revision: 0,
            published_epoch: None,
            last_error: None,
            typed_emitter_components: 0,
            eligible_supplied_emitters: 0,
            metrics: IndoorLightingMetrics::default(),
        }
    }
}

impl IndoorLightRuntime {
    pub const fn availability(&self) -> IndoorLightAvailability {
        self.availability
    }

    pub fn snapshot(&self) -> Option<&FieldSnapshot> {
        if self.availability == IndoorLightAvailability::Available {
            self.snapshot.as_ref()
        } else {
            None
        }
    }

    pub fn snapshot_for_epoch(&self, world_epoch: WorldEpoch) -> Option<&FieldSnapshot> {
        (self.published_epoch == Some(world_epoch.get()))
            .then(|| self.snapshot())
            .flatten()
    }

    pub const fn published_epoch(&self) -> Option<u64> {
        self.published_epoch
    }

    pub const fn input_revision(&self) -> u64 {
        self.input_revision
    }

    pub fn output_revision(&self) -> u64 {
        self.snapshot
            .as_ref()
            .map_or(0, FieldSnapshot::field_revision)
    }

    pub fn field_checksum_hex(&self) -> Option<String> {
        self.snapshot()
            .map(|snapshot| digest_hex(snapshot.field_checksum()))
    }

    pub fn mask_checksum_hex(&self) -> Option<String> {
        self.snapshot()
            .map(|snapshot| digest_hex(snapshot.mask_checksum()))
    }

    pub fn indoor_mask_cells(&self) -> Option<u32> {
        self.snapshot().and_then(|snapshot| {
            u32::try_from(
                snapshot
                    .indoor_mask_bytes()
                    .iter()
                    .filter(|&&value| value != 0)
                    .count(),
            )
            .ok()
        })
    }

    pub fn is_dark(&self) -> Option<bool> {
        self.snapshot()
            .map(|snapshot| snapshot.cells().iter().all(|cell| cell.luminance == 0))
    }

    pub fn is_fail_dark(&self) -> bool {
        self.availability == IndoorLightAvailability::Unavailable && self.snapshot.is_none()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub const fn typed_emitter_components(&self) -> u32 {
        self.typed_emitter_components
    }

    pub const fn eligible_supplied_emitters(&self) -> u32 {
        self.eligible_supplied_emitters
    }

    pub const fn metrics(&self) -> &IndoorLightingMetrics {
        &self.metrics
    }

    pub(crate) fn reset_for_world_replace(&mut self) {
        self.availability = IndoorLightAvailability::Unavailable;
        self.snapshot = None;
        self.pending_input = None;
        self.last_input_checksum = None;
        self.input_revision = 0;
        self.published_epoch = None;
        self.last_error = Some("world replacement in progress".to_owned());
        self.typed_emitter_components = 0;
        self.eligible_supplied_emitters = 0;
        self.metrics = IndoorLightingMetrics::default();
    }

    fn publish_unavailable(&mut self, error: impl fmt::Display) {
        self.availability = IndoorLightAvailability::Unavailable;
        self.snapshot = None;
        self.pending_input = None;
        self.last_input_checksum = None;
        self.published_epoch = None;
        self.last_error = Some(error.to_string());
        self.metrics.failed_update_count = self.metrics.failed_update_count.saturating_add(1);
    }
}

#[derive(Resource, Debug)]
pub struct IndoorLightingDirty {
    topology: bool,
    emitters: bool,
    room_mask: bool,
    last_room_mask_revision: u64,
}

impl Default for IndoorLightingDirty {
    fn default() -> Self {
        Self {
            topology: true,
            emitters: true,
            room_mask: true,
            last_room_mask_revision: 0,
        }
    }
}

impl IndoorLightingDirty {
    fn any(&self) -> bool {
        self.topology || self.emitters || self.room_mask
    }

    fn clear(&mut self) {
        self.topology = false;
        self.emitters = false;
        self.room_mask = false;
    }

    pub(crate) fn request_full_rebuild(&mut self) {
        self.topology = true;
        self.emitters = true;
        self.room_mask = true;
    }

    pub(crate) fn reset_for_world_replace(&mut self) {
        self.topology = false;
        self.emitters = false;
        self.room_mask = false;
        self.last_room_mask_revision = 0;
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct IndoorLightingLifecycleProbe {
    enabled: bool,
    reset_count: u64,
    wake_count: u64,
    field_read_count: u64,
    old_epoch_field_read_attempts: u64,
    old_epoch_field_reads: u64,
    all_resets_fail_dark: bool,
}

impl Default for IndoorLightingLifecycleProbe {
    fn default() -> Self {
        Self {
            enabled: false,
            reset_count: 0,
            wake_count: 0,
            field_read_count: 0,
            old_epoch_field_read_attempts: 0,
            old_epoch_field_reads: 0,
            all_resets_fail_dark: true,
        }
    }
}

impl IndoorLightingLifecycleProbe {
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub const fn reset_count(&self) -> u64 {
        self.reset_count
    }

    pub const fn wake_count(&self) -> u64 {
        self.wake_count
    }

    pub const fn field_read_count(&self) -> u64 {
        self.field_read_count
    }

    pub const fn old_epoch_field_read_attempts(&self) -> u64 {
        self.old_epoch_field_read_attempts
    }

    pub const fn old_epoch_field_reads(&self) -> u64 {
        self.old_epoch_field_reads
    }

    pub const fn all_resets_fail_dark(&self) -> bool {
        self.all_resets_fail_dark
    }

    pub(crate) fn record_reset(&mut self, fail_dark: bool) {
        if self.enabled {
            self.reset_count = self.reset_count.saturating_add(1);
            self.all_resets_fail_dark &= fail_dark;
        }
    }

    pub(crate) fn record_wake(&mut self) {
        if self.enabled {
            self.wake_count = self.wake_count.saturating_add(1);
        }
    }

    fn record_read(&mut self, requested: WorldEpoch, current: WorldEpoch, returned: bool) {
        if !self.enabled {
            return;
        }
        self.field_read_count = self.field_read_count.saturating_add(1);
        if requested != current {
            self.old_epoch_field_read_attempts =
                self.old_epoch_field_read_attempts.saturating_add(1);
            if returned {
                self.old_epoch_field_reads = self.old_epoch_field_reads.saturating_add(1);
            }
        }
    }
}

pub fn read_indoor_light_snapshot<'a>(
    runtime: &'a IndoorLightRuntime,
    requested_epoch: WorldEpoch,
    current_epoch: WorldEpoch,
    probe: &mut IndoorLightingLifecycleProbe,
) -> Option<&'a FieldSnapshot> {
    let snapshot = runtime.snapshot_for_epoch(requested_epoch);
    probe.record_read(requested_epoch, current_epoch, snapshot.is_some());
    snapshot
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct DoorLockToggleMetrics {
    pub applied: u64,
    pub stale_target: u64,
    pub invalid_target: u64,
    pub wrong_world_owner: u64,
}

type DoorMutationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building,
        &'static Transform,
        &'static mut Door,
    ),
>;

pub fn consume_door_lock_toggle_requests_system(
    mut requests: MessageReader<DoorLockToggleRequest>,
    mut doors: DoorMutationQuery,
    entities: Query<(), ()>,
    mut world_map: ResMut<WorldMap>,
    mut metrics: ResMut<DoorLockToggleMetrics>,
) {
    for request in requests.read() {
        let Ok((owner, building, transform, mut door)) = doors.get_mut(request.owner) else {
            if entities.get(request.owner).is_ok() {
                metrics.invalid_target = metrics.invalid_target.saturating_add(1);
            } else {
                metrics.stale_target = metrics.stale_target.saturating_add(1);
            }
            continue;
        };
        if building.kind != BuildingType::Door || building.is_provisional {
            metrics.invalid_target = metrics.invalid_target.saturating_add(1);
            continue;
        }
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        if world_map.door_entity(grid.0, grid.1) != Some(owner) {
            metrics.wrong_world_owner = metrics.wrong_world_owner.saturating_add(1);
            continue;
        }
        let next_state = if door.state == DoorState::Locked {
            DoorState::Closed
        } else {
            DoorState::Locked
        };
        apply_door_state(&mut door, &mut world_map, grid, next_state);
        metrics.applied = metrics.applied.saturating_add(1);
    }
}

type ChangedBuildingQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<Building>,
        Or<(Added<Building>, Changed<Building>, Changed<Transform>)>,
    ),
>;
type ChangedDoorQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<Door>,
        Or<(Added<Door>, Changed<Door>, Changed<Transform>)>,
    ),
>;
type ChangedEmitterQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<RadialLightEmitter>,
        Or<(
            Added<RadialLightEmitter>,
            Changed<RadialLightEmitter>,
            Changed<Transform>,
            Changed<PowerSupplyState>,
        )>,
    ),
>;

#[derive(SystemParam)]
pub struct IndoorLightingDirtyInputs<'w, 's> {
    changed_buildings: ChangedBuildingQuery<'w, 's>,
    changed_doors: ChangedDoorQuery<'w, 's>,
    changed_emitters: ChangedEmitterQuery<'w, 's>,
    existing_emitters: Query<'w, 's, (), With<RadialLightEmitter>>,
    removed_buildings: RemovedComponents<'w, 's, Building>,
    removed_doors: RemovedComponents<'w, 's, Door>,
    removed_emitters: RemovedComponents<'w, 's, RadialLightEmitter>,
    removed_supply: RemovedComponents<'w, 's, PowerSupplyState>,
    room_lookup: Res<'w, RoomTileLookup>,
}

pub fn mark_indoor_lighting_dirty_system(
    mut inputs: IndoorLightingDirtyInputs,
    mut dirty: ResMut<IndoorLightingDirty>,
    mut runtime: ResMut<IndoorLightRuntime>,
) {
    let building_removed = inputs.removed_buildings.read().count() != 0;
    let door_removed = inputs.removed_doors.read().count() != 0;
    if inputs.changed_buildings.iter().next().is_some()
        || inputs.changed_doors.iter().next().is_some()
        || building_removed
        || door_removed
    {
        dirty.topology = true;
        runtime.metrics.topology_dirty_updates =
            runtime.metrics.topology_dirty_updates.saturating_add(1);
    }

    let emitter_removed = inputs.removed_emitters.read().count() != 0;
    let supply_removed = inputs
        .removed_supply
        .read()
        .filter(|entity| inputs.existing_emitters.get(*entity).is_ok())
        .count()
        != 0;
    if inputs.changed_emitters.iter().next().is_some() || emitter_removed || supply_removed {
        dirty.emitters = true;
        runtime.metrics.emitter_dirty_updates =
            runtime.metrics.emitter_dirty_updates.saturating_add(1);
    }

    let room_revision = inputs.room_lookup.mask_signature().revision();
    if room_revision != dirty.last_room_mask_revision {
        dirty.last_room_mask_revision = room_revision;
        dirty.room_mask = true;
        runtime.metrics.room_mask_dirty_updates =
            runtime.metrics.room_mask_dirty_updates.saturating_add(1);
    }
}

/// Reconstructs P04's runtime-only emitter component after loading a completed
/// OutdoorLamp. P05 owns persisted mount registration; P04 derives the v1
/// free-standing mount from the authoritative building root.
type OutdoorLampSyncQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building,
        &'static Transform,
        Option<&'static LightingFixtureMount>,
        Option<&'static RadialLightEmitter>,
    ),
    Or<(
        Added<Building>,
        Changed<Building>,
        Changed<Transform>,
        Added<LightingFixtureMount>,
        Changed<LightingFixtureMount>,
        Without<LightingFixtureMount>,
        Without<RadialLightEmitter>,
    )>,
>;

pub fn sync_outdoor_lamp_emitters_system(mut commands: Commands, lamps: OutdoorLampSyncQuery) {
    for (entity, building, transform, mount, emitter) in &lamps {
        if building.kind != BuildingType::OutdoorLamp || building.is_provisional {
            continue;
        }
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        let free_standing = LightingFixtureMount::free_standing(grid);
        let desired_mount = match mount.map(|mount| mount.mount()) {
            Some(FixtureMount::WallMounted { anchor, inward }) => {
                FixtureMount::WallMounted { anchor, inward }
            }
            Some(FixtureMount::FreeStanding { .. }) | None => free_standing.mount(),
        };
        let desired_adapter = LightingFixtureMount(desired_mount);
        let desired_emitter = RadialLightEmitter::outdoor_lamp_at_mount(desired_mount);
        let adapter_changed = mount.is_none_or(|mount| *mount != desired_adapter);
        let emitter_changed = emitter.is_none_or(|emitter| *emitter != desired_emitter);
        let mut entity_commands = commands.entity(entity);
        if adapter_changed && emitter_changed {
            entity_commands.insert((desired_adapter, desired_emitter));
        } else if adapter_changed {
            entity_commands.insert(desired_adapter);
        } else if emitter_changed {
            entity_commands.insert(desired_emitter);
        }
    }
}

type BuildingSnapshotQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building,
        &'static Transform,
        Option<&'static ProvisionalWall>,
    ),
>;
type DoorSnapshotQuery<'w, 's> =
    Query<'w, 's, (&'static Building, &'static Door, &'static Transform)>;
type EmitterSnapshotQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RadialLightEmitter,
        &'static Transform,
        Option<&'static PowerSupplyState>,
        Option<&'static Building>,
    ),
>;

#[derive(SystemParam)]
pub struct IndoorLightingSnapshotInputs<'w, 's> {
    buildings: BuildingSnapshotQuery<'w, 's>,
    doors: DoorSnapshotQuery<'w, 's>,
    emitters: EmitterSnapshotQuery<'w, 's>,
    room_lookup: Res<'w, RoomTileLookup>,
}

pub fn collect_indoor_lighting_snapshot_system(
    inputs: IndoorLightingSnapshotInputs,
    mut dirty: ResMut<IndoorLightingDirty>,
    mut runtime: ResMut<IndoorLightRuntime>,
    mut allocation_probe: ResMut<IndoorLightingAllocationProbe>,
) {
    if !dirty.any() {
        return;
    }
    runtime.metrics.snapshot_collect_count =
        runtime.metrics.snapshot_collect_count.saturating_add(1);
    runtime.metrics.full_snapshot_scan_count =
        runtime.metrics.full_snapshot_scan_count.saturating_add(1);

    let collected = build_input(&inputs);
    if allocation_probe.enabled
        && let Ok(collected) = &collected
    {
        allocation_probe.emitter_collect_events = Some(collected.allocation_events);
        allocation_probe.emitter_collect_bytes = Some(collected.allocation_bytes);
    }
    match collected {
        Ok(collected) => {
            runtime.typed_emitter_components = collected.typed_emitter_components;
            runtime.eligible_supplied_emitters = collected.eligible_supplied_emitters;
            runtime.pending_input = Some(collected.input);
            runtime.last_error = None;
        }
        Err(error) => runtime.publish_unavailable(error),
    }
    dirty.clear();
}

pub fn rebuild_indoor_lighting_field_system(
    mut runtime: ResMut<IndoorLightRuntime>,
    world_epoch: Res<WorldEpoch>,
) {
    let Some(input) = runtime.pending_input.take() else {
        return;
    };
    runtime.metrics.field_rebuild_count = runtime.metrics.field_rebuild_count.saturating_add(1);
    runtime.metrics.max_rebuilds_per_update = runtime.metrics.max_rebuilds_per_update.max(1);
    let previous_revision = runtime.output_revision();
    let result = rebuild_field(runtime.snapshot.as_ref(), &input);
    match result {
        Ok(outcome) if outcome.diagnostics.is_empty() => {
            if runtime.last_input_checksum != Some(outcome.input_checksum) {
                let Some(next_revision) = runtime.input_revision.checked_add(1) else {
                    runtime.publish_unavailable("indoor lighting input revision overflow");
                    return;
                };
                runtime.input_revision = next_revision;
            }
            runtime.last_input_checksum = Some(outcome.input_checksum);
            runtime.snapshot = Some(outcome.snapshot);
            runtime.published_epoch = Some(world_epoch.get());
            runtime.metrics.last_changed_cell_count = runtime
                .snapshot
                .as_ref()
                .map_or(0, FieldSnapshot::changed_cell_count);
            runtime.availability = IndoorLightAvailability::Available;
            runtime.last_error = None;
            if runtime.output_revision() != previous_revision {
                runtime.metrics.output_revision_increment_count = runtime
                    .metrics
                    .output_revision_increment_count
                    .saturating_add(1);
            }
        }
        Ok(outcome) => runtime.publish_unavailable(format!(
            "{} invalid emitter diagnostic(s); first={:?}",
            outcome.diagnostics.len(),
            outcome.diagnostics.first()
        )),
        Err(error) => runtime.publish_unavailable(error),
    }
}

struct CollectedInput {
    input: LightFieldInput,
    typed_emitter_components: u32,
    eligible_supplied_emitters: u32,
    allocation_events: u64,
    allocation_bytes: u64,
}

fn build_input(inputs: &IndoorLightingSnapshotInputs) -> Result<CollectedInput, RuntimeInputError> {
    let dimensions = GridDimensions::new(
        u16::try_from(MAP_WIDTH).map_err(|_| RuntimeInputError::InvalidDimensions)?,
        u16::try_from(MAP_HEIGHT).map_err(|_| RuntimeInputError::InvalidDimensions)?,
    )?;
    let mut topology = Vec::new();
    for (building, transform, provisional_marker) in inputs.buildings.iter() {
        if building.kind != BuildingType::Wall {
            continue;
        }
        if building.is_provisional != provisional_marker.is_some() {
            warn!(
                "INDOOR_LIGHTING: Wall provisional marker disagrees with Building.is_provisional"
            );
        }
        let position = checked_grid_pos(dimensions, transform, "Wall")?;
        let cell = if building.is_provisional {
            OcclusionCell::ProvisionalWall
        } else {
            OcclusionCell::CompletedWall
        };
        topology.push((position, cell));
    }
    for (building, door, transform) in inputs.doors.iter() {
        if building.kind != BuildingType::Door || building.is_provisional {
            return Err(RuntimeInputError::InvalidDoorRoot);
        }
        let position = checked_grid_pos(dimensions, transform, "Door")?;
        let cell = match door.state {
            DoorState::Open => OcclusionCell::OpenDoor,
            DoorState::Closed => OcclusionCell::ClosedDoor,
            DoorState::Locked => OcclusionCell::LockedDoor,
        };
        topology.push((position, cell));
    }
    topology
        .sort_unstable_by_key(|(position, cell)| (position.y, position.x, cell.canonical_byte()));
    let topology_allocation_bytes =
        allocation_bytes::<(LightGridPos, OcclusionCell)>(topology.capacity())?;
    let topology_allocation_event = u64::from(topology.capacity() != 0);
    let mut occlusion_cells = vec![OcclusionCell::Clear; dimensions.area()];
    let mut occupied = vec![false; dimensions.area()];
    for (position, cell) in topology {
        let index = dimensions
            .index(position)
            .ok_or(RuntimeInputError::OutOfBounds {
                source: "occlusion",
                position,
            })?;
        if occupied[index] {
            return Err(RuntimeInputError::DuplicateOcclusionCell(position));
        }
        occupied[index] = true;
        occlusion_cells[index] = cell;
    }

    let mut indoor_mask = vec![0_u8; dimensions.area()];
    for &grid in inputs.room_lookup.mask_signature().canonical_tiles() {
        let position = LightGridPos::new(grid.0, grid.1);
        let index = dimensions
            .index(position)
            .ok_or(RuntimeInputError::OutOfBounds {
                source: "Room mask",
                position,
            })?;
        indoor_mask[index] = 1;
    }

    let typed_emitter_components = u32::try_from(inputs.emitters.iter().count())
        .map_err(|_| RuntimeInputError::CountOverflow)?;
    let mut emitters = Vec::new();
    for (emitter, transform, supply, building) in inputs.emitters.iter() {
        let building = building.ok_or(RuntimeInputError::InvalidEmitterOwner)?;
        if building.kind != BuildingType::OutdoorLamp || building.is_provisional {
            return Err(RuntimeInputError::InvalidEmitterOwner);
        }
        if supply != Some(&PowerSupplyState::Supplied) {
            continue;
        }
        let transform_grid = checked_grid_pos(dimensions, transform, "emitter")?;
        if transform_grid != emitter.mount.origin() {
            return Err(RuntimeInputError::EmitterTransformMismatch {
                transform: transform_grid,
                mount: emitter.mount.origin(),
            });
        }
        emitters.push(RadialLightEmitterSnapshot {
            stable_key: stable_emitter_key(dimensions, emitter.mount)?,
            mount: emitter.mount,
            radius_tiles: emitter.radius_tiles,
            color: emitter.color,
            intensity: emitter.intensity,
        });
    }
    emitters.sort_unstable_by_key(|emitter| emitter.stable_key);
    let eligible_supplied_emitters =
        u32::try_from(emitters.len()).map_err(|_| RuntimeInputError::CountOverflow)?;
    let allocation_events = topology_allocation_event
        + u64::from(occlusion_cells.capacity() != 0)
        + u64::from(occupied.capacity() != 0)
        + u64::from(indoor_mask.capacity() != 0)
        + u64::from(emitters.capacity() != 0);
    let allocation_bytes = topology_allocation_bytes
        .checked_add(allocation_bytes::<OcclusionCell>(
            occlusion_cells.capacity(),
        )?)
        .and_then(|bytes| bytes.checked_add(allocation_bytes::<bool>(occupied.capacity()).ok()?))
        .and_then(|bytes| bytes.checked_add(allocation_bytes::<u8>(indoor_mask.capacity()).ok()?))
        .and_then(|bytes| {
            bytes.checked_add(
                allocation_bytes::<RadialLightEmitterSnapshot>(emitters.capacity()).ok()?,
            )
        })
        .ok_or(RuntimeInputError::CountOverflow)?;
    let input = LightFieldInput::new(
        dimensions,
        IndoorMask::from_bytes(dimensions, indoor_mask)?,
        LightOcclusionGrid::from_cells(dimensions, occlusion_cells)?,
        emitters,
    )?;
    Ok(CollectedInput {
        input,
        typed_emitter_components,
        eligible_supplied_emitters,
        allocation_events,
        allocation_bytes,
    })
}

fn allocation_bytes<T>(capacity: usize) -> Result<u64, RuntimeInputError> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(RuntimeInputError::CountOverflow)
}

fn checked_grid_pos(
    dimensions: GridDimensions,
    transform: &Transform,
    source: &'static str,
) -> Result<LightGridPos, RuntimeInputError> {
    let grid = WorldMap::world_to_grid(transform.translation.truncate());
    let position = LightGridPos::new(grid.0, grid.1);
    dimensions
        .contains(position)
        .then_some(position)
        .ok_or(RuntimeInputError::OutOfBounds { source, position })
}

fn stable_emitter_key(
    dimensions: GridDimensions,
    mount: FixtureMount,
) -> Result<u64, RuntimeInputError> {
    let anchor = match mount {
        FixtureMount::FreeStanding { origin } => origin,
        FixtureMount::WallMounted { anchor, .. } => anchor,
    };
    let index = dimensions
        .index(anchor)
        .ok_or(RuntimeInputError::OutOfBounds {
            source: "emitter mount",
            position: anchor,
        })?;
    let tag = match mount {
        FixtureMount::FreeStanding { .. } => 0_u64,
        FixtureMount::WallMounted {
            inward: CardinalDirection::North,
            ..
        } => 1,
        FixtureMount::WallMounted {
            inward: CardinalDirection::East,
            ..
        } => 2,
        FixtureMount::WallMounted {
            inward: CardinalDirection::South,
            ..
        } => 3,
        FixtureMount::WallMounted {
            inward: CardinalDirection::West,
            ..
        } => 4,
    };
    u64::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(8))
        .and_then(|base| base.checked_add(tag))
        .ok_or(RuntimeInputError::StableKeyOverflow)
}

#[derive(Debug)]
enum RuntimeInputError {
    InvalidDimensions,
    InvalidDoorRoot,
    InvalidEmitterOwner,
    OutOfBounds {
        source: &'static str,
        position: LightGridPos,
    },
    DuplicateOcclusionCell(LightGridPos),
    EmitterTransformMismatch {
        transform: LightGridPos,
        mount: LightGridPos,
    },
    StableKeyOverflow,
    CountOverflow,
    Core(LightFieldError),
}

impl fmt::Display for RuntimeInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => {
                write!(formatter, "invalid production Light Field dimensions")
            }
            Self::InvalidDoorRoot => write!(
                formatter,
                "Door component is not on a completed Door building root"
            ),
            Self::InvalidEmitterOwner => write!(
                formatter,
                "RadialLightEmitter is not on a completed OutdoorLamp root"
            ),
            Self::OutOfBounds { source, position } => write!(
                formatter,
                "{source} grid ({}, {}) is outside the Light Field",
                position.x, position.y
            ),
            Self::DuplicateOcclusionCell(position) => write!(
                formatter,
                "multiple occlusion owners occupy grid ({}, {})",
                position.x, position.y
            ),
            Self::EmitterTransformMismatch { transform, mount } => write!(
                formatter,
                "emitter transform ({}, {}) disagrees with mount origin ({}, {})",
                transform.x, transform.y, mount.x, mount.y
            ),
            Self::StableKeyOverflow => write!(formatter, "emitter stable key overflow"),
            Self::CountOverflow => write!(formatter, "lighting fixture count overflow"),
            Self::Core(error) => error.fmt(formatter),
        }
    }
}

impl From<LightFieldError> for RuntimeInputError {
    fn from(value: LightFieldError) -> Self {
        Self::Core(value)
    }
}
