use super::super::{Blueprint, Building, BuildingType, Door, DoorState, ProvisionalWall};
use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BUILDING_FLOOR, Z_BUILDING_STRUCT};
use hw_visual::layer::VisualLayerKind;
use hw_visual::visual3d::{
    Building3dVisual, Door3dVisual, DoorPresentationState, StructuralPresentationState,
};
use hw_world::WorldMap;

const LIGHT_ANCHOR_PRESENT: u32 = 1 << 31;
const LIGHT_ANCHOR_POLICY_SHIFT: u32 = 18;
const LIGHT_ANCHOR_POLICY_WALL: u32 = 1;
const LIGHT_ANCHOR_POLICY_DOOR: u32 = 2;

pub(crate) fn structural_light_anchor_mesh_tag(
    kind: BuildingType,
    owner: &Transform,
) -> Option<MeshTag> {
    let policy = match kind {
        BuildingType::Wall => LIGHT_ANCHOR_POLICY_WALL,
        BuildingType::Door => LIGHT_ANCHOR_POLICY_DOOR,
        _ => return None,
    };
    let (grid_x, grid_y) = WorldMap::world_to_grid(owner.translation.truncate());
    let (Ok(grid_x), Ok(grid_y)) = (u32::try_from(grid_x), u32::try_from(grid_y)) else {
        return None;
    };
    if grid_x > u32::from(u8::MAX) || grid_y > u32::from(u8::MAX) {
        return None;
    }

    let local_north = owner.rotation * Vec3::Y;
    let direction = if local_north.x.abs() > local_north.y.abs() {
        if local_north.x >= 0.0 { 1 } else { 3 }
    } else if local_north.y >= 0.0 {
        0
    } else {
        2
    };
    Some(MeshTag(
        LIGHT_ANCHOR_PRESENT
            | grid_x
            | (grid_y << 8)
            | (direction << 16)
            | (policy << LIGHT_ANCHOR_POLICY_SHIFT),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderPresentationClass {
    Structural3d,
    Foreground2d,
}

pub(crate) const fn presentation_class(kind: BuildingType) -> RenderPresentationClass {
    match kind {
        BuildingType::Wall
        | BuildingType::Door
        | BuildingType::Floor
        | BuildingType::Bridge
        | BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::SoulSpa => RenderPresentationClass::Structural3d,
        BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::WheelbarrowParking
        | BuildingType::OutdoorLamp => RenderPresentationClass::Foreground2d,
    }
}

pub(super) fn spawn_completed_building(
    commands: &mut Commands,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
    handles_3d: &Building3dHandles,
) -> Entity {
    let is_provisional = !bp.is_fully_complete();
    let pos2d = transform.translation.truncate();

    let z = match bp.kind {
        BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => Z_BUILDING_FLOOR,
        _ => Z_BUILDING_STRUCT,
    };

    let building_entity = commands
        .spawn((
            Building {
                kind: bp.kind,
                is_provisional,
            },
            Transform::from_xyz(pos2d.x, pos2d.y, z),
        ))
        .id();

    attach_building_shell(
        commands,
        building_entity,
        bp.kind,
        is_provisional,
        pos2d,
        game_assets,
        handles_3d,
    );

    if bp.kind == BuildingType::Wall && is_provisional {
        commands
            .entity(building_entity)
            .insert(ProvisionalWall::default());
    }

    if bp.kind == BuildingType::Door {
        commands.entity(building_entity).insert(Door {
            state: DoorState::Closed,
        });
    }

    building_entity
}

/// Building の「シェル」を付与する: Name / 完成バウンス演出 / 2D VisualLayer 子エンティティ /
/// 独立 3D ビジュアルエンティティ。
///
/// 建築完成時（`spawn_completed_building`）とセーブデータのロード後（rehydrate）の
/// 両方から呼ばれる。永続化される simulation 状態（`Building` / `Door` /
/// `ProvisionalWall` / `Transform`）はここに含めないこと。
/// 壁の 2D スプライトは初期画像を入れておけば `wall_connection` システムが上書きする。
pub(crate) fn attach_building_shell(
    commands: &mut Commands,
    building_entity: Entity,
    kind: BuildingType,
    is_provisional: bool,
    pos2d: Vec2,
    game_assets: &GameAssets,
    handles_3d: &Building3dHandles,
) {
    let layer_kind = match kind {
        BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => {
            VisualLayerKind::Floor
        }
        _ => VisualLayerKind::Struct,
    };

    // Foreground2d uses this Sprite as its one active presentation. Structural
    // buildings are represented only by their owner-linked 3D visual.
    let (sprite_image_2d, custom_size_2d) = match kind {
        BuildingType::Wall => (
            game_assets.mud_wall_isolated.clone(),
            Vec2::splat(TILE_SIZE),
        ),
        BuildingType::Door => (game_assets.door_closed.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Floor => (game_assets.mud_floor.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::RestArea => (game_assets.rest_area.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::SandPile => (game_assets.sand_pile.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::BonePile => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::WheelbarrowParking => (
            game_assets.wheelbarrow_parking.clone(),
            Vec2::splat(TILE_SIZE * 2.0),
        ),
        BuildingType::Bridge => (
            game_assets.bridge.clone(),
            Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
        ),
        BuildingType::SoulSpa => (game_assets.rest_area.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::OutdoorLamp => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE)),
    };

    let class = presentation_class(kind);
    let mut owner = commands.entity(building_entity);
    owner.insert((
        Name::new(format!("Building ({:?})", kind)),
        // Presentation children carry Visibility, so the owner must participate
        // in inherited visibility as well (Bevy B0004).
        Visibility::Inherited,
        hw_visual::blueprint::BuildingBounceEffect::completion(),
    ));

    if class == RenderPresentationClass::Foreground2d {
        owner.with_children(|parent| {
            parent.spawn((
                layer_kind,
                Sprite {
                    image: sprite_image_2d,
                    custom_size: Some(custom_size_2d),
                    ..default()
                },
                Transform::default(),
                Visibility::Inherited,
                Name::new(format!("VisualLayer ({:?})", layer_kind)),
            ));
        });
    }

    // Structural3d visuals are independent entities so the logical Building
    // transform stays in the 2D simulation coordinate system.
    spawn_building_3d_visual(
        commands,
        building_entity,
        kind,
        pos2d,
        is_provisional,
        handles_3d,
    );
}

/// Building エンティティに対応する独立 3D ビジュアルエンティティを XZ 平面上に spawn する。
///
/// 2D 座標 (x, y) → 3D 座標 (x, height/2, -y) の変換を使用する。
/// Camera3d は up=NEG_Z で XZ 平面を俯瞰するため、2D +y = 3D -z。
pub(crate) fn spawn_building_3d_visual(
    commands: &mut Commands,
    owner: Entity,
    kind: BuildingType,
    pos2d: Vec2,
    is_provisional: bool,
    handles_3d: &Building3dHandles,
) {
    if presentation_class(kind) == RenderPresentationClass::Foreground2d {
        return;
    }

    match kind {
        BuildingType::Wall => {
            let material = if is_provisional {
                handles_3d.wall_provisional_material.clone()
            } else {
                handles_3d.wall_material.clone()
            };
            let transform_3d = Transform::from_xyz(pos2d.x, TILE_SIZE * 0.5, -pos2d.y);
            let owner_transform = Transform::from_xyz(pos2d.x, pos2d.y, 0.0);
            commands.spawn((
                Mesh3d(handles_3d.wall_mesh.clone()),
                MeshMaterial3d(material),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                structural_light_anchor_mesh_tag(kind, &owner_transform)
                    .expect("Wall grid anchor fits MeshTag"),
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::Door => {
            let transform_3d = Transform::from_xyz(pos2d.x, TILE_SIZE * 0.25, -pos2d.y);
            let owner_transform = Transform::from_xyz(pos2d.x, pos2d.y, 0.0);
            commands.spawn((
                Mesh3d(handles_3d.door_mesh.clone()),
                MeshMaterial3d(handles_3d.door_closed_material.clone()),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                Door3dVisual { owner },
                DoorPresentationState::Closed,
                structural_light_anchor_mesh_tag(kind, &owner_transform)
                    .expect("Door grid anchor fits MeshTag"),
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::Floor => {
            let transform_3d = Transform::from_xyz(pos2d.x, Z_BUILDING_FLOOR, -pos2d.y);
            commands.spawn((
                Mesh3d(handles_3d.floor_mesh.clone()),
                MeshMaterial3d(handles_3d.floor_material.clone()),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::SoulSpa => {
            let transform_3d = Transform::from_xyz(pos2d.x, TILE_SIZE * 0.4, -pos2d.y);
            commands.spawn((
                Mesh3d(handles_3d.equipment_2x2_mesh.clone()),
                MeshMaterial3d(handles_3d.equipment_material.clone()),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                match kind {
                    BuildingType::Tank => StructuralPresentationState::TankEmpty,
                    BuildingType::MudMixer => StructuralPresentationState::MixerIdle,
                    _ => StructuralPresentationState::Neutral,
                },
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::Bridge => {
            commands.spawn((
                Mesh3d(handles_3d.bridge_mesh.clone()),
                MeshMaterial3d(handles_3d.bridge_material.clone()),
                Transform::from_xyz(pos2d.x, TILE_SIZE * 0.09, -pos2d.y),
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                Name::new("Building3dVisual (Bridge)"),
            ));
        }
        BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::WheelbarrowParking
        | BuildingType::OutdoorLamp => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;

    #[test]
    fn every_building_has_an_explicit_p02_presentation_class() {
        let structural = BuildingType::ALL
            .into_iter()
            .filter(|kind| presentation_class(*kind) == RenderPresentationClass::Structural3d)
            .count();
        let foreground = BuildingType::ALL.len() - structural;

        assert_eq!(structural, 8);
        assert_eq!(foreground, 4);
    }

    #[test]
    fn door_light_anchor_stays_on_the_logical_root_at_edges_and_corners() {
        for (grid, direction, rotation) in [
            ((0, 0), 0, Quat::IDENTITY),
            (
                (99, 0),
                1,
                Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            ),
            ((99, 99), 2, Quat::from_rotation_z(std::f32::consts::PI)),
            (
                (0, 99),
                3,
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ),
        ] {
            let world = WorldMap::grid_to_world(grid.0, grid.1);
            let owner = Transform::from_translation(world.extend(0.0)).with_rotation(rotation);
            let tag = structural_light_anchor_mesh_tag(BuildingType::Door, &owner)
                .expect("canonical Door root fits MeshTag")
                .0;

            assert_ne!(tag & LIGHT_ANCHOR_PRESENT, 0);
            assert_eq!(tag & 0xff, grid.0 as u32);
            assert_eq!((tag >> 8) & 0xff, grid.1 as u32);
            assert_eq!((tag >> 16) & 0x3, direction);
            assert_eq!(
                (tag >> LIGHT_ANCHOR_POLICY_SHIFT) & 0x3,
                LIGHT_ANCHOR_POLICY_DOOR
            );
        }
    }

    #[test]
    fn fallback_wall_spawn_has_exactly_one_visual_mesh_material_and_tag() {
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<hw_visual::TopDownStructuralMaterial>::default();
        let wall_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE));
        let complete_material = materials.add(hw_visual::TopDownStructuralMaterial::default());
        let provisional_material = materials.add(hw_visual::TopDownStructuralMaterial::default());
        let mut handles = crate::test_support::empty_building_3d_handles();
        handles.wall_mesh = wall_mesh.clone();
        handles.wall_material = complete_material.clone();
        handles.wall_provisional_material = provisional_material.clone();
        let pos2d = WorldMap::grid_to_world(3, 4);

        for (is_provisional, expected_material) in
            [(false, &complete_material), (true, &provisional_material)]
        {
            let mut world = World::new();
            let owner = world.spawn_empty().id();
            world.insert_resource(crate::assets::wall_asset_set::WallProductionActivation {
                topology_ready: true,
                decision_revision: 1,
                state: crate::assets::wall_asset_set::WallProductionActivationState::ReadyToApply {
                    asset_set_generation: 1,
                    authority: crate::assets::wall_asset_set::WallAssetAuthority::IsolatedCandidate,
                    manifest_sha256: "b".repeat(64),
                    asset_activation_revision: 1,
                },
            });
            let mut queue = CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, &world);
                spawn_building_3d_visual(
                    &mut commands,
                    owner,
                    BuildingType::Wall,
                    pos2d,
                    is_provisional,
                    &handles,
                );
            }
            queue.apply(&mut world);

            let visual_count = world.query::<&Building3dVisual>().iter(&world).count();
            assert_eq!(visual_count, 1);
            let mut query = world.query::<(
                &Building3dVisual,
                &Mesh3d,
                &MeshMaterial3d<hw_visual::TopDownStructuralMaterial>,
                &MeshTag,
                &Transform,
            )>();
            let rows = query.iter(&world).collect::<Vec<_>>();
            assert_eq!(rows.len(), 1);
            let (visual, mesh, material, _, transform) = rows[0];
            assert_eq!(visual.owner, owner);
            assert_eq!(&mesh.0, &wall_mesh);
            assert_eq!(&material.0, expected_material);
            assert_eq!(
                transform.translation,
                Vec3::new(pos2d.x, TILE_SIZE * 0.5, -pos2d.y)
            );
        }
    }
}
