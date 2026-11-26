//! A paused native scene with 3×3 and 4×3 rooms sharing one completed vent.
use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use bevy::prelude::*;
use hw_core::world::DoorState;
use hw_jobs::{Building, BuildingType, Door};
use hw_world::WorldMap;

#[derive(Resource)]
struct FixtureOrigin((i32, i32));

pub(super) fn prepare(world: &mut World, exterior_ports: bool) {
    world
        .resource_mut::<hw_ui::world_view::WorldViewState>()
        .manual = hw_ui::world_view::WorldView::Rooms;
    let sites: Vec<_> = world
        .query::<&hw_world::Site>()
        .iter(world)
        .cloned()
        .collect();
    let yards: Vec<_> = world
        .query::<&hw_world::Yard>()
        .iter(world)
        .cloned()
        .collect();
    let control =
        std::env::var("HW_NATIVE_UI_LAYOUT_SCENE").as_deref() == Ok("ventilation-control");
    let map = world.resource::<WorldMap>();
    let origin = (10..80)
        .flat_map(|y| (10..80).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let pos = WorldMap::grid_to_world(x + 1, y + 2);
            (!control
                || (sites.iter().all(|s| !s.contains(pos))
                    && yards.iter().all(|s| !s.contains(pos))))
                && (0..10).all(|dx| {
                    (0..5).all(|dy| {
                        let grid = (x + dx, y + dy);
                        map.is_walkable(grid.0, grid.1)
                            && !map.has_building(grid)
                            && !map.has_stockpile(grid)
                            && map
                                .pos_to_idx(grid.0, grid.1)
                                .and_then(|idx| map.terrain_at_idx(idx))
                                == Some(hw_world::terrain::TerrainType::Grass)
                    })
                })
        })
        .min_by_key(|&(x, y)| (x - 52_i32).abs() + (y - 40_i32).abs())
        .expect("generated world contains a dry 10x5 clearing");
    world.insert_resource(FixtureOrigin(origin));
    let mut selected = None;
    world.resource_scope::<GameAssets, _>(|world, assets| {
        world.resource_scope::<Building3dHandles, _>(|world, handles| {
            for x in 0..10 {
                for y in 0..5 {
                    let kind = match (x, y) {
                        (4, 2) => BuildingType::ScreamingVent,
                        (0 | 9, 2) if exterior_ports => BuildingType::ScreamingVent,
                        (2 | 7, 0) | (4, 1) => BuildingType::Door,
                        (0 | 4 | 9, _) | (_, 0 | 4) => BuildingType::Wall,
                        _ => BuildingType::Floor,
                    };
                    let grid = (origin.0 + x, origin.1 + y);
                    let pos = WorldMap::grid_to_world(grid.0, grid.1);
                    let entity = world
                        .spawn((
                            Building {
                                kind,
                                is_provisional: false,
                            },
                            Transform::from_translation(pos.extend(0.0)),
                        ))
                        .id();
                    if kind == BuildingType::Door {
                        world.entity_mut(entity).insert(Door {
                            state: DoorState::Closed,
                        });
                    }
                    if kind == BuildingType::Floor {
                        world.resource_mut::<WorldMap>().set_floor(grid, entity);
                    } else {
                        world
                            .resource_mut::<WorldMap>()
                            .register_completed_building_footprint(kind, entity, [grid]);
                    }
                    crate::systems::jobs::attach_building_shell(
                        &mut world.commands(),
                        entity,
                        kind,
                        false,
                        pos,
                        &assets,
                        &handles,
                    );
                    if (x, y) == (4, 2) {
                        selected = Some(entity);
                    }
                }
            }
        });
    });
    world.flush();
    world
        .resource_mut::<hw_world::RoomDetectionState>()
        .mark_dirty(origin);
    // The paused scene relies on the regular room and ventilation systems to publish its data.
    world.resource_mut::<hw_ui::selection::SelectedEntity>().0 = selected;
    let center = WorldMap::grid_to_world(origin.0 + 5, origin.1 + 2);
    for (mut camera, mut controller) in world
        .query_filtered::<(
            &mut Transform,
            &mut bevy::camera_controller::pan_camera::PanCamera,
        ), With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        camera.translation.x = center.x;
        camera.translation.y = center.y;
        controller.zoom_factor = 0.38;
        camera.scale = Vec3::splat(controller.zoom_factor);
    }
}

