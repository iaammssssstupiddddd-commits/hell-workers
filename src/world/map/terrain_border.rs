//! 地形境界オーバーレイの生成
//!
//! 高優先度の地形が隣接する低優先度タイル上にエッジ/コーナーとしてはみ出す。
//! 優先度: Grass(3) > Dirt(2) > Sand(1) > River(0)

use crate::assets::GameAssets;
use hw_core::constants::*;
use hw_world::{TerrainBorderKind, generate_terrain_border_specs, grid_to_world};
use bevy::prelude::*;

use super::{TerrainType, WorldMapRead};

/// 境界オーバーレイであることを示すマーカー
#[derive(Component)]
pub struct TerrainBorder;

pub fn spawn_terrain_borders(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: WorldMapRead,
) {
    for spec in generate_terrain_border_specs(world_map.terrain_tiles(), MAP_WIDTH, MAP_HEIGHT) {
        let Some(texture) = border_texture(spec.terrain, spec.kind, &game_assets) else {
            continue;
        };
        let pos = grid_to_world(spec.grid.0, spec.grid.1);

        commands.spawn((
            TerrainBorder,
            Sprite {
                image: texture,
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, spec.terrain.z_layer())
                .with_rotation(Quat::from_rotation_z(spec.rotation_radians)),
        ));
    }

    info!("BEVY_STARTUP: Terrain border overlays spawned");
}

/// 地形タイプに対応する (edge, corner, inner_corner) テクスチャを返す。
/// River は最低優先度なのでオーバーレイ不要で None を返す。
fn border_texture(
    terrain: TerrainType,
    kind: TerrainBorderKind,
    assets: &GameAssets,
) -> Option<Handle<Image>> {
    match terrain {
        TerrainType::Grass => match kind {
            TerrainBorderKind::Edge => Some(assets.grass_edge.clone()),
            TerrainBorderKind::Corner => Some(assets.grass_corner.clone()),
            TerrainBorderKind::InnerCorner => Some(assets.grass_inner_corner.clone()),
        },
        TerrainType::Dirt => match kind {
            TerrainBorderKind::Edge => Some(assets.dirt_edge.clone()),
            TerrainBorderKind::Corner => Some(assets.dirt_corner.clone()),
            TerrainBorderKind::InnerCorner => Some(assets.dirt_inner_corner.clone()),
        },
        TerrainType::Sand => match kind {
            TerrainBorderKind::Edge => Some(assets.sand_edge.clone()),
            TerrainBorderKind::Corner => Some(assets.sand_corner.clone()),
            TerrainBorderKind::InnerCorner => Some(assets.sand_inner_corner.clone()),
        },
        TerrainType::River => None,
    }
}
