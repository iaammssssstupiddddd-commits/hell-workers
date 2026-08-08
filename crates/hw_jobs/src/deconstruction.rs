//! Durable deconstruction orders and pure target-domain policy.

use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_energy::{SoulSpaPhase, SoulSpaSite, SoulSpaTile};

use crate::construction::{
    FloorConstructionSite, FloorTileBlueprint, WallConstructionSite, WallTileBlueprint,
};
use crate::{
    ActiveTaskIdentity, Blueprint, Building, BuildingType, MovePlanned, TaskDiagnosticDomainMask,
    TaskDiagnosticInputStamp,
};

/// Persisted root marker for a player-issued deconstruction order.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component, Default)]
pub struct DeconstructionOrder;

/// Durable order -> canonical building/Soul Spa root relationship.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = DeconstructionOrders)]
pub struct TargetDeconstructionRoot(pub Entity);

impl Default for TargetDeconstructionRoot {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// Automatically maintained reverse relationship on a deconstruction target.
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = TargetDeconstructionRoot)]
pub struct DeconstructionOrders(Vec<Entity>);

impl DeconstructionOrders {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Runtime gate rebuilt from the durable relationship after loading.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionPending {
    pub order: Entity,
}

/// Runtime-only exactly-once commit claim. M2 owns acquisition and release.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionCommitClaim {
    pub world_epoch: u64,
    pub order: Entity,
}

/// Runtime-only request emitted by the Soul executor after dismantling finishes.
///
/// The worker identity remains attached to the durable order entity. `target`
/// is carried separately so the order keeps its one-slot `TaskWorkers`
/// relationship until the root finalizer terminalizes the worker.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionCommitRequest {
    pub world_epoch: u64,
    pub worker: Entity,
    pub identity: ActiveTaskIdentity,
    pub order: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeconstructionCommitResult {
    Committed,
    Canceled,
    Duplicate,
    StaleWorld,
    StaleIdentity,
    StaleTarget,
    OwnerMismatch,
    NoSafeRecovery,
    InconsistentMixerInventory,
    Moving,
    UnsupportedTarget,
}

/// Typed receipt for one logical commit request.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionCommitOutcome {
    pub worker: Entity,
    pub order: Entity,
    pub target: Entity,
    pub result: DeconstructionCommitResult,
}