pub(super) fn observations(world: &World) -> serde_json::Value {
    let tiles = world.resource::<hw_world::RoomTileLookup>();
    let (x, y) = world.resource::<FixtureOrigin>().0;
    let rooms = [(x + 2, y + 2), (x + 6, y + 2)].map(|grid| {
        let room = tiles.tile_to_room.get(&grid)?;
        let ventilation = world.get::<crate::systems::ventilation::RoomVentilation>(*room)?;
        let children = world.get::<Children>(*room)?;
        let mut colors = Vec::new();
        for child in children.iter() {
            if !world.entity(child).contains::<hw_world::RoomOverlayTile>()
                || !world.get::<InheritedVisibility>(child).is_some_and(|visibility| visibility.get())
                || !world.get::<ViewVisibility>(child).is_some_and(|visibility| visibility.get()) {
                continue;
            }
            let color = world.get::<Sprite>(child)?.color.to_srgba();
            colors.push([color.red, color.green, color.blue]);
        }
        Some(serde_json::json!({"supply": ventilation.0.supplied(), "need": ventilation.0.required(), "visible_borders": colors}))
    });
    serde_json::json!(rooms)
}

pub(super) fn visible_selected_grates(world: &mut World) -> usize {
    use hw_visual::{TopDownStructuralMaterial, visual3d::Building3dVisual};
    let selected = world.resource::<hw_ui::selection::SelectedEntity>().0;
    let mut roots = world.query::<(&Building3dVisual, &Children)>();
    let handles = world.resource::<Building3dHandles>();
    roots
        .iter(world)
        .filter(|(visual, _)| Some(visual.owner) == selected)
        .flat_map(|(_, children)| children.iter())
        .filter(|&child| {
            world
                .get::<Mesh3d>(child)
                .is_some_and(|m| m.0 == handles.vent_grate_mesh)
                && world
                    .get::<MeshMaterial3d<TopDownStructuralMaterial>>(child)
                    .is_some_and(|m| m.0 == handles.vent_material)
                && world
                    .get::<Transform>(child)
                    .is_some_and(|pose| pose.scale.z == 2.0)
                && world
                    .get::<InheritedVisibility>(child)
                    .is_some_and(|v| v.get())
                && world.get::<ViewVisibility>(child).is_some_and(|v| v.get())
        })
        .count()
}

#[derive(Resource)]
pub(super) struct ControlFixture {
    fan: Entity,
    generator: Entity,
    stage: u8,
    outage_at: u64,
    restored_at: u64,
    phase_rates: Vec<[f64; 2]>,
}

