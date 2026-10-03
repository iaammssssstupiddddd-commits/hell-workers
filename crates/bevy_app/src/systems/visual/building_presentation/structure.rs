use super::production;
use crate::assets::building_asset_set::{
    BuildingAssetAuthority, BuildingAssetPool, BuildingAssetSetIdentity, BuildingProductionState,
};
use crate::plugins::startup::Building3dHandles;
use crate::systems::visual::building3d_cleanup::{
    building_presentation_transform, structural_presentation_state,
};
use bevy::prelude::*;
use hw_core::relationships::{StoredItems, TaskWorkers};
use hw_core::visual_mirror::{MudMixerVisualState, StockpileVisualState};
use hw_energy::{SoulSpaPhase, SoulSpaSite, SoulSpaTile};
use hw_jobs::{Building, BuildingType};
use hw_visual::{Building3dVisual, StructuralPresentationState, TopDownStructuralMaterial};

/// Only this independent root follows the logical owner's world transform.
#[derive(Component, Default)]
pub struct EquipmentRoot {
    identity: Option<BuildingAssetSetIdentity>,
    pub(super) spa_mask: u8,
    // Presentation-only phase: load creates a new root at angle zero. Idle and
    // pause retain the phase; generation replacement retains the owner's phase.
    rotor_angle: f32,
}

// M2 clay values, checked against the authoring fixtures by the regression test.
// These are not frozen production art parameters or a gameplay refinement rate.
const TANK_PARTIAL_Y: f32 = 12.0;
const TANK_FULL_Y: f32 = 22.0;
const MIXER_RADIANS_PER_SECOND: f32 = std::f32::consts::FRAC_PI_2;

fn clay_motion_allowed(pool: &BuildingAssetPool, identity: &BuildingAssetSetIdentity) -> bool {
    // The pool admits non-release identities only in profiling builds and only
    // when the complete identity matches the explicit acceptance session.
    // Release/candidate motion comes exclusively from the validated manifest.
    cfg!(feature = "profiling")
        && identity.authority == BuildingAssetAuthority::ArtPreview
        && pool.presentation_allowed(identity)
}

/// Leaves never carry Building3dVisual and therefore never receive a world
/// transform from the legacy building synchronizer. Every spawn explicitly
/// includes Visibility so state synchronization does not depend on renderer
/// plugins registering additional required components for Mesh3d.
#[derive(Component)]
struct EquipmentPart {
    name: String,
}

#[derive(Clone)]
struct GenerationMaterials {
    identity: BuildingAssetSetIdentity,
    opaque: Handle<TopDownStructuralMaterial>,
    slot_on: Handle<TopDownStructuralMaterial>,
}

#[derive(Resource, Default)]
pub(crate) struct EquipmentMaterials(Vec<GenerationMaterials>);

pub fn spawn_equipment_root(
    commands: &mut Commands,
    owner: Entity,
    kind: BuildingType,
    pos: Vec2,
    handles: &Building3dHandles,
) {
    let transform =
        building_presentation_transform(kind, &Transform::from_translation(pos.extend(0.0)));
    commands
        .spawn((
            Building3dVisual { owner },
            EquipmentRoot::default(),
            StructuralPresentationState::default(),
            transform,
            Visibility::Inherited,
            Name::new(format!("Building3dVisual ({kind:?})")),
        ))
        .with_children(|parent| {
            parent.spawn((
                EquipmentPart {
                    name: "fallback".into(),
                },
                Mesh3d(fallback_mesh(kind, handles)),
                MeshMaterial3d(fallback_material(
                    kind,
                    StructuralPresentationState::Neutral,
                    handles,
                )),
                Transform::IDENTITY,
                Visibility::Inherited,
                handles.render_layers.clone(),
            ));
        });
}

fn fallback_mesh(kind: BuildingType, handles: &Building3dHandles) -> Handle<Mesh> {
    if kind == BuildingType::Bridge {
        handles.bridge_mesh.clone()
    } else {
        handles.equipment_2x2_mesh.clone()
    }
}

fn fallback_material(
    kind: BuildingType,
    state: StructuralPresentationState,
    handles: &Building3dHandles,
) -> Handle<TopDownStructuralMaterial> {
    if kind == BuildingType::Bridge {
        return handles.bridge_material.clone();
    }
    match state {
        StructuralPresentationState::TankPartial => handles.tank_partial_material.clone(),
        StructuralPresentationState::TankFull => handles.tank_full_material.clone(),
        StructuralPresentationState::MixerIdle => handles.mixer_idle_material.clone(),
        StructuralPresentationState::MixerActive => handles.mixer_active_material.clone(),
        _ => handles.equipment_material.clone(),
    }
}

