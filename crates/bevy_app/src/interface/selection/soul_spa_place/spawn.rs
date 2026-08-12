use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::Z_BUILDING_STRUCT;
use hw_energy::{GeneratesFor, SoulSpaSite, SoulSpaTile};

/// SoulSpaSite + 4× SoulSpaTile をスポーンし、WorldMap に footprint を登録する。
/// 2D Sprite + 3D メッシュの両方を付与して即座に可視にする。
pub(crate) fn spawn_soul_spa(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    tiles: &[(i32, i32)],
    center_pos: Vec2,
    power_grid_entity: Option<Entity>,
    game_assets: &GameAssets,
    handles_3d: &Building3dHandles,
) -> (Entity, Vec<Entity>) {
    let site_entity = commands
        .spawn((
            SoulSpaSite::default(),
            Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            },
            Transform::from_translation(center_pos.extend(Z_BUILDING_STRUCT)),
            Visibility::default(),
            Name::new("SoulSpaSite"),
        ))
        .id();

    crate::systems::jobs::attach_building_shell(
        commands,
        site_entity,
        BuildingType::SoulSpa,
        false,
        center_pos,
        game_assets,
        handles_3d,
    );

    if let Some(grid_entity) = power_grid_entity {
        commands
            .entity(site_entity)
            .insert(GeneratesFor(grid_entity));
    }

    let mut tile_entities = Vec::with_capacity(tiles.len());
    for &(gx, gy) in tiles {
        let tile_pos = WorldMap::grid_to_world(gx, gy);
        tile_entities.push(
            commands
                .spawn((
                    SoulSpaTile {
                        parent_site: site_entity,
                        grid_pos: (gx, gy),
                    },
                    Transform::from_translation(tile_pos.extend(Z_BUILDING_STRUCT)),
                    Visibility::default(),
                    Name::new("SoulSpaTile"),
                    ChildOf(site_entity),
                ))
                .id(),
        );
    }

    // WorldMap footprint 登録（SoulSpa は obstacle なし — occupancy のみ。Soulがタイル上を歩ける）
    for &(gx, gy) in tiles {
        world_map.set_building((gx, gy), site_entity);
    }

    (site_entity, tile_entities)
}
