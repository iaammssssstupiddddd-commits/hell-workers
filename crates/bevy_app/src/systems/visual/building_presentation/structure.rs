use super::production;
use crate::assets::building_asset_set::{BuildingAssetPool, BuildingAssetSetIdentity};
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
                Mesh3d(handles.equipment_2x2_mesh.clone()),
                MeshMaterial3d(handles.equipment_material.clone()),
                Transform::IDENTITY,
                Visibility::Inherited,
                handles.render_layers.clone(),
            ));
        });
}

fn fallback_material(
    state: StructuralPresentationState,
    handles: &Building3dHandles,
) -> Handle<TopDownStructuralMaterial> {
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
                    let opaque = assets.add(material.clone());
                    // Keep the existing emission strength. M3 owns art-approved
                    // strength; a texture alone must not invent a light source.
                    material.base.emissive_texture = set.image("slot_emissive").cloned();
                    let slot_on = assets.add(material);
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
                    Mesh3d(handles.equipment_2x2_mesh.clone()),
                    MeshMaterial3d(fallback_material(state, handles)),
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
        )>();
        for child in children {
            let Ok((part, mut visibility, mut handle)) = parts.get_mut(world, child) else {
                continue;
            };
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
                fallback_material(state, handles)
            };
            if handle.0 != next_material {
                handle.0 = next_material;
            }
        }
    }
    world.insert_resource(cache);
}
