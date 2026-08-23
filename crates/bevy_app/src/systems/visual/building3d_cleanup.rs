//! Building3dVisual クリーンアップ・マテリアル遷移システム
//!
//! - Building が削除された時、対応する Building3dVisual エンティティを despawn する。
//! - Building が仮設→本設に遷移した時、Building3dVisual のマテリアルを通常色に差し替える。

use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::structural_light_anchor_mesh_tag;
use bevy::ecs::entity::EntityHashMap;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BUILDING_FLOOR};
use hw_core::relationships::StoredItems;
use hw_core::visual_mirror::{MudMixerVisualState, StockpileVisualState};
use hw_core::world::DoorState;
use hw_jobs::{Building, BuildingType, Door};
use hw_visual::TopDownStructuralMaterial;
use hw_visual::visual3d::{
    Building3dVisual, Door3dVisual, DoorPresentationState, StructuralPresentationState,
};
/// Stable boundary used by profiling and future Light Field consumers. Door
/// domain writers run earlier; observers must run after this set.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DoorPresentationSyncSet;

/// Building エンティティ削除時に対応する Building3dVisual を despawn する。
pub fn cleanup_building_3d_visuals_system(
    mut commands: Commands,
    mut removed: RemovedComponents<Building>,
    q_visuals: Query<(Entity, &Building3dVisual)>,
) {
    for removed_entity in removed.read() {
        for (visual_entity, visual) in q_visuals.iter() {
            if visual.owner == removed_entity {
                commands.entity(visual_entity).despawn();
            }
        }
    }
}

/// 仮設壁が本設壁に遷移した時に Building3dVisual のマテリアルを通常色に差し替える。
pub fn sync_provisional_wall_material_system(
    handles_3d: Res<Building3dHandles>,
    q_buildings: Query<(Entity, &Building), Changed<Building>>,
    q_visuals: Query<(Entity, &Building3dVisual)>,
    mut q_materials: Query<&mut MeshMaterial3d<TopDownStructuralMaterial>>,
) {
    for (building_entity, building) in q_buildings.iter() {
        // 仮設から本設への遷移のみ対象
        if building.is_provisional {
            continue;
        }
        if !matches!(building.kind, hw_jobs::BuildingType::Wall) {
            continue;
        }

        for (visual_entity, visual) in q_visuals.iter() {
            if visual.owner != building_entity {
                continue;
            }
            if let Ok(mut mat) = q_materials.get_mut(visual_entity) {
                mat.0 = handles_3d.wall_material.clone();
            }
        }
    }
}

/// Converts a logical 2D building root transform into its active RtT
/// presentation transform. Translation, type height, rotation and completion
/// bounce scale are intentionally resolved independently.
pub fn building_presentation_transform(kind: BuildingType, owner: &Transform) -> Transform {
    let height = match kind {
        BuildingType::Floor => Z_BUILDING_FLOOR,
        BuildingType::Bridge => TILE_SIZE * 0.09,
        BuildingType::Door => TILE_SIZE * 0.25,
        BuildingType::Wall => TILE_SIZE * 0.5,
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::SoulSpa => TILE_SIZE * 0.4,
        BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::WheelbarrowParking
        | BuildingType::OutdoorLamp => TILE_SIZE * 0.3,
    };
    Transform {
        translation: Vec3::new(owner.translation.x, height, -owner.translation.y),
        rotation: Quat::from_rotation_y(-owner.rotation.to_euler(EulerRot::XYZ).2),
        scale: Vec3::splat(owner.scale.x),
    }
}

type BuildingVisualTransformQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static mut Transform,
        Option<&'static mut MeshTag>,
    ),
    (Without<Door3dVisual>, Without<Building>),
>;

type ChangedBuildingTransformQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Building, &'static Transform),
    (Without<Building3dVisual>, Changed<Transform>),
>;

type AddedBuildingVisualQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Building3dVisual),
    (
        Added<Building3dVisual>,
        Without<Door3dVisual>,
        Without<Building>,
    ),
>;

