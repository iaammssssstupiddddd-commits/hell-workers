//! Prevalidated storage and occupant recovery for M3 facility cleanup.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::{MUD_MIXER_CAPACITY, MUD_MIXER_MUD_CAPACITY, Z_ITEM_PICKUP};
use hw_core::relationships::{DeliveringTo, LoadedIn, ParkedAt, PushedBy, StoredIn};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer};
use hw_jobs::{Building, BuildingType, DeconstructionPending, DeconstructionSalvage, MovePlanned};
use hw_logistics::construction_helpers::ResourceItemVisualHandles;
use hw_logistics::types::{BelongsTo, BucketStorage, ResourceItem, Wheelbarrow};
use hw_logistics::zone::Stockpile;
use hw_logistics::{Inventory, ResourceType, build_recovery_placement_plan};
use hw_soul_ai::rest_area_relationship_sources;
use hw_world::WorldMap;
use hw_world::map::WorldMapOwnerSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecoveryPlanFailure {
    OwnerMismatch,
    NoSafeRecovery,
    InconsistentMixerInventory,
    UnsupportedTarget,
}

#[derive(Debug)]
pub(super) struct FacilityRecoveryPlan {
    pub companions_to_remove: Vec<OwnedCompanion>,
    pub ground_items: Vec<GroundItemRecovery>,
    pub wheelbarrows: Vec<WheelbarrowRecovery>,
    pub sand_items_to_absorb: Vec<Entity>,
    pub mud_transfers: Vec<MudTransfer>,
    pub mixer_increments: Vec<MixerIncrement>,
    pub expected_target_mixer: Option<MixerStorageSnapshot>,
    pub spawned_items: Vec<SpawnedRecoveryItem>,
    pub rest_sources: Vec<Entity>,
}

impl FacilityRecoveryPlan {
    pub(super) fn removed_owner_entities(&self, target: Entity) -> Vec<Entity> {
        let mut owners = vec![target];
        owners.extend(
            self.companions_to_remove
                .iter()
                .map(|companion| companion.entity),
        );
        owners.sort_unstable_by_key(|entity| entity.to_bits());
        owners
    }

    pub(super) fn cleanup_reference_entities(&self) -> Vec<Entity> {
        let mut entities = self
            .companions_to_remove
            .iter()
            .map(|companion| companion.entity)
            .chain(self.ground_items.iter().map(|item| item.entity))
            .chain(self.wheelbarrows.iter().map(|carrier| carrier.entity))
            .chain(
                self.wheelbarrows
                    .iter()
                    .flat_map(|carrier| carrier.loaded_items.iter().map(|item| item.entity)),
            )
            .chain(self.sand_items_to_absorb.iter().copied())
            .chain(self.mud_transfers.iter().map(|transfer| transfer.entity))
            .collect::<Vec<_>>();
        entities.sort_unstable_by_key(|entity| entity.to_bits());
        entities.dedup();
        entities
    }
}

#[derive(Debug)]
pub(super) struct OwnedCompanion {
    pub entity: Entity,
    pub owner_snapshot: WorldMapOwnerSnapshot,
}

#[derive(Debug)]
pub(super) struct GroundItemRecovery {
    pub entity: Entity,
    pub resource_type: ResourceType,
    pub position: Vec2,
}

#[derive(Debug)]
pub(super) struct WheelbarrowRecovery {
    pub entity: Entity,
    pub position: Vec2,
    pub loaded_items: Vec<LoadedItemRecovery>,
}

#[derive(Debug)]
pub(super) struct LoadedItemRecovery {
    pub entity: Entity,
    pub resource_type: ResourceType,
}

