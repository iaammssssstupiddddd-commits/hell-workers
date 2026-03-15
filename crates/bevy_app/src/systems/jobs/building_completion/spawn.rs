use super::super::{Blueprint, Building, BuildingType, Door, DoorState, ProvisionalWall};
use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use bevy::prelude::*;
use hw_core::constants::{
    TILE_SIZE, Z_BUILDING_FLOOR, Z_BUILDING_STRUCT,
};
use hw_visual::layer::VisualLayerKind;
use hw_visual::visual3d::Building3dVisual;

pub(super) fn spawn_completed_building(
    commands: &mut Commands,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
    handles_3d: &Building3dHandles,
) -> Entity {
    let is_provisional = !bp.is_fully_complete();
    let pos2d = transform.translation.truncate();

    let (z, layer_kind) = match bp.kind {
        BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => {
            (Z_BUILDING_FLOOR, VisualLayerKind::Floor)
        }
        _ => (Z_BUILDING_STRUCT, VisualLayerKind::Struct),
    };

    let parent_transform = Transform::from_xyz(pos2d.x, pos2d.y, z);

    // Phase 2: 全 BuildingType が 3D ビジュアルを使用する（Bridge は除外）
    let use_3d = !matches!(bp.kind, BuildingType::Bridge);

    // 2D スプライト初期画像の選択（wall_connection システムが後から上書きする）
    let (sprite_image_2d, custom_size_2d) = match bp.kind {
        BuildingType::Wall => (game_assets.mud_wall_isolated.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Door => (game_assets.door_closed.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Floor => (game_assets.mud_floor.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::RestArea => (game_assets.rest_area.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::SandPile => (game_assets.sand_pile.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::BonePile => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::WheelbarrowParking => {
            (game_assets.wheelbarrow_parking.clone(), Vec2::splat(TILE_SIZE * 2.0))
        }
        BuildingType::Bridge => unreachable!("Bridge uses use_3d = false path"),
    };

    let building_entity = if use_3d {
        commands
            .spawn((
                Building {
                    kind: bp.kind,
                    is_provisional,
                },
                parent_transform,
                Name::new(format!("Building ({:?})", bp.kind)),
                hw_visual::blueprint::BuildingBounceEffect {
                    bounce_animation: hw_visual::animations::BounceAnimation {
                        timer: 0.0,
                        config: hw_visual::animations::BounceAnimationConfig {
                            duration: hw_visual::blueprint::BOUNCE_DURATION,
                            min_scale: 1.0,
                            max_scale: 1.2,
                        },
                    },
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    layer_kind,
                    Sprite {
                        image: sprite_image_2d,
                        custom_size: Some(custom_size_2d),
                        ..default()
                    },
                    Transform::default(),
                    Name::new(format!("VisualLayer ({:?})", layer_kind)),
                ));
            })
            .id()
    } else {
        let (sprite_image, custom_size) = match bp.kind {
            BuildingType::Wall => unreachable!(),
            BuildingType::Door => (game_assets.door_closed.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::Floor => (game_assets.mud_floor.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
            BuildingType::MudMixer => {
                (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0))
            }
            BuildingType::RestArea => {
                (game_assets.rest_area.clone(), Vec2::splat(TILE_SIZE * 2.0))
            }
            BuildingType::Bridge => (
                game_assets.bridge.clone(),
                Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
            ),
            BuildingType::SandPile => (game_assets.sand_pile.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::BonePile => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::WheelbarrowParking => (
                game_assets.wheelbarrow_parking.clone(),
                Vec2::splat(TILE_SIZE * 2.0),
            ),
        };

        commands
            .spawn((
                Building {
                    kind: bp.kind,
                    is_provisional,
                },
                parent_transform,
                Name::new(format!("Building ({:?})", bp.kind)),
                hw_visual::blueprint::BuildingBounceEffect {
                    bounce_animation: hw_visual::animations::BounceAnimation {
                        timer: 0.0,
                        config: hw_visual::animations::BounceAnimationConfig {
                            duration: hw_visual::blueprint::BOUNCE_DURATION,
                            min_scale: 1.0,
                            max_scale: 1.2,
                        },
                    },
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    layer_kind,
                    Sprite {
                        image: sprite_image,
                        custom_size: Some(custom_size),
                        ..default()
                    },
                    Transform::default(),
                    Name::new(format!("VisualLayer ({:?})", layer_kind)),
                ));
            })
            .id()
    };

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

    // 3D ビジュアルエンティティを独立して spawn（Building の Transform を変えない）
    if use_3d {
        spawn_building_3d_visual(commands, building_entity, bp.kind, pos2d, is_provisional, handles_3d);
    }

    building_entity
}

/// Building エンティティに対応する独立 3D ビジュアルエンティティを XZ 平面上に spawn する。
///
/// 2D 座標 (x, y) → 3D 座標 (x, height/2, -y) の変換を使用する。
/// Camera3d は up=NEG_Z で XZ 平面を俯瞰するため、2D +y = 3D -z。
fn spawn_building_3d_visual(
    commands: &mut Commands,
    owner: Entity,
    kind: BuildingType,
    pos2d: Vec2,
    is_provisional: bool,
    handles_3d: &Building3dHandles,
) {
    let (mesh, material, height) = match kind {
        BuildingType::Wall => {
            let mat = if is_provisional {
                handles_3d.wall_provisional_material.clone()
            } else {
                handles_3d.wall_material.clone()
            };
            (handles_3d.wall_mesh.clone(), mat, TILE_SIZE)
        }
        BuildingType::Door => (
            handles_3d.door_mesh.clone(),
            handles_3d.door_material.clone(),
            TILE_SIZE * 0.5,
        ),
        BuildingType::Floor => (
            handles_3d.floor_mesh.clone(),
            handles_3d.floor_material.clone(),
            0.0,
        ),
        BuildingType::SandPile | BuildingType::BonePile => (
            handles_3d.equipment_1x1_mesh.clone(),
            handles_3d.equipment_material.clone(),
            TILE_SIZE * 0.6,
        ),
        BuildingType::WheelbarrowParking => (
            handles_3d.equipment_1x1_mesh.clone(),
            handles_3d.equipment_material.clone(),
            TILE_SIZE * 0.6,
        ),
        BuildingType::Tank | BuildingType::MudMixer | BuildingType::RestArea => (
            handles_3d.equipment_2x2_mesh.clone(),
            handles_3d.equipment_material.clone(),
            TILE_SIZE * 0.8,
        ),
        BuildingType::Bridge => return, // Bridge は 2D スプライトのまま（Phase 2 対象外）
    };

    // Floor は XZ 平面に平置き（y=0）、それ以外は高さの中心に配置
    let center_y = height / 2.0;
    let transform_3d = Transform::from_xyz(pos2d.x, center_y, -pos2d.y);

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform_3d,
        handles_3d.render_layers.clone(),
        Building3dVisual { owner },
        Name::new(format!("Building3dVisual ({:?})", kind)),
    ));
}