fn spa_mask(world: &mut World, owner: Entity, transform: &Transform) -> u8 {
    if world
        .get::<SoulSpaSite>(owner)
        .is_none_or(|site| site.phase != SoulSpaPhase::Operational)
    {
        return 0;
    }
    let shape = hw_jobs::placement_geometry::building_shape(BuildingType::SoulSpa);
    let anchor = hw_world::WorldMap::world_to_grid(
        transform.translation.truncate()
            - Vec2::from(shape.center_offset_tiles) * hw_core::constants::TILE_SIZE,
    );
    let mut query = world.query::<(&SoulSpaTile, Option<&TaskWorkers>)>();
    query.iter(world).fold(0, |mask, (tile, workers)| {
        if tile.parent_site != owner || workers.is_none_or(|workers| workers.is_empty()) {
            return mask;
        }
        let offset = (tile.grid_pos.0 - anchor.0, tile.grid_pos.1 - anchor.1);
        shape
            .ordered_relative_tiles
            .iter()
            .position(|value| *value == offset)
            .map_or(mask, |index| mask | (1 << index))
    })
}

/// Exclusive access keeps root/leaf replacement atomic even for a late set or
/// a newly rehydrated owner. No owner index survives world replacement.
pub fn sync_equipment_structure(world: &mut World) {
    world.resource_scope(|world, handles: Mut<Building3dHandles>| {
        world.resource_scope(|world, pool: Mut<BuildingAssetPool>| {
            sync_structure(world, &handles, &pool);
        });
    });
}