pub fn sync_building_3d_transform_system(
    owners: Query<(&Building, &Transform), Without<Building3dVisual>>,
    changed_owners: ChangedBuildingTransformQuery,
    mut visuals: ParamSet<(AddedBuildingVisualQuery, BuildingVisualTransformQuery)>,
) {
    let added_visuals: Vec<_> = visuals
        .p0()
        .iter()
        .map(|(entity, visual)| (entity, visual.owner))
        .collect();
    for (entity, owner_entity) in added_visuals {
        let Ok((building, owner)) = owners.get(owner_entity) else {
            continue;
        };
        let mut transform_query = visuals.p1();
        let Ok((_, mut transform, mesh_tag)) = transform_query.get_mut(entity) else {
            continue;
        };
        let next = building_presentation_transform(building.kind, owner);
        if *transform != next {
            *transform = next;
        }
        if let (Some(mut mesh_tag), Some(next_tag)) = (
            mesh_tag,
            structural_light_anchor_mesh_tag(building.kind, owner),
        ) && *mesh_tag != next_tag
        {
            *mesh_tag = next_tag;
        }
    }

    let changed: EntityHashMap<_> = changed_owners
        .iter()
        .map(|(entity, building, transform)| (entity, (building.kind, *transform)))
        .collect();
    if changed.is_empty() {
        return;
    }
    for (visual, mut transform, mesh_tag) in &mut visuals.p1() {
        let Some((kind, owner)) = changed.get(&visual.owner) else {
            continue;
        };
        let next = building_presentation_transform(*kind, owner);
        if *transform != next {
            *transform = next;
        }
        if let (Some(mut mesh_tag), Some(next_tag)) =
            (mesh_tag, structural_light_anchor_mesh_tag(*kind, owner))
            && *mesh_tag != next_tag
        {
            *mesh_tag = next_tag;
        }
    }
}

fn structural_presentation_state(
    kind: BuildingType,
    stockpile: Option<&StockpileVisualState>,
    stored_items: Option<&StoredItems>,
    mixer: Option<&MudMixerVisualState>,
) -> StructuralPresentationState {
    match kind {
        BuildingType::Tank => {
            let count = stored_items.map(StoredItems::len).unwrap_or(0);
            let capacity = stockpile.map_or(0, |value| value.capacity);
            if count == 0 {
                StructuralPresentationState::TankEmpty
            } else if capacity > 0 && count >= capacity {
                StructuralPresentationState::TankFull
            } else {
                StructuralPresentationState::TankPartial
            }
        }
        BuildingType::MudMixer => {
            if mixer.is_some_and(|value| value.is_active) {
                StructuralPresentationState::MixerActive
            } else {
                StructuralPresentationState::MixerIdle
            }
        }
        _ => StructuralPresentationState::Neutral,
    }
}

type StructuralOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building,
        Option<&'static StockpileVisualState>,
        Option<&'static StoredItems>,
        Option<&'static MudMixerVisualState>,
    ),
>;

pub fn sync_structural_presentation_state_system(
    owners: StructuralOwnerQuery,
    mut visuals: Query<(
        &Building3dVisual,
        &mut StructuralPresentationState,
        &mut MeshMaterial3d<TopDownStructuralMaterial>,
    )>,
    handles: Res<Building3dHandles>,
) {
    for (visual, mut state, mut material) in &mut visuals {
        let Ok((building, stockpile, stored_items, mixer)) = owners.get(visual.owner) else {
            continue;
        };
        let next = structural_presentation_state(building.kind, stockpile, stored_items, mixer);
        let next_material = match next {
            StructuralPresentationState::TankPartial => &handles.tank_partial_material,
            StructuralPresentationState::TankFull => &handles.tank_full_material,
            StructuralPresentationState::MixerIdle => &handles.mixer_idle_material,
            StructuralPresentationState::MixerActive => &handles.mixer_active_material,
            StructuralPresentationState::Neutral | StructuralPresentationState::TankEmpty => {
                &handles.equipment_material
            }
        };
        if *state != next {
            *state = next;
        }
        if material.0 != *next_material {
            material.0 = next_material.clone();
        }
    }
}

type DoorOwnerQuery<'w, 's> =
    Query<'w, 's, (&'static Door, &'static Transform), Without<Building3dVisual>>;

type DoorVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static mut DoorPresentationState,
        &'static mut Transform,
        &'static mut MeshMaterial3d<TopDownStructuralMaterial>,
        &'static mut MeshTag,
    ),
    Without<Door>,
>;

