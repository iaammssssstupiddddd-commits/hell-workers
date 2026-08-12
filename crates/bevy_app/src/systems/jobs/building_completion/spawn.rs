use super::super::{Blueprint, Building, BuildingType, Door, DoorState, ProvisionalWall};
use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use crate::systems::visual::wall_orientation_aid::attach_wall_orientation_aid;
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BUILDING_FLOOR, Z_BUILDING_STRUCT};
use hw_visual::layer::VisualLayerKind;
use hw_visual::visual3d::{
    Building3dVisual, Door3dVisual, DoorPresentationState, LegacyStructural2dMirror,
    StructuralPresentationState,
};

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

/// Whether P02 still needs a hidden 2D state mirror for an active 3D building.
///
/// Door state synchronization and the legacy Tank / MudMixer state consumers
/// still write Sprite handles. Other structural kinds have no such consumer,
/// so retaining a Sprite for them would only preserve duplicate topology.
pub(crate) const fn requires_legacy_structural_2d_mirror(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Door | BuildingType::Tank | BuildingType::MudMixer
    )
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

    // Door/Tank/MudMixer keep a hidden legacy mirror for state consumers until
    // P08; Foreground2d uses this Sprite as its one active presentation.
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
    ));

    if class == RenderPresentationClass::Foreground2d || requires_legacy_structural_2d_mirror(kind)
    {
        owner.with_children(|parent| {
            let mut visual = parent.spawn((
                layer_kind,
                Sprite {
                    image: sprite_image_2d,
                    custom_size: Some(custom_size_2d),
                    ..default()
                },
                Transform::default(),
                if class == RenderPresentationClass::Structural3d {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                Name::new(format!("VisualLayer ({:?})", layer_kind)),
            ));
            if class == RenderPresentationClass::Structural3d {
                visual.insert(LegacyStructural2dMirror);
            }
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
                MeshMaterial3d(handles_3d.door_closed_material.clone()),
                transform_3d,
                handles_3d.render_layers.clone(),
                Building3dVisual { owner },
                Door3dVisual { owner },
                DoorPresentationState::Closed,
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
    fn only_structural_state_consumers_keep_a_legacy_2d_mirror() {
        let mirrored = BuildingType::ALL
            .into_iter()
            .filter(|kind| requires_legacy_structural_2d_mirror(*kind))
            .collect::<Vec<_>>();

        assert_eq!(
            mirrored,
            [
                BuildingType::Door,
                BuildingType::Tank,
                BuildingType::MudMixer,
            ]
        );
    }
}