pub(super) fn prepare_control(world: &mut World) {
    prepare(world, true);
    let (x, y) = world.resource::<FixtureOrigin>().0;
    let pos = WorldMap::grid_to_world(x + 1, y + 2);
    // Give this isolated acceptance circuit its own small site and generator.
    let yard = world
        .spawn(hw_world::Yard {
            min: pos - Vec2::splat(1.0),
            max: pos + Vec2::splat(1.0),
        })
        .id();
    let site = world
        .spawn((
            hw_world::Site {
                min: pos - Vec2::splat(1.0),
                max: pos + Vec2::splat(1.0),
            },
            hw_world::PairedYard(yard),
        ))
        .id();
    world.entity_mut(yard).insert(hw_world::PairedSite(site));
    let kind = BuildingType::Equipment(hw_jobs::Equipment::SupplyFan);
    let fan = world
        .spawn((
            Building {
                kind,
                is_provisional: false,
            },
            Transform::from_translation(pos.extend(hw_core::constants::Z_BUILDING_STRUCT)),
            hw_energy::PowerConsumer { demand: 0.5 },
        ))
        .id();
    world
        .resource_mut::<WorldMap>()
        .register_completed_building_footprint(kind, fan, [(x + 1, y + 2)]);
    world.resource_scope::<GameAssets, _>(|w, assets| {
        w.resource_scope::<Building3dHandles, _>(|w, handles| {
            crate::systems::jobs::attach_building_shell(
                &mut w.commands(),
                fan,
                kind,
                false,
                pos,
                &assets,
                &handles,
            );
        })
    });
    world.flush();
    let generator = world
        .spawn((
            hw_energy::PowerGenerator {
                current_output: 4.0,
                ..default()
            },
            Transform::from_translation(pos.extend(0.0)),
        ))
        .id();
    world.resource_mut::<hw_ui::selection::SelectedEntity>().0 = Some(fan);
    world.insert_resource(ControlFixture {
        fan,
        generator,
        stage: 0,
        outage_at: 0,
        restored_at: 0,
        phase_rates: Vec::new(),
    });
}

pub(super) fn observe_control(world: &mut World) {
    if !world.contains_resource::<ControlFixture>() {
        return;
    }
    let Some(tick) = world
        .query::<&crate::systems::ventilation::climate::AirLedger>()
        .iter(world)
        .next()
        .map(|l| l.tick)
    else {
        return;
    };
    world.resource_scope(|world, mut scene: Mut<ControlFixture>| {
        use hw_energy::{PowerGenerator, PowerSupplyState as Power};
        let supply = world.get::<Power>(scene.fan).copied();
        let room_rates = |world: &World| {
            let (x, y) = world.resource::<FixtureOrigin>().0;
            let tiles = world.resource::<hw_world::RoomTileLookup>();
            [x + 2, x + 6].map(|x| {
                let room = tiles.tile_to_room[&(x, y + 2)];
                world
                    .get::<crate::systems::ventilation::RoomVentilation>(room)
                    .expect("resolved fixture room")
                    .0
                    .supplied()
            })
        };
        match scene.stage {
            0 if supply == Some(Power::Supplied) && tick >= 2 => {
                scene.phase_rates.push(room_rates(world));
                world
                    .get_mut::<PowerGenerator>(scene.generator)
                    .unwrap()
                    .current_output = 0.0;
                scene.outage_at = tick;
                scene.stage = 1;
            }
            1 if matches!(supply, Some(Power::Shed { .. })) && tick >= scene.outage_at + 2 => {
                scene.phase_rates.push(room_rates(world));
                world
                    .get_mut::<PowerGenerator>(scene.generator)
                    .unwrap()
                    .current_output = 4.0;
                world
                    .entity_mut(scene.fan)
                    .insert(crate::systems::ventilation::climate::ClimateSetting(2));
                scene.restored_at = tick;
                scene.stage = 2;
            }
            2 if supply == Some(Power::Supplied) && tick >= scene.restored_at + 2 => {
                scene.phase_rates.push(room_rates(world));
                scene.stage = 3;
                world.resource_mut::<Time<Virtual>>().pause();
            }
            _ => {}
        }
    });
}

pub(super) fn control_observation(world: &World) -> serde_json::Value {
    let Some(s) = world.get_resource::<ControlFixture>() else {
        return serde_json::Value::Null;
    };
    serde_json::json!({"stage":s.stage,"shed_tick":s.outage_at,"restore_tick":s.restored_at,"phase_rates":s.phase_rates,
        "demand":world.get::<hw_energy::PowerConsumer>(s.fan).map(|p|p.demand),
        "supplied":world.get::<hw_energy::PowerSupplyState>(s.fan)==Some(&hw_energy::PowerSupplyState::Supplied)})
}