/// Synchronizes the active 3D leaf from the root `Door`. This system never
/// writes semantic state or `WorldMap`.
pub fn sync_door_presentation_system(
    owners: DoorOwnerQuery,
    mut visuals: DoorVisualQuery,
    handles_3d: Res<Building3dHandles>,
) {
    for (visual, mut observed_state, mut transform, mut material, mut mesh_tag) in &mut visuals {
        let Ok((door, owner_transform)) = owners.get(visual.owner) else {
            continue;
        };
        let next_state = presentation_state(door.state);
        let next_transform = door_visual_transform(owner_transform, next_state);
        let next_material = match next_state {
            DoorPresentationState::Closed => &handles_3d.door_closed_material,
            DoorPresentationState::Open => &handles_3d.door_open_material,
            DoorPresentationState::Locked => &handles_3d.door_locked_material,
        };
        if *observed_state != next_state {
            *observed_state = next_state;
        }
        if *transform != next_transform {
            *transform = next_transform;
        }
        if material.0 != *next_material {
            material.0 = next_material.clone();
        }
        if let Some(next_tag) =
            structural_light_anchor_mesh_tag(BuildingType::Door, owner_transform)
            && *mesh_tag != next_tag
        {
            *mesh_tag = next_tag;
        }
    }
}

fn presentation_state(value: DoorState) -> DoorPresentationState {
    match value {
        DoorState::Closed => DoorPresentationState::Closed,
        DoorState::Open => DoorPresentationState::Open,
        DoorState::Locked => DoorPresentationState::Locked,
    }
}

fn door_visual_transform(owner: &Transform, state: DoorPresentationState) -> Transform {
    let pos2d = owner.translation.truncate();
    let mut transform = Transform::from_xyz(pos2d.x, TILE_SIZE * 0.25, -pos2d.y)
        .with_rotation(Quat::from_rotation_y(
            -owner.rotation.to_euler(EulerRot::XYZ).2,
        ))
        .with_scale(Vec3::splat(owner.scale.x));
    if state == DoorPresentationState::Open {
        // Deterministic east hinge fallback. The asymmetric leaf and offset
        // make the state readable even before topology-owned orientation lands.
        let hinge_offset = TILE_SIZE * 0.32;
        transform.translation.x += hinge_offset;
        transform.translation.z += hinge_offset;
        transform.rotation *= Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    }
    transform
}

#[cfg(test)]
mod door_tests {
    use super::*;

    #[test]
    fn open_leaf_has_distinct_hinged_transform() {
        let owner = Transform::from_xyz(10.0, 20.0, 0.0);
        let closed = door_visual_transform(&owner, DoorPresentationState::Closed);
        let open = door_visual_transform(&owner, DoorPresentationState::Open);
        let locked = door_visual_transform(&owner, DoorPresentationState::Locked);

        assert_ne!(open.translation, closed.translation);
        assert_ne!(open.rotation, closed.rotation);
        assert_eq!(locked, closed);
    }

    #[test]
    fn moving_structural_owner_maps_xy_to_xz_and_preserves_bounce_scale() {
        let owner = Transform::from_xyz(12.0, 34.0, 0.12)
            .with_rotation(Quat::from_rotation_z(0.4))
            .with_scale(Vec3::splat(1.15));
        let visual = building_presentation_transform(BuildingType::Tank, &owner);

        assert_eq!(visual.translation, Vec3::new(12.0, TILE_SIZE * 0.4, -34.0));
        assert_eq!(visual.scale, Vec3::splat(1.15));
        assert_ne!(visual.rotation, Quat::IDENTITY);
    }

    #[test]
    fn floor_stays_above_the_coplanar_terrain_surface() {
        let owner = Transform::from_xyz(12.0, 34.0, Z_BUILDING_FLOOR);
        let visual = building_presentation_transform(BuildingType::Floor, &owner);

        assert_eq!(visual.translation, Vec3::new(12.0, Z_BUILDING_FLOOR, -34.0));
        assert!(visual.translation.y > 0.0);
    }

    #[test]
    fn tank_and_mixer_resolvers_have_finite_distinct_states() {
        let empty = structural_presentation_state(BuildingType::Tank, None, None, None);
        let active = structural_presentation_state(
            BuildingType::MudMixer,
            None,
            None,
            Some(&MudMixerVisualState { is_active: true }),
        );

        assert_eq!(empty, StructuralPresentationState::TankEmpty);
        assert_eq!(active, StructuralPresentationState::MixerActive);
    }
}
