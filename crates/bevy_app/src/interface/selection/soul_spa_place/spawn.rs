use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BUILDING_STRUCT};
use hw_energy::{GeneratesFor, SoulSpaSite, SoulSpaTile};
use hw_visual::layer::VisualLayerKind;
use hw_visual::visual3d::Building3dVisual;

/// SoulSpaSite + 4× SoulSpaTile をスポーンし、WorldMap に footprint を登録する。
/// 2D Sprite + 3D メッシュの両方を付与して即座に可視にする。
pub fn spawn_soul_spa(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    tiles: [(i32, i32); 4],
    center_pos: Vec2,
    power_grid_entity: Option<Entity>,
    game_assets: &GameAssets,
    handles_3d: &Building3dHandles,
) {
    let site_entity = commands
        .spawn((
            SoulSpaSite::default(),
            Transform::from_translation(center_pos.extend(Z_BUILDING_STRUCT)),
            Visibility::default(),
            Name::new("SoulSpaSite"),
        ))
        .with_children(|parent| {
            // 2D スプライト（VisualLayer 子エンティティ — building_completion/spawn.rs と同パターン）
            parent.spawn((
                VisualLayerKind::Struct,
                Sprite {
                    image: game_assets.rest_area.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 2.0)),
                    ..default()
                },
                Transform::default(),
                Name::new("VisualLayer (SoulSpa)"),
            ));
        })
        .id();

    if let Some(grid_entity) = power_grid_entity {
        commands
            .entity(site_entity)
            .insert(GeneratesFor(grid_entity));
    }

    for (gx, gy) in tiles {
        let tile_pos = WorldMap::grid_to_world(gx, gy);
        commands.spawn((
            SoulSpaTile {
                parent_site: site_entity,
                grid_pos: (gx, gy),
            },
            Transform::from_translation(tile_pos.extend(Z_BUILDING_STRUCT)),
            Visibility::default(),
            Name::new("SoulSpaTile"),
            ChildOf(site_entity),
        ));
    }

    // 3D ビジュアル（独立エンティティ — building_completion/spawn.rs と同パターン）
    let height = TILE_SIZE * 0.8;
    let center_y = height / 2.0;
    commands.spawn((
        Mesh3d(handles_3d.equipment_2x2_mesh.clone()),
        MeshMaterial3d(handles_3d.equipment_material.clone()),
        Transform::from_xyz(center_pos.x, center_y, -center_pos.y),
        handles_3d.render_layers.clone(),
        Building3dVisual { owner: site_entity },
        Name::new("Building3dVisual (SoulSpa)"),
    ));

    // WorldMap footprint 登録（SoulSpa は obstacle なし — occupancy のみ。Soulがタイル上を歩ける）
    for (gx, gy) in tiles {
        world_map.set_building((gx, gy), site_entity);
    }
}
