use super::super::{Blueprint, Building, BuildingType, Door, DoorState, ProvisionalWall};
use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use crate::systems::visual::wall_orientation_aid::attach_wall_orientation_aid;
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BUILDING_FLOOR, Z_BUILDING_STRUCT};
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

    // Phase 2: 全 BuildingType が 3D ビジュアルを使用する（Bridge は除外）
    // 2D スプライト初期画像の選択（wall_connection システムが後から上書きする）
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

    commands
        .entity(building_entity)
        .insert((
            Name::new(format!("Building ({:?})", kind)),
            // VisualLayer 子が Visibility を持つため、親にも必要（Bevy B0004）
            Visibility::Inherited,
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
        });

    // 3D ビジュアルエンティティを独立して spawn（Building の Transform を変えない）
    // Bridge は 2D のみ（spawn_building_3d_visual 側で early return）
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
fn spawn_building_3d_visual(
    commands: &mut Commands,
    owner: Entity,
    kind: BuildingType,
    pos2d: Vec2,
    is_provisional: bool,
    handles_3d: &Building3dHandles,
) {
    if matches!(kind, BuildingType::Bridge) {
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
            let entity = commands
                .spawn((
                    Mesh3d(handles_3d.wall_mesh.clone()),
                    MeshMaterial3d(material),
                    transform_3d,
                    handles_3d.render_layers.clone(),
                    Building3dVisual { owner },
                    Name::new(format!("Building3dVisual ({:?})", kind)),
                ))
                .id();
            attach_wall_orientation_aid(commands, entity, handles_3d);
        }
        BuildingType::Door => {
            let transform_3d = Transform::from_xyz(pos2d.x, TILE_SIZE * 0.25, -pos2d.y);
            commands.spawn((
                Mesh3d(handles_3d.door_mesh.clone()),
                MeshMaterial3d(handles_3d.door_material.clone()),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::Floor => {
            let transform_3d = Transform::from_xyz(pos2d.x, 0.0, -pos2d.y);
            commands.spawn((
                Mesh3d(handles_3d.floor_mesh.clone()),
                MeshMaterial3d(handles_3d.floor_material.clone()),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::WheelbarrowParking
        | BuildingType::OutdoorLamp => {
            let transform_3d = Transform::from_xyz(pos2d.x, TILE_SIZE * 0.3, -pos2d.y);
            commands.spawn((
                Mesh3d(handles_3d.equipment_1x1_mesh.clone()),
                MeshMaterial3d(handles_3d.equipment_material.clone()),
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
                Name::new(format!("Building3dVisual ({:?})", kind)),
            ));
        }
        BuildingType::Bridge => unreachable!(),
    }
}
