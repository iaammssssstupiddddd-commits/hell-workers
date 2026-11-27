//! Existing utilities supplied by the client. Award, installation and energization use player input.
use super::*;
use crate::{
    interface::selection::placement_geometry::live_building_geometry,
    world::map::GeneratedWorldLayoutResource,
};
use hw_core::{GameTime, soul::DreamPool, world::DoorState};
use hw_jobs::{
    Blueprint, Building, BuildingType, Door, Equipment,
    reception::{ReceivingPanel, ReceptionContract},
};
use hw_world::WorldMap;

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("first-power-reception")
}

pub(super) fn prepare(world: &mut World) {
    world.init_resource::<hw_jobs::delivery_observations::BlueprintDeliveryReceipts>();
    let anchors = world
        .resource::<GeneratedWorldLayoutResource>()
        .layout
        .anchors
        .clone();
    let map = world.resource::<WorldMap>();
    let x_range = anchors.site.min_x..=anchors.site.max_x - 4;
    let room = anchors
        .site
        .iter_cells()
        .find(|&(x, y)| {
            x_range.contains(&x)
                && y + 4 <= anchors.site.max_y
                && (0..5).all(|dx| {
                    (0..5).all(|dy| {
                        map.is_walkable(x + dx, y + dy)
                            && !map.has_building((x + dx, y + dy))
                            && !map.has_stockpile((x + dx, y + dy))
                    })
                })
        })
        .expect("clear 5x5 room in the initial Site");
    let basin = (room.0 + 2, room.1 + 2);
    let intake_kind = BuildingType::Equipment(Equipment::Intake);
    let intake = (anchors.site.min_x..=anchors.site.max_x - 2)
        .flat_map(|x| (1..hw_core::constants::MAP_HEIGHT - 2).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            (y - anchors.site.min_y)
                .abs()
                .min((y - anchors.site.max_y).abs())
                <= 30
                && [y - 1, y + 2]
                    .into_iter()
                    .any(|bank| (x..x + 3).all(|col| map.is_river_tile(col, bank)))
                && (0..3).all(|dx| {
                    (0..2).all(|dy| {
                        let p = (x + dx, y + dy);
                        map.is_walkable(p.0, p.1)
                            && !map.has_building(p)
                            && !map.has_stockpile(p)
                            && !(p.0 >= room.0
                                && p.0 <= room.0 + 4
                                && p.1 >= room.1
                                && p.1 <= room.1 + 4)
                    })
                })
        })
        .min_by_key(|&(x, y)| (x - basin.0).abs() + (y - basin.1).abs())
        .expect("legal river bank adjacent to initial Site");
    for dx in 0..5 {
        for dy in 0..5 {
            let kind = match (dx, dy) {
                (2, 0) => BuildingType::Door,
                (0 | 4, _) | (_, 0 | 4) => BuildingType::Wall,
                _ => BuildingType::Floor,
            };
            install(world, kind, (room.0 + dx, room.1 + dy));
        }
    }
    install(world, BuildingType::Equipment(Equipment::Basin), basin);
    install(world, intake_kind, intake);
    let mut grid = intake;
    loop {
        install(world, BuildingType::Equipment(Equipment::Conduit), grid);
        if grid == basin {
            break;
        }
        if grid.0 == basin.0 {
            grid.1 += (basin.1 - grid.1).signum();
        } else {
            grid.0 += (basin.0 - grid.0).signum();
        }
    }
    let pos = WorldMap::grid_to_world(anchors.yard.min_x, anchors.yard.min_y);
    world.spawn((
        hw_energy::PowerGenerator {
            current_output: 4.,
            ..default()
        },
        Transform::from_translation(pos.extend(0.)),
    ));
    world
        .resource_mut::<hw_world::RoomDetectionState>()
        .mark_dirty(room);
    world.resource_mut::<DreamPool>().points = 10.;
    world.resource_mut::<GameTime>().seconds = 16. * 3600.;
    world.resource_mut::<Time<Virtual>>().pause();
}
fn install(world: &mut World, kind: BuildingType, anchor: (i32, i32)) {
    let geometry = live_building_geometry(world.resource::<WorldMap>(), kind, anchor);
    let pos = geometry.draw_pos;
    let mut entity = world.spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(pos.extend(0.)),
    ));
    if kind == BuildingType::Door {
        entity.insert(Door {
            state: DoorState::Closed,
        });
    }
    if let BuildingType::Equipment(e) = kind
        && e.demand() > 0.
    {
        entity.insert(hw_energy::PowerConsumer { demand: e.demand() });
    }
    let id = entity.id();
    if kind == BuildingType::Floor {
        world.resource_mut::<WorldMap>().set_floor(anchor, id);
    } else {
        world
            .resource_mut::<WorldMap>()
            .register_completed_building_footprint(kind, id, geometry.occupied_grids);
    }
    world.resource_scope::<crate::assets::GameAssets, _>(|w, assets| {
        w.resource_scope::<crate::plugins::startup::Building3dHandles, _>(|w, h| {
            crate::systems::jobs::attach_building_shell(
                &mut w.commands(),
                id,
                kind,
                false,
                pos,
                &assets,
                &h,
            )
        })
    });
    world.flush();
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let contract = world
        .query::<&ReceptionContract>()
        .single(world)
        .ok()
        .copied();
    let panel = world
        .query::<(
            Entity,
            &ReceivingPanel,
            Option<&Blueprint>,
            Option<&Building>,
        )>()
        .single(world)
        .ok();
    let state=panel.map(|(id,p,b,building)|json!({"id":id.to_bits(),"designation":p.designation,"blueprint":b.is_some(),"built":building.is_some(),"reported":p.reported,"live":p.live}));
    let point = contract.and_then(|c| {
        let (camera, transform) = world
            .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
            .single(world)
            .ok()?;
        camera
            .world_to_viewport(
                transform,
                WorldMap::grid_to_world(c.terms.point.x, c.terms.point.y).extend(0.),
            )
            .ok()
            .map(|p| p.to_array())
    });
    let epoch = world.resource::<hw_core::WorldEpoch>().get();
    let deliveries =
        world.get_resource::<hw_jobs::delivery_observations::BlueprintDeliveryReceipts>();
    let wood: Vec<_> = deliveries
        .into_iter()
        .flat_map(|r| &r.rows)
        .filter(|r| {
            r.world_epoch == epoch
                && r.kind == BuildingType::TemporaryReceivingPanel
                && r.resource == hw_core::logistics::ResourceType::Wood
        })
        .map(|r| {
            json!([
                r.owner.to_bits(),
                r.item.to_bits(),
                r.before_count,
                r.after_count
            ])
        })
        .collect();
    json!({"wood":wood,"receipt_error":deliveries.and_then(|r|r.failure),"accepted":contract.map(|c|c.accepted_at),"received":contract.and_then(|c|c.first_received_at),"scheduled":contract.map(|c|c.terms.scheduled_at),
        "deadline":contract.map(|c|c.terms.deadline),"panel":state,"screen":point,"balance":world.resource::<DreamPool>().points})
}