/// Headless owner request used by the dashboard adapter added in M3.
/// Keeping it in M2 makes cancel-vs-claim behavior testable before UI exposure.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionCancelRequest {
    pub world_epoch: u64,
    pub order: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeconstructionCancelResult {
    Canceled,
    ClaimInProgress,
    StaleWorld,
    StaleOrder,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionCancelOutcome {
    pub order: Entity,
    pub target: Option<Entity>,
    pub result: DeconstructionCancelResult,
}

/// One logical player designation request.
///
/// `request_id` is allocated by the runtime input owner and is only used to
/// provide a stable receipt for every click.  It is deliberately not part of
/// the persisted order model.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionDesignationRequest {
    pub request_id: u64,
    pub world_epoch: u64,
    pub hit: Option<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeconstructionDesignationResult {
    Designated {
        order: Entity,
        target: Entity,
        class: DeconstructionTargetClass,
    },
    Rejected(DeconstructionDesignationRejectReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeconstructionDesignationRejectReason {
    StaleWorld,
    NoTarget,
    Target(DeconstructionRejectReason),
    CleanupUnavailable,
}

/// Typed receipt for exactly one logical designation request.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionDesignationOutcome {
    pub request_id: u64,
    pub hit: Option<Entity>,
    pub result: DeconstructionDesignationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeconstructionBlockReason {
    StaleTarget,
    OwnerMismatch,
    NoSafeRecovery,
    InconsistentMixerInventory,
    Moving,
    UnsupportedTarget,
}

/// Latest-only runtime blocker attached to the durable order.
///
/// `pending` is available to producers that cannot read the canonical revision
/// resource. Root transactions should prefer `armed` with a baseline that
/// accounts for their own known cleanup changes, so a real recovery change is
/// not swallowed before the next revision sync.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructionBlocker {
    pub reason: DeconstructionBlockReason,
    pub stamp: Option<TaskDiagnosticInputStamp>,
    pub domains: TaskDiagnosticDomainMask,
    pub active: bool,
}

impl DeconstructionBlocker {
    pub const fn pending(
        reason: DeconstructionBlockReason,
        domains: TaskDiagnosticDomainMask,
    ) -> Self {
        Self {
            reason,
            stamp: None,
            domains,
            active: true,
        }
    }

    pub const fn armed(
        reason: DeconstructionBlockReason,
        domains: TaskDiagnosticDomainMask,
        stamp: TaskDiagnosticInputStamp,
    ) -> Self {
        Self {
            reason,
            stamp: Some(stamp),
            domains,
            active: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DeconstructionTargetClass {
    Building(BuildingType),
    SoulSpa,
}

impl DeconstructionTargetClass {
    pub const fn building_type(self) -> BuildingType {
        match self {
            Self::Building(kind) => kind,
            Self::SoulSpa => BuildingType::SoulSpa,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct ResolvedDeconstructionTarget {
    pub root: Entity,
    pub class: DeconstructionTargetClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DeconstructionRejectReason {
    StaleTarget,
    UnsupportedTarget,
    ConstructionInProgress,
    Moving,
    AlreadyDesignated,
    OwnerMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DeconstructionSalvage {
    None,
    Material {
        resource_type: ResourceType,
        amount: u32,
    },
}

/// Move evidence owned by higher-level runtime crates.
///
/// `MovePlanned` is checked directly. M2's root adapter must aggregate the
/// remaining move task, assignment, and pending-apply evidence into this value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeconstructionEligibilityFacts {
    pub move_task_targets_root: bool,
    pub assigned_move_targets_root: bool,
    pub pending_building_move: bool,
}

impl DeconstructionEligibilityFacts {
    const fn has_external_move_conflict(self) -> bool {
        self.move_task_targets_root || self.assigned_move_targets_root || self.pending_building_move
    }
}

/// Fixed salvage table. This deliberately does not derive from construction cost.
pub const fn deconstruction_salvage(kind: BuildingType) -> DeconstructionSalvage {
    match kind {
        BuildingType::Wall => salvage_material(ResourceType::Wood, 1),
        BuildingType::Door => salvage_material(ResourceType::Wood, 1),
        BuildingType::Floor => salvage_material(ResourceType::Bone, 1),
        BuildingType::Tank => salvage_material(ResourceType::Wood, 1),
        BuildingType::MudMixer => salvage_material(ResourceType::Wood, 2),
        BuildingType::RestArea => salvage_material(ResourceType::Wood, 2),
        BuildingType::Bridge => salvage_material(ResourceType::Rock, 3),
        BuildingType::SandPile => DeconstructionSalvage::None,
        BuildingType::BonePile => salvage_material(ResourceType::Bone, 5),
        BuildingType::WheelbarrowParking => salvage_material(ResourceType::Wood, 1),
        BuildingType::SoulSpa => salvage_material(ResourceType::Bone, 6),
        BuildingType::OutdoorLamp => salvage_material(ResourceType::Bone, 1),
    }
}

/// Cleanup implementations that are safe to expose in the current runtime.
///
/// Keep this as the shared allow-list used by designation, assignment, task
/// execution, and finalization.
pub const fn supports_deconstruction_cleanup(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Wall
            | BuildingType::Door
            | BuildingType::Floor
            | BuildingType::Tank
            | BuildingType::MudMixer
            | BuildingType::RestArea
            | BuildingType::Bridge
            | BuildingType::SandPile
            | BuildingType::BonePile
            | BuildingType::WheelbarrowParking
            | BuildingType::SoulSpa
            | BuildingType::OutdoorLamp
    )
}

/// Compatibility name retained while the M3 facility transactions land.
/// New consumers should use [`supports_deconstruction_cleanup`].
pub const fn supports_basic_deconstruction_cleanup(kind: BuildingType) -> bool {
    supports_deconstruction_cleanup(kind)
}

/// Cross-crate marker facts needed to prove that a completed building has the
/// concrete owner shape expected by its cleanup implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeconstructionTargetMarkers {
    pub water_storage: bool,
    pub mud_mixer_storage: bool,
    pub rest_area: bool,
    pub wheelbarrow_parking: bool,
    pub sand_pile: bool,
    pub bone_pile: bool,
    pub door: bool,
    pub bridge: bool,
    pub operational_soul_spa: bool,
    pub power_consumer: bool,
    pub power_generator: bool,
}

pub const fn deconstruction_marker_matches(
    kind: BuildingType,
    markers: DeconstructionTargetMarkers,
) -> bool {
    let no_pile = !markers.sand_pile && !markers.bone_pile;
    let no_facility = !markers.water_storage
        && !markers.mud_mixer_storage
        && !markers.rest_area
        && !markers.wheelbarrow_parking;
    let no_structure = !markers.door && !markers.bridge;
    let no_energy =
        !markers.operational_soul_spa && !markers.power_consumer && !markers.power_generator;
    let plain = no_pile && no_facility && no_structure && no_energy;
    match kind {
        BuildingType::Wall | BuildingType::Floor => plain,
        BuildingType::Door => {
            markers.door && no_pile && no_facility && no_energy && !markers.bridge
        }
        BuildingType::Bridge => {
            markers.bridge && no_pile && no_facility && no_energy && !markers.door
        }
        BuildingType::Tank => {
            markers.water_storage
                && !markers.mud_mixer_storage
                && !markers.rest_area
                && !markers.wheelbarrow_parking
                && no_pile
                && no_structure
                && no_energy
        }
        BuildingType::MudMixer => {
            markers.water_storage
                && markers.mud_mixer_storage
                && !markers.rest_area
                && !markers.wheelbarrow_parking
                && no_pile
                && no_structure
                && no_energy
        }
        BuildingType::RestArea => {
            markers.rest_area
                && !markers.water_storage
                && !markers.mud_mixer_storage
                && !markers.wheelbarrow_parking
                && no_pile
                && no_structure
                && no_energy
        }
        BuildingType::WheelbarrowParking => {
            markers.wheelbarrow_parking
                && !markers.water_storage
                && !markers.mud_mixer_storage
                && !markers.rest_area
                && no_pile
                && no_structure
                && no_energy
        }
        BuildingType::SandPile => {
            markers.sand_pile && !markers.bone_pile && no_facility && no_structure && no_energy
        }
        BuildingType::BonePile => {
            markers.bone_pile && !markers.sand_pile && no_facility && no_structure && no_energy
        }
        BuildingType::SoulSpa => {
            markers.operational_soul_spa
                && markers.power_generator
                && !markers.power_consumer
                && no_pile
                && no_facility
                && no_structure
        }
        BuildingType::OutdoorLamp => {
            markers.power_consumer
                && !markers.power_generator
                && !markers.operational_soul_spa
                && no_pile
                && no_facility
                && no_structure
        }
    }
}

/// Verifies the concrete marker owned by an M2 resource-pile building.
///
/// `BuildingType` alone is not enough for the cleanup pipeline: the producer,
/// assignment apply boundary, and finalizer must all reject the same malformed
/// world instead of assigning work that can only fail at commit time.
pub const fn basic_deconstruction_marker_matches(
    kind: BuildingType,
    has_sand_pile: bool,
    has_bone_pile: bool,
) -> bool {
    deconstruction_marker_matches(
        kind,
        DeconstructionTargetMarkers {
            water_storage: false,
            mud_mixer_storage: false,
            rest_area: false,
            wheelbarrow_parking: false,
            sand_pile: has_sand_pile,
            bone_pile: has_bone_pile,
            door: false,
            bridge: false,
            operational_soul_spa: false,
            power_consumer: false,
            power_generator: false,
        },
    )
}

const fn salvage_material(resource_type: ResourceType, amount: u32) -> DeconstructionSalvage {
    DeconstructionSalvage::Material {
        resource_type,
        amount,
    }
}

/// Resolves a directly hit entity to the canonical completed target root.
///
/// WorldMap grid lookup should happen before this call. A Soul Spa tile is the
/// only supported child hit because it has a durable parent relationship.
pub fn resolve_deconstruction_target(
    world: &World,
    hit: Entity,
) -> Result<ResolvedDeconstructionTarget, DeconstructionRejectReason> {
    if world.get_entity(hit).is_err() {
        return Err(DeconstructionRejectReason::StaleTarget);
    }

    if world.get::<Blueprint>(hit).is_some()
        || world.get::<FloorConstructionSite>(hit).is_some()
        || world.get::<WallConstructionSite>(hit).is_some()
        || world.get::<FloorTileBlueprint>(hit).is_some()
        || world.get::<WallTileBlueprint>(hit).is_some()
    {
        return Err(DeconstructionRejectReason::ConstructionInProgress);
    }

    if let Some(tile) = world.get::<SoulSpaTile>(hit) {
        return resolve_soul_spa_root(world, tile.parent_site);
    }
    if world.get::<SoulSpaSite>(hit).is_some() {
        return resolve_soul_spa_root(world, hit);
    }

    if let Some(building) = world.get::<Building>(hit) {
        if building.is_provisional {
            return Err(DeconstructionRejectReason::ConstructionInProgress);
        }
        if building.kind == BuildingType::SoulSpa {
            return resolve_soul_spa_root(world, hit);
        }
        return Ok(ResolvedDeconstructionTarget {
            root: hit,
            class: DeconstructionTargetClass::Building(building.kind),
        });
    }

    Err(DeconstructionRejectReason::UnsupportedTarget)
}

/// Applies runtime exclusivity gates after canonical target resolution.
pub fn evaluate_deconstruction_target(
    world: &World,
    hit: Entity,
    facts: DeconstructionEligibilityFacts,
) -> Result<ResolvedDeconstructionTarget, DeconstructionRejectReason> {
    let target = resolve_deconstruction_target(world, hit)?;
    if world.get::<MovePlanned>(target.root).is_some() || facts.has_external_move_conflict() {
        return Err(DeconstructionRejectReason::Moving);
    }
    if world.get::<DeconstructionPending>(target.root).is_some()
        || world
            .get::<DeconstructionOrders>(target.root)
            .is_some_and(|orders| !orders.is_empty())
    {
        return Err(DeconstructionRejectReason::AlreadyDesignated);
    }
    Ok(target)
}

fn resolve_soul_spa_root(
    world: &World,
    root: Entity,
) -> Result<ResolvedDeconstructionTarget, DeconstructionRejectReason> {
    let Some(site) = world.get::<SoulSpaSite>(root) else {
        return Err(if world.get_entity(root).is_ok() {
            DeconstructionRejectReason::OwnerMismatch
        } else {
            DeconstructionRejectReason::StaleTarget
        });
    };
    let Some(building) = world.get::<Building>(root) else {
        return Err(DeconstructionRejectReason::OwnerMismatch);
    };
    if building.kind != BuildingType::SoulSpa {
        return Err(DeconstructionRejectReason::OwnerMismatch);
    }
    if building.is_provisional {
        return Err(DeconstructionRejectReason::ConstructionInProgress);
    }
    if site.phase != SoulSpaPhase::Operational {
        return Err(DeconstructionRejectReason::ConstructionInProgress);
    }
    Ok(ResolvedDeconstructionTarget {
        root,
        class: DeconstructionTargetClass::SoulSpa,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Designation, PlayerIssuedDesignation, Priority, TaskSlots, WorkType};

    #[test]
    fn salvage_table_is_exhaustive_and_keeps_special_cases_explicit() {
        assert_eq!(
            BuildingType::ALL.map(deconstruction_salvage),
            [
                salvage_material(ResourceType::Wood, 1),
                salvage_material(ResourceType::Wood, 1),
                salvage_material(ResourceType::Bone, 1),
                salvage_material(ResourceType::Wood, 1),
                salvage_material(ResourceType::Wood, 2),
                salvage_material(ResourceType::Wood, 2),
                salvage_material(ResourceType::Rock, 3),
                DeconstructionSalvage::None,
                salvage_material(ResourceType::Bone, 5),
                salvage_material(ResourceType::Wood, 1),
                salvage_material(ResourceType::Bone, 6),
                salvage_material(ResourceType::Bone, 1),
            ]
        );
    }

    #[test]
    fn every_completed_building_type_is_eligible() {
        let mut world = World::new();
        for kind in BuildingType::ALL {
            let entity = if kind == BuildingType::SoulSpa {
                world
                    .spawn((
                        Building {
                            kind,
                            is_provisional: false,
                        },
                        SoulSpaSite {
                            phase: SoulSpaPhase::Operational,
                            ..default()
                        },
                    ))
                    .id()
            } else {
                world
                    .spawn(Building {
                        kind,
                        is_provisional: false,
                    })
                    .id()
            };
            assert_eq!(
                evaluate_deconstruction_target(
                    &world,
                    entity,
                    DeconstructionEligibilityFacts::default(),
                ),
                Ok(ResolvedDeconstructionTarget {
                    root: entity,
                    class: if kind == BuildingType::SoulSpa {
                        DeconstructionTargetClass::SoulSpa
                    } else {
                        DeconstructionTargetClass::Building(kind)
                    },
                })
            );
        }
    }

    #[test]
    fn every_soul_spa_tile_resolves_to_the_same_operational_root() {
        let mut world = World::new();
        let site = world
            .spawn((
                Building {
                    kind: BuildingType::SoulSpa,
                    is_provisional: false,
                },
                SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    ..default()
                },
            ))
            .id();
        for grid_pos in [(2, 3), (3, 3), (2, 4), (3, 4)] {
            let tile = world
                .spawn(SoulSpaTile {
                    parent_site: site,
                    grid_pos,
                })
                .id();
            assert_eq!(
                resolve_deconstruction_target(&world, tile),
                Ok(ResolvedDeconstructionTarget {
                    root: site,
                    class: DeconstructionTargetClass::SoulSpa,
                })
            );
        }
    }

    #[test]
    fn construction_move_and_existing_order_are_rejected() {
        let mut world = World::new();
        let provisional = world
            .spawn(Building {
                kind: BuildingType::Wall,
                is_provisional: true,
            })
            .id();
        assert_eq!(
            evaluate_deconstruction_target(
                &world,
                provisional,
                DeconstructionEligibilityFacts::default(),
            ),
            Err(DeconstructionRejectReason::ConstructionInProgress)
        );

        let moving = world.spawn(Building::default()).id();
        let move_task = world.spawn_empty().id();
        world.entity_mut(moving).insert(MovePlanned {
            task_entity: move_task,
        });
        assert_eq!(
            evaluate_deconstruction_target(
                &world,
                moving,
                DeconstructionEligibilityFacts::default(),
            ),
            Err(DeconstructionRejectReason::Moving)
        );

        let target = world.spawn(Building::default()).id();
        let order = world
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                PlayerIssuedDesignation,
                Priority::default(),
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
                Transform::default(),
            ))
            .id();
        world.flush();
        assert_eq!(
            evaluate_deconstruction_target(
                &world,
                target,
                DeconstructionEligibilityFacts::default(),
            ),
            Err(DeconstructionRejectReason::AlreadyDesignated)
        );
        assert_eq!(
            world
                .get::<DeconstructionOrders>(target)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![order]
        );
    }

    #[test]
    fn malformed_or_constructing_soul_spa_roots_are_rejected() {
        let mut world = World::new();
        let generic = world
            .spawn(Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            })
            .id();
        let site_only = world
            .spawn(SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                ..default()
            })
            .id();
        let constructing = world
            .spawn((
                Building {
                    kind: BuildingType::SoulSpa,
                    is_provisional: false,
                },
                SoulSpaSite::default(),
            ))
            .id();

        assert_eq!(
            resolve_deconstruction_target(&world, generic),
            Err(DeconstructionRejectReason::OwnerMismatch)
        );
        assert_eq!(
            resolve_deconstruction_target(&world, site_only),
            Err(DeconstructionRejectReason::OwnerMismatch)
        );
        assert_eq!(
            resolve_deconstruction_target(&world, constructing),
            Err(DeconstructionRejectReason::ConstructionInProgress)
        );
    }

    #[test]
    fn completed_building_with_a_construction_role_is_rejected() {
        let mut world = World::new();
        let malformed = world
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Blueprint::new(BuildingType::Wall, vec![(1, 2)]),
            ))
            .id();

        assert_eq!(
            resolve_deconstruction_target(&world, malformed),
            Err(DeconstructionRejectReason::ConstructionInProgress)
        );
    }

    #[test]
    fn every_external_move_conflict_is_fail_closed() {
        let mut world = World::new();
        let target = world.spawn(Building::default()).id();
        for facts in [
            DeconstructionEligibilityFacts {
                move_task_targets_root: true,
                ..default()
            },
            DeconstructionEligibilityFacts {
                assigned_move_targets_root: true,
                ..default()
            },
            DeconstructionEligibilityFacts {
                pending_building_move: true,
                ..default()
            },
        ] {
            assert_eq!(
                evaluate_deconstruction_target(&world, target, facts),
                Err(DeconstructionRejectReason::Moving)
            );
        }
    }
}