fn sync_structure(world: &mut World, handles: &Building3dHandles, pool: &BuildingAssetPool) {
    let delta = world.get_resource::<Time<Virtual>>().map_or(0.0, |time| {
        if time.is_paused() {
            0.0
        } else {
            time.delta_secs()
        }
    });
    let mut cache = world
        .remove_resource::<EquipmentMaterials>()
        .unwrap_or_default();
    cache.0.retain(|entry| {
        pool.descriptor(entry.identity.kind)
            .is_some_and(|set| set.manifest.identity == entry.identity)
    });
    let mut roots = world.query_filtered::<(Entity, &Building3dVisual), With<EquipmentRoot>>();
    let roots: Vec<_> = roots
        .iter(world)
        .map(|(entity, visual)| (entity, visual.owner))
        .collect();
    let mut seen = bevy::ecs::entity::EntityHashSet::default();
    for (root, owner) in roots {
        if !seen.insert(owner) {
            world.despawn(root);
            continue;
        }
        let Some(building) = world.get::<Building>(owner) else {
            world.despawn(root);
            continue;
        };
        let kind = building.kind;
        let Some(owner_transform) = world.get::<Transform>(owner).copied() else {
            continue;
        };
        let state = structural_presentation_state(
            kind,
            world.get::<StockpileVisualState>(owner),
            world.get::<StoredItems>(owner),
            world.get::<MudMixerVisualState>(owner),
        );
        let descriptor = production(pool, kind);
        let identity = descriptor.map(|set| &set.manifest.identity);
        let motion = descriptor.and_then(|set| {
            if clay_motion_allowed(pool, &set.manifest.identity) {
                match kind {
                    BuildingType::Tank => Some(BuildingProductionState::Tank {
                        partial_y_wu: TANK_PARTIAL_Y,
                        full_y_wu: TANK_FULL_Y,
                    }),
                    BuildingType::MudMixer => Some(BuildingProductionState::MudMixer {
                        axis: [0.0, 1.0, 0.0],
                        radians_per_second: MIXER_RADIANS_PER_SECOND,
                    }),
                    _ => None,
                }
            } else {
                set.manifest.production_state
            }
        });
        let mut transform = building_presentation_transform(kind, &owner_transform);
        if descriptor.is_some() {
            transform.translation.y = 0.0;
        }
        if world.get::<Transform>(root) != Some(&transform) {
            world.entity_mut(root).insert(transform);
        }
        if world.get::<StructuralPresentationState>(root) != Some(&state) {
            world.entity_mut(root).insert(state);
        }
        let replace = world
            .get::<EquipmentRoot>(root)
            .is_some_and(|current| current.identity.as_ref() != identity);
        if replace {
            let children: Vec<_> = world
                .get::<Children>(root)
                .map(|children| children.iter().collect())
                .unwrap_or_default();
            for child in children {
                if world.get::<EquipmentPart>(child).is_some() {
                    world.despawn(child);
                }
            }
            if let Some(set) = descriptor {
                if !cache
                    .0
                    .iter()
                    .any(|entry| &entry.identity == identity.unwrap())
                {
                    let mut assets = world.resource_mut::<Assets<TopDownStructuralMaterial>>();
                    let mut material = assets
                        .get(&handles.equipment_material)
                        .expect("equipment material initialized")
                        .clone();
                    material.base.base_color = Color::WHITE;
                    material.base.base_color_texture = set.image("albedo").cloned();
                    material.base.emissive = LinearRgba::BLACK;
                    material.base.emissive_texture = None;
                    let opaque = assets.add(material.clone());
                    // Only active Spa slots use this material. The authored
                    // RGB mask controls surface emission, never a logical light.
                    material.base.emissive = LinearRgba::WHITE;
                    material.base.emissive_texture = set.image("slot_emissive").cloned();
                    let slot_on = if kind == BuildingType::Bridge {
                        opaque.clone()
                    } else {
                        assets.add(material)
                    };
                    cache.0.push(GenerationMaterials {
                        identity: set.manifest.identity.clone(),
                        opaque,
                        slot_on,
                    });
                }
                let material = cache
                    .0
                    .iter()
                    .find(|entry| &entry.identity == identity.unwrap())
                    .unwrap();
                for part in &set.manifest.parts {
                    world.spawn((
                        ChildOf(root),
                        EquipmentPart {
                            name: part.name.clone(),
                        },
                        Name::new(part.name.clone()),
                        Mesh3d(
                            set.mesh(&part.mesh_role)
                                .expect("validated mesh role")
                                .clone(),
                        ),
                        MeshMaterial3d(material.opaque.clone()),
                        Transform {
                            translation: Vec3::from_array(part.translation_wu),
                            rotation: Quat::from_array(part.rotation_xyzw),
                            scale: Vec3::from_array(part.scale),
                        },
                        Visibility::Inherited,
                        handles.render_layers.clone(),
                    ));
                }
            } else {
                world.spawn((
                    ChildOf(root),
                    EquipmentPart {
                        name: "fallback".into(),
                    },
                    Mesh3d(fallback_mesh(kind, handles)),
                    MeshMaterial3d(fallback_material(kind, state, handles)),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    handles.render_layers.clone(),
                ));
            }
            world.get_mut::<EquipmentRoot>(root).unwrap().identity = identity.cloned();
        }
        let mask = if kind == BuildingType::SoulSpa {
            spa_mask(world, owner, &owner_transform)
        } else {
            0
        };
        if world.get::<EquipmentRoot>(root).unwrap().spa_mask != mask {
            world.get_mut::<EquipmentRoot>(root).unwrap().spa_mask = mask;
        }
        let material = identity.and_then(|id| cache.0.iter().find(|entry| &entry.identity == id));
        if let Some(BuildingProductionState::MudMixer {
            radians_per_second, ..
        }) = motion
            && kind == BuildingType::MudMixer
            && state == StructuralPresentationState::MixerActive
            && !replace
            && delta > 0.0
        {
            let mut equipment = world.get_mut::<EquipmentRoot>(root).unwrap();
            equipment.rotor_angle = (equipment.rotor_angle + delta * radians_per_second)
                .rem_euclid(std::f32::consts::TAU);
        }
        let rotor_angle = world.get::<EquipmentRoot>(root).unwrap().rotor_angle;
        let children: Vec<_> = world
            .get::<Children>(root)
            .map(|children| children.iter().collect())
            .unwrap_or_default();
        let constructing = world
            .get::<SoulSpaSite>(owner)
            .is_some_and(|site| site.phase == SoulSpaPhase::Constructing);
        let mut parts = world.query::<(
            &EquipmentPart,
            &mut Visibility,
            &mut MeshMaterial3d<TopDownStructuralMaterial>,
            &mut Transform,
        )>();
        for child in children {
            let Ok((part, mut visibility, mut handle, mut transform)) = parts.get_mut(world, child)
            else {
                continue;
            };
            if let Some(spec) = descriptor.and_then(|set| {
                set.manifest
                    .parts
                    .iter()
                    .find(|spec| spec.name == part.name)
            }) {
                if let Some(BuildingProductionState::Tank {
                    partial_y_wu,
                    full_y_wu,
                }) = motion
                    && part.name == "water"
                {
                    let y = if state == StructuralPresentationState::TankFull {
                        full_y_wu
                    } else {
                        partial_y_wu
                    };
                    if transform.translation.y != y {
                        transform.translation.y = y;
                    }
                } else if let Some(BuildingProductionState::MudMixer { axis, .. }) = motion
                    && part.name == "rotor"
                {
                    // The role origin is the shaft pivot; the fixed trough and
                    // frame are never rotated. Preserve the authored local pose.
                    let rotation = Quat::from_array(spec.rotation_xyzw)
                        * Quat::from_axis_angle(Vec3::from_array(axis), rotor_angle);
                    if transform.rotation != rotation {
                        transform.rotation = rotation;
                    }
                }
            }
            let next_visibility = if kind == BuildingType::Tank
                && part.name == "water"
                && state == StructuralPresentationState::TankEmpty
            {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *visibility != next_visibility {
                *visibility = next_visibility;
            }
            let next_material = if constructing {
                handles.equipment_material.clone()
            } else if let Some(material) = material {
                let active = part
                    .name
                    .strip_prefix("slot")
                    .and_then(|index| index.parse::<u8>().ok())
                    .is_some_and(|index| index < 4 && mask & (1 << index) != 0);
                if active {
                    material.slot_on.clone()
                } else {
                    material.opaque.clone()
                }
            } else {
                fallback_material(kind, state, handles)
            };
            if handle.0 != next_material {
                handle.0 = next_material;
            }
        }
    }
    world.insert_resource(cache);
}
