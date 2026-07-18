use crate::systems::jobs::TaskSlots;
use crate::systems::jobs::wall_construction::{WallConstructionSite, WallTileBlueprint};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::visual_mirror::construction::{WallSiteVisualState, WallTileVisualMirror};
use hw_ui::selection::AreaPlacementPlan;

pub(super) fn apply_wall_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    area: &crate::systems::command::TaskArea,
    plan: &AreaPlacementPlan,
) {
    let Some(&center_grid) = plan.valid_tiles.get(plan.valid_tiles.len() / 2) else {
        return;
    };
    let tiles_total = plan.valid_tiles.len() as u32;
    let material_center = WorldMap::grid_to_world(center_grid.0, center_grid.1);

    let site_entity = commands
        .spawn((
            WallConstructionSite::new(area.clone(), material_center, tiles_total),
            WallSiteVisualState::default(),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("WallConstructionSite"),
        ))
        .id();

    for &(gx, gy) in &plan.valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            WallTileBlueprint::new(site_entity, (gx, gy)),
            WallTileVisualMirror::default(),
            TaskSlots::new(1),
            Sprite {
                color: Color::srgba(0.8, 0.55, 0.3, 0.25),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.02)),
            Visibility::default(),
            Name::new(format!("WallTile({},{})", gx, gy)),
        ));

        world_map.set_building_occupancy((gx, gy), site_entity);
    }
}