#[derive(Debug)]
pub(super) struct MudTransfer {
    pub entity: Entity,
    pub receiver: Entity,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MixerIncrement {
    pub receiver: Entity,
    pub expected: MixerStorageSnapshot,
    pub sand: u32,
    pub mud: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MixerStorageSnapshot {
    pub sand: u32,
    pub rock: u32,
    pub mud: u32,
}

impl From<&MudMixerStorage> for MixerStorageSnapshot {
    fn from(storage: &MudMixerStorage) -> Self {
        Self {
            sand: storage.sand,
            rock: storage.rock,
            mud: storage.mud,
        }
    }
}

#[derive(Debug)]
pub(super) struct SpawnedRecoveryItem {
    pub resource_type: ResourceType,
    pub position: Vec2,
}

#[derive(Debug)]
struct ItemSnapshot {
    entity: Entity,
    resource_type: ResourceType,
    belongs_to: Option<Entity>,
    stored_in: Option<Entity>,
    stored_by_mixer: Option<Entity>,
    loaded_in: Option<Entity>,
    delivering_to: Option<Entity>,
}

#[derive(Debug, Clone)]
struct MixerCandidate {
    entity: Entity,
    grid: (i32, i32),
    storage: MixerStorageSnapshot,
}

pub(super) fn prepare_facility_recovery(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
    target_snapshot: &WorldMapOwnerSnapshot,
    anchor: (i32, i32),
    salvage: DeconstructionSalvage,
) -> Result<FacilityRecoveryPlan, RecoveryPlanFailure> {
    let companions_to_remove = collect_companions(world, target, kind)?;
    let removed_owners = {
        let mut owners = vec![target];
        owners.extend(
            companions_to_remove
                .iter()
                .map(|companion| companion.entity),
        );
        owners
    };
    let items = collect_item_snapshots(world);
    let wheelbarrow_entities = collect_wheelbarrows(world, target, kind, &items)?;
    let wheelbarrow_set = wheelbarrow_entities.iter().copied().collect::<HashSet<_>>();
    let related_inventory_items = collect_related_inventory_items(world, &removed_owners);

    let mut ordinary_items = Vec::<(Entity, ResourceType)>::new();
    let mut sand_items_to_absorb = Vec::new();
    let mut mud_entities = Vec::new();
    for item in &items {
        if wheelbarrow_set.contains(&item.entity) {
            continue;
        }
        let owned = item.belongs_to.is_some_and(|owner| owner == target)
            || item
                .stored_in
                .is_some_and(|owner| removed_owners.contains(&owner))
            || item.stored_by_mixer == Some(target)
            || item
                .delivering_to
                .is_some_and(|owner| removed_owners.contains(&owner))
            || related_inventory_items.contains(&item.entity);
        if !owned {
            continue;
        }
        if item
            .loaded_in
            .is_some_and(|carrier| wheelbarrow_set.contains(&carrier))
        {
            continue;
        }
        match item.resource_type {
            ResourceType::Sand => sand_items_to_absorb.push(item.entity),
            ResourceType::StasisMud => mud_entities.push(item.entity),
            resource_type => ordinary_items.push((item.entity, resource_type)),
        }
    }
    ordinary_items.sort_unstable_by_key(|(entity, _)| entity.to_bits());
    sand_items_to_absorb.sort_unstable_by_key(|entity| entity.to_bits());
    mud_entities.sort_unstable_by_key(|entity| entity.to_bits());

    validate_kind_inventory(
        kind,
        target,
        &companions_to_remove,
        &ordinary_items,
        &mud_entities,
        &items,
    )?;

    let (expected_target_mixer, numeric_sand, numeric_rock) = if kind == BuildingType::MudMixer {
        let storage = world
            .get::<MudMixerStorage>(target)
            .ok_or(RecoveryPlanFailure::OwnerMismatch)?;
        let snapshot = MixerStorageSnapshot::from(storage);
        let stored_mud_count = mud_entities
            .iter()
            .filter(|entity| {
                world
                    .get::<StoredByMixer>(**entity)
                    .is_some_and(|owner| owner.0 == target)
            })
            .count() as u32;
        if stored_mud_count != snapshot.mud {
            return Err(RecoveryPlanFailure::InconsistentMixerInventory);
        }
        (Some(snapshot), snapshot.sand, snapshot.rock)
    } else {
        (None, 0, 0)
    };

    let sand_total = numeric_sand
        .checked_add(sand_items_to_absorb.len() as u32)
        .ok_or(RecoveryPlanFailure::UnsupportedTarget)?;
    let (mixer_increments, mud_transfers) = if sand_total == 0 && mud_entities.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let mut candidates = collect_mixer_candidates(world, target, anchor)?;
        allocate_volatile_recovery(&mut candidates, sand_total, &mud_entities)?
    };

    let mut spawned_resource_types = Vec::new();
    spawned_resource_types.extend(std::iter::repeat_n(
        ResourceType::Rock,
        numeric_rock as usize,
    ));
    match salvage {
        DeconstructionSalvage::None => {}
        DeconstructionSalvage::Material {
            resource_type,
            amount,
        } => spawned_resource_types.extend(std::iter::repeat_n(resource_type, amount as usize)),
    }

    let placement = {
        let map = world
            .get_resource::<WorldMap>()
            .ok_or(RecoveryPlanFailure::OwnerMismatch)?;
        let mut target_footprint = target_snapshot.building_grids.clone();
        target_footprint.extend(target_snapshot.floor_grids.iter().copied());
        target_footprint.sort_unstable_by_key(|&(x, y)| (y, x));
        target_footprint.dedup();
        build_recovery_placement_plan(
            map,
            anchor,
            &target_footprint,
            &removed_owners,
            ordinary_items.len() + spawned_resource_types.len(),
            wheelbarrow_entities.len(),
        )
        .ok_or(RecoveryPlanFailure::NoSafeRecovery)?
    };
    let mut item_positions = placement.item_positions.into_iter();
    let ground_items = ordinary_items
        .into_iter()
        .map(|(entity, resource_type)| GroundItemRecovery {
            entity,
            resource_type,
            position: item_positions
                .next()
                .expect("placement count matches ordinary item count"),
        })
        .collect::<Vec<_>>();
    let spawned_items = spawned_resource_types
        .into_iter()
        .map(|resource_type| SpawnedRecoveryItem {
            resource_type,
            position: item_positions
                .next()
                .expect("placement count matches spawned item count"),
        })
        .collect::<Vec<_>>();
    debug_assert!(item_positions.next().is_none());
    let wheelbarrows = wheelbarrow_entities
        .into_iter()
        .zip(placement.carrier_positions)
        .map(|(entity, position)| WheelbarrowRecovery {
            entity,
            position,
            loaded_items: items
                .iter()
                .filter(|item| item.loaded_in == Some(entity))
                .map(|item| LoadedItemRecovery {
                    entity: item.entity,
                    resource_type: item.resource_type,
                })
                .collect(),
        })
        .collect();

    Ok(FacilityRecoveryPlan {
        companions_to_remove,
        ground_items,
        wheelbarrows,
        sand_items_to_absorb,
        mud_transfers,
        mixer_increments,
        expected_target_mixer,
        spawned_items,
        rest_sources: if kind == BuildingType::RestArea {
            rest_area_relationship_sources(world, target)
        } else {
            Vec::new()
        },
    })
}

fn collect_companions(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
) -> Result<Vec<OwnedCompanion>, RecoveryPlanFailure> {
    let mut query = world.query::<(Entity, &BelongsTo, Option<&BucketStorage>)>();
    let mut owned = query
        .iter(world)
        .filter_map(|(entity, belongs, bucket_storage)| {
            (belongs.0 == target).then_some((entity, bucket_storage.is_some()))
        })
        .collect::<Vec<_>>();
    owned.sort_unstable_by_key(|(entity, _)| entity.to_bits());

    if kind != BuildingType::Tank && owned.iter().any(|(_, bucket)| *bucket) {
        return Err(RecoveryPlanFailure::OwnerMismatch);
    }
    let mut companions = Vec::new();
    for (entity, is_bucket_storage) in owned {
        if !is_bucket_storage {
            continue;
        }
        let stockpile = world
            .get::<Stockpile>(entity)
            .ok_or(RecoveryPlanFailure::OwnerMismatch)?;
        if kind != BuildingType::Tank
            || !matches!(
                stockpile.resource_type,
                None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
            )
        {
            return Err(RecoveryPlanFailure::OwnerMismatch);
        }
        let snapshot = world
            .get_resource::<WorldMap>()
            .ok_or(RecoveryPlanFailure::OwnerMismatch)?
            .snapshot_owner(entity);
        if snapshot.stockpile_grids.len() != 1
            || !snapshot.building_grids.is_empty()
            || !snapshot.floor_grids.is_empty()
            || !snapshot.door_grids.is_empty()
            || !snapshot.bridge_grids.is_empty()
        {
            return Err(RecoveryPlanFailure::OwnerMismatch);
        }
        companions.push(OwnedCompanion {
            entity,
            owner_snapshot: snapshot,
        });
    }
    if kind == BuildingType::Tank && companions.is_empty() {
        return Err(RecoveryPlanFailure::OwnerMismatch);
    }
    Ok(companions)
}

fn collect_item_snapshots(world: &mut World) -> Vec<ItemSnapshot> {
    let mut query = world.query::<(
        Entity,
        &ResourceItem,
        Option<&BelongsTo>,
        Option<&StoredIn>,
        Option<&StoredByMixer>,
        Option<&LoadedIn>,
        Option<&DeliveringTo>,
    )>();
    let mut items = query
        .iter(world)
        .map(
            |(entity, item, belongs, stored, mixer, loaded, delivering)| ItemSnapshot {
                entity,
                resource_type: item.0,
                belongs_to: belongs.map(|owner| owner.0),
                stored_in: stored.map(|owner| owner.0),
                stored_by_mixer: mixer.map(|owner| owner.0),
                loaded_in: loaded.map(|owner| owner.0),
                delivering_to: delivering.map(|owner| owner.0),
            },
        )
        .collect::<Vec<_>>();
    items.sort_unstable_by_key(|item| item.entity.to_bits());
    items
}

fn collect_related_inventory_items(world: &mut World, owners: &[Entity]) -> HashSet<Entity> {
    let mut query = world.query::<(&hw_jobs::AssignedTask, Option<&Inventory>)>();
    query
        .iter(world)
        .filter_map(|(task, inventory)| {
            owners
                .iter()
                .any(|&owner| task.references_entity(owner))
                .then(|| inventory.and_then(|inventory| inventory.0))
                .flatten()
        })
        .collect()
}

fn collect_wheelbarrows(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
    items: &[ItemSnapshot],
) -> Result<Vec<Entity>, RecoveryPlanFailure> {
    let mut query = world.query::<(Entity, &Wheelbarrow, Option<&BelongsTo>, Option<&ParkedAt>)>();
    let mut carriers = query
        .iter(world)
        .filter_map(|(entity, _, belongs, parked)| {
            (belongs.is_some_and(|owner| owner.0 == target)
                || parked.is_some_and(|owner| owner.0 == target))
            .then_some((
                entity,
                belongs.map(|owner| owner.0),
                parked.map(|owner| owner.0),
            ))
        })
        .collect::<Vec<_>>();
    carriers.sort_unstable_by_key(|(entity, _, _)| entity.to_bits());
    if kind != BuildingType::WheelbarrowParking && !carriers.is_empty() {
        return Err(RecoveryPlanFailure::OwnerMismatch);
    }
    if carriers
        .iter()
        .any(|(_, belongs, parked)| *belongs != Some(target) || parked.is_some_and(|p| p != target))
    {
        return Err(RecoveryPlanFailure::OwnerMismatch);
    }
    let entities = carriers
        .into_iter()
        .map(|(entity, _, _)| entity)
        .collect::<Vec<_>>();
    if kind == BuildingType::WheelbarrowParking {
        let owned_non_wheelbarrow = items.iter().any(|item| {
            item.belongs_to == Some(target)
                && !entities.contains(&item.entity)
                && item.loaded_in.is_none()
        });
        if owned_non_wheelbarrow {
            return Err(RecoveryPlanFailure::OwnerMismatch);
        }
    }
    Ok(entities)
}

fn validate_kind_inventory(
    kind: BuildingType,
    target: Entity,
    companions: &[OwnedCompanion],
    ordinary_items: &[(Entity, ResourceType)],
    mud_entities: &[Entity],
    items: &[ItemSnapshot],
) -> Result<(), RecoveryPlanFailure> {
    if kind == BuildingType::Tank {
        let companion_entities = companions
            .iter()
            .map(|companion| companion.entity)
            .collect::<Vec<_>>();
        for item in items.iter().filter(|item| {
            item.belongs_to == Some(target)
                || item.stored_in == Some(target)
                || item
                    .stored_in
                    .is_some_and(|owner| companion_entities.contains(&owner))
        }) {
            let valid = if item.stored_in == Some(target) {
                item.resource_type == ResourceType::Water
            } else {
                matches!(
                    item.resource_type,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                ) && item.belongs_to == Some(target)
                    && item
                        .stored_in
                        .is_none_or(|owner| companion_entities.contains(&owner))
            };
            if !valid {
                return Err(RecoveryPlanFailure::OwnerMismatch);
            }
        }
    }
    if kind != BuildingType::MudMixer && !mud_entities.is_empty() {
        return Err(RecoveryPlanFailure::OwnerMismatch);
    }
    if ordinary_items
        .iter()
        .any(|(_, resource)| matches!(resource, ResourceType::Sand | ResourceType::StasisMud))
    {
        return Err(RecoveryPlanFailure::UnsupportedTarget);
    }
    Ok(())
}

fn collect_mixer_candidates(
    world: &mut World,
    target: Entity,
    anchor: (i32, i32),
) -> Result<Vec<MixerCandidate>, RecoveryPlanFailure> {
    let mut query = world.query::<(
        Entity,
        &Transform,
        &Building,
        &MudMixerStorage,
        Option<&MovePlanned>,
        Option<&DeconstructionPending>,
    )>();
    let mut candidates = query
        .iter(world)
        .filter_map(|(entity, transform, building, storage, moving, pending)| {
            (entity != target
                && building.kind == BuildingType::MudMixer
                && !building.is_provisional
                && moving.is_none()
                && pending.is_none())
            .then_some(MixerCandidate {
                entity,
                grid: WorldMap::world_to_grid(transform.translation.truncate()),
                storage: MixerStorageSnapshot::from(storage),
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|candidate| {
        (
            candidate.grid.0.abs_diff(anchor.0) + candidate.grid.1.abs_diff(anchor.1),
            candidate.grid.1,
            candidate.grid.0,
            candidate.entity.to_bits(),
        )
    });

    let stored_mud_counts = {
        let mut counts = HashMap::<Entity, u32>::new();
        let mut mud_query = world.query::<(&ResourceItem, &StoredByMixer)>();
        for (item, owner) in mud_query.iter(world) {
            if item.0 == ResourceType::StasisMud {
                *counts.entry(owner.0).or_default() += 1;
            }
        }
        counts
    };
    // A malformed third-party mixer is not a safe recovery destination, but
    // it also does not make the target's own inventory inconsistent. Exclude
    // it from allocation and let the normal capacity preflight decide whether
    // another valid receiver exists.
    candidates.retain(|candidate| {
        stored_mud_counts
            .get(&candidate.entity)
            .copied()
            .unwrap_or_default()
            == candidate.storage.mud
    });
    Ok(candidates)
}

fn allocate_volatile_recovery(
    candidates: &mut [MixerCandidate],
    mut sand_remaining: u32,
    mud_entities: &[Entity],
) -> Result<(Vec<MixerIncrement>, Vec<MudTransfer>), RecoveryPlanFailure> {
    let mut increments = HashMap::<Entity, (MixerStorageSnapshot, u32, u32)>::new();
    for candidate in candidates.iter_mut() {
        let capacity = MUD_MIXER_CAPACITY.saturating_sub(candidate.storage.sand);
        let amount = sand_remaining.min(capacity);
        if amount > 0 {
            increments.insert(candidate.entity, (candidate.storage, amount, 0));
            candidate.storage.sand += amount;
            sand_remaining -= amount;
        }
        if sand_remaining == 0 {
            break;
        }
    }
    if sand_remaining > 0 {
        return Err(RecoveryPlanFailure::NoSafeRecovery);
    }

    let mut mud_transfers = Vec::with_capacity(mud_entities.len());
    let mut mud_index = 0;
    for candidate in candidates.iter_mut() {
        let capacity = MUD_MIXER_MUD_CAPACITY.saturating_sub(candidate.storage.mud);
        let remaining = (mud_entities.len() - mud_index) as u32;
        let amount = remaining.min(capacity);
        if amount > 0 {
            let entry = increments
                .entry(candidate.entity)
                .or_insert((candidate.storage, 0, 0));
            entry.2 += amount;
            candidate.storage.mud += amount;
            for &entity in &mud_entities[mud_index..mud_index + amount as usize] {
                mud_transfers.push(MudTransfer {
                    entity,
                    receiver: candidate.entity,
                });
            }
            mud_index += amount as usize;
        }
        if mud_index == mud_entities.len() {
            break;
        }
    }
    if mud_index != mud_entities.len() {
        return Err(RecoveryPlanFailure::NoSafeRecovery);
    }

    let mut mixer_increments = increments
        .into_iter()
        .map(|(receiver, (expected, sand, mud))| MixerIncrement {
            receiver,
            expected,
            sand,
            mud,
        })
        .collect::<Vec<_>>();
    mixer_increments.sort_unstable_by_key(|increment| increment.receiver.to_bits());
    Ok((mixer_increments, mud_transfers))
}

pub(super) fn recovery_plan_still_matches(
    world: &World,
    plan: &FacilityRecoveryPlan,
    target: Entity,
) -> bool {
    if let Some(expected) = plan.expected_target_mixer
        && world
            .get::<MudMixerStorage>(target)
            .is_none_or(|storage| MixerStorageSnapshot::from(storage) != expected)
    {
        return false;
    }
    plan.mixer_increments.iter().all(|increment| {
        world
            .get::<MudMixerStorage>(increment.receiver)
            .is_some_and(|storage| MixerStorageSnapshot::from(storage) == increment.expected)
    }) && plan.ground_items.iter().all(|item| {
        world
            .get::<ResourceItem>(item.entity)
            .is_some_and(|resource| resource.0 == item.resource_type)
    }) && plan.sand_items_to_absorb.iter().all(|entity| {
        world
            .get::<ResourceItem>(*entity)
            .is_some_and(|item| item.0 == ResourceType::Sand)
    }) && plan.mud_transfers.iter().all(|transfer| {
        world
            .get::<ResourceItem>(transfer.entity)
            .is_some_and(|item| item.0 == ResourceType::StasisMud)
    }) && plan.wheelbarrows.iter().all(|carrier| {
        world.get::<Wheelbarrow>(carrier.entity).is_some()
            && carrier.loaded_items.iter().all(|item| {
                world
                    .get::<ResourceItem>(item.entity)
                    .is_some_and(|resource| resource.0 == item.resource_type)
                    && world
                        .get::<LoadedIn>(item.entity)
                        .is_some_and(|loaded| loaded.0 == carrier.entity)
            })
    })
}

pub(super) fn apply_facility_recovery(world: &mut World, plan: &FacilityRecoveryPlan) {
    for increment in &plan.mixer_increments {
        let mut storage = world
            .get_mut::<MudMixerStorage>(increment.receiver)
            .expect("prevalidated recovery receiver disappeared");
        debug_assert_eq!(MixerStorageSnapshot::from(&*storage), increment.expected);
        storage.sand += increment.sand;
        storage.mud += increment.mud;
    }
    for transfer in &plan.mud_transfers {
        let mut item = world.entity_mut(transfer.entity);
        item.remove::<(StoredIn, LoadedIn, BelongsTo, DeliveringTo)>();
        item.insert((StoredByMixer(transfer.receiver), Visibility::Hidden));
    }
    for &entity in &plan.sand_items_to_absorb {
        if let Ok(item) = world.get_entity_mut(entity) {
            item.despawn();
        }
    }
    for recovery in &plan.ground_items {
        let mut item = world.entity_mut(recovery.entity);
        item.remove::<(
            StoredIn,
            LoadedIn,
            StoredByMixer,
            BelongsTo,
            DeliveringTo,
            ParkedAt,
            PushedBy,
        )>();
        item.insert((
            Visibility::Visible,
            Transform::from_xyz(recovery.position.x, recovery.position.y, Z_ITEM_PICKUP),
        ));
    }
    for recovery in &plan.wheelbarrows {
        let mut carrier = world.entity_mut(recovery.entity);
        carrier.remove::<(ParkedAt, PushedBy, BelongsTo)>();
        carrier.insert((
            Visibility::Visible,
            Transform::from_xyz(recovery.position.x, recovery.position.y, Z_ITEM_PICKUP),
        ));
        for loaded in &recovery.loaded_items {
            let mut item = world.entity_mut(loaded.entity);
            item.remove::<(StoredIn, StoredByMixer, BelongsTo, DeliveringTo)>();
            item.insert(Visibility::Hidden);
        }
    }
    for companion in &plan.companions_to_remove {
        if let Ok(entity) = world.get_entity_mut(companion.entity) {
            entity.despawn();
        }
    }
    for item in &plan.spawned_items {
        spawn_recovery_item(world, item.resource_type, item.position);
    }
}

fn spawn_recovery_item(world: &mut World, resource_type: ResourceType, position: Vec2) {
    let handles = world.resource::<ResourceItemVisualHandles>();
    let (image, name) = match resource_type {
        ResourceType::Bone => (handles.icon_bone_small.clone(), "Item (Bone, Recovery)"),
        ResourceType::Wood => (handles.icon_wood_small.clone(), "Item (Wood, Recovery)"),
        ResourceType::Rock => (handles.icon_rock_small.clone(), "Item (Rock, Recovery)"),
        ResourceType::Sand => (handles.icon_sand_small.clone(), "Item (Sand, Recovery)"),
        ResourceType::StasisMud => (
            handles.icon_stasis_mud_small.clone(),
            "Item (StasisMud, Recovery)",
        ),
        ResourceType::Water
        | ResourceType::BucketEmpty
        | ResourceType::BucketWater
        | ResourceType::Wheelbarrow => {
            unreachable!("entity-backed facility contents are never spawned from counts")
        }
    };
    world.spawn((
        ResourceItem(resource_type),
        Sprite {
            image,
            custom_size: Some(Vec2::splat(hw_core::constants::TILE_SIZE * 0.5)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, Z_ITEM_PICKUP),
        Name::new(name),
    ));
}
