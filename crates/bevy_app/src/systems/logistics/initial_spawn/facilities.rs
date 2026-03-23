use crate::assets::GameAssets;
use crate::systems::jobs::{Building, BuildingType, ObstaclePosition, TaskSlots};
use crate::systems::logistics::{
    BelongsTo, ResourceItem, ResourceType, Wheelbarrow, WheelbarrowParking,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::relationships::{LoadedItems, ParkedAt};
use hw_world::zones::{PairedSite, PairedYard, Site, Yard};

use super::layout::{ParkingLayout, SiteYardLayout};

const INITIAL_WHEELBARROW_PARKING_CAPACITY: usize = 2;

/// Site と Yard エンティティをスポーンしてペアリングする。
/// レイアウト計算は呼び出し元で完了済みであること。
pub fn spawn_site_and_yard(commands: &mut Commands, layout: &SiteYardLayout) {
    let site_min = WorldMap::grid_to_world(layout.site_min_x, layout.site_min_y);
    let site_max = WorldMap::grid_to_world(layout.site_max_x, layout.site_max_y);
    let yard_min = WorldMap::grid_to_world(layout.yard_min_x, layout.yard_min_y);
    let yard_max = WorldMap::grid_to_world(layout.yard_max_x, layout.yard_max_y);

    let site_entity = commands
        .spawn((
            Name::new("Initial Site"),
            Site {
                min: site_min,
                max: site_max,
            },
        ))
        .id();

    let yard_entity = commands
        .spawn((
            Name::new("Initial Yard"),
            Yard {
                min: yard_min,
                max: yard_max,
            },
            PairedSite(site_entity),
        ))
        .id();

    commands.entity(site_entity).insert(PairedYard(yard_entity));

    info!(
        "INITIAL_SPAWN: spawned Site {:?}-{:?} and Yard {:?}-{:?}",
        site_min, site_max, yard_min, yard_max
    );
}

/// WheelbarrowParking ビルディングと初期 Wheelbarrow をスポーンする。
/// `layout` は事前に walkability 検証済みであること。
pub fn spawn_wheelbarrow_parking(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
    layout: &ParkingLayout,
) {
    let base = layout.base;
    let occupied = layout.occupied;

    let building_pos = WorldMap::grid_to_world(base.0, base.1) + Vec2::splat(TILE_SIZE * 0.5);
    let building_entity = commands
        .spawn((
            Building {
                kind: BuildingType::WheelbarrowParking,
                is_provisional: false,
            },
            WheelbarrowParking {
                capacity: INITIAL_WHEELBARROW_PARKING_CAPACITY,
            },
            Sprite {
                image: game_assets.wheelbarrow_parking.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 2.0)),
                ..default()
            },
            Transform::from_xyz(building_pos.x, building_pos.y, Z_ITEM_OBSTACLE),
            Name::new("Initial Wheelbarrow Parking"),
        ))
        .id();

    commands.entity(building_entity).with_children(|parent| {
        for (gx, gy) in occupied {
            parent.spawn((ObstaclePosition(gx, gy), Name::new("Building Obstacle")));
        }
    });

    world_map.register_completed_building_footprint(
        BuildingType::WheelbarrowParking,
        building_entity,
        occupied,
    );

    let offsets = [Vec2::new(-8.0, -8.0), Vec2::new(8.0, 8.0)];
    for i in 0..INITIAL_WHEELBARROW_PARKING_CAPACITY {
        let offset = offsets
            .get(i % offsets.len())
            .copied()
            .unwrap_or(Vec2::ZERO);
        let pos = building_pos + offset;

        commands.spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow {
                capacity: WHEELBARROW_CAPACITY,
            },
            BelongsTo(building_entity),
            ParkedAt(building_entity),
            LoadedItems::default(),
            TaskSlots::new(1),
            Sprite {
                image: game_assets.wheelbarrow_empty.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_PICKUP),
            Visibility::Visible,
            Name::new(format!("Initial Wheelbarrow #{}", i)),
        ));
    }
}
