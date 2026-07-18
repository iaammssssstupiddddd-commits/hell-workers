use crate::systems::jobs::TaskSlots;
use crate::systems::jobs::floor_construction::{FloorConstructionSite, FloorTileBlueprint};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::visual_mirror::construction::{FloorSiteVisualState, FloorTileVisualMirror};
use hw_ui::selection::AreaPlacementPlan;

pub(super) fn apply_floor_placement(
    commands: &mut Commands,
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
            FloorConstructionSite::new(area.clone(), material_center, tiles_total),
            FloorSiteVisualState::default(),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("FloorConstructionSite"),
        ))
        .id();

    for &(gx, gy) in &plan.valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            FloorTileBlueprint::new(site_entity, (gx, gy)),
            FloorTileVisualMirror::default(),
            TaskSlots::new(1),
            Sprite {
                color: Color::srgba(0.5, 0.5, 0.8, 0.2),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.02)),
            Visibility::default(),
            Name::new(format!("FloorTile({},{})", gx, gy)),
        ));
    }
}
