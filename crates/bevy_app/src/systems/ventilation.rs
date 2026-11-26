//! Derived airflow for the current building envelope and completed ventilation equipment.
pub(crate) mod climate;
mod layout;
#[cfg(feature = "profiling")]
pub(crate) mod perf;
mod publication;
#[cfg(feature = "profiling")]
mod replay;
mod services;
mod solver;
#[cfg(feature = "profiling")]
pub(crate) mod transport_probe;
use bevy::prelude::*;
use hw_infra::ventilation::{self, OpeningFlow, Ventilation, VentilationState};
use hw_jobs::Building;
use hw_world::{Room, RoomDetectionState, RoomOverlayTile};
use layout::{Layout, Passage};

#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(crate) struct RoomVentilation(pub Ventilation);

fn rate(amount: f64) -> String {
    if amount > 1e-9 && amount < 0.005 {
        "0.01未満".into()
    } else {
        format!("{amount:.2}")
    }
}
impl RoomVentilation {
    pub(crate) fn state_label(self) -> &'static str {
        match self.0.state() {
            VentilationState::Ventilated => "良好",
            VentilationState::Insufficient => "換気不足",
            VentilationState::NoOutdoorExchange => "外気交換なし",
        }
    }

    pub(crate) fn balance(self) -> String {
        let mut text = format!(
            "有効外気 {} / 必要 {:.2}",
            rate(self.0.supplied()),
            self.0.required()
        );
        if self.0.deficit() > 1e-9 {
            text.push_str(&format!("　不足 {}", rate(self.0.deficit())));
        }
        text
    }

    pub(crate) fn description(self) -> String {
        let mut text = format!("換気　{}\n{}", self.state_label(), self.balance());
        text.push_str(&format!(
            "\n屋外流入 {:.2}　室間流入 {:.2}",
            self.0.outdoor_in, self.0.transfer_in
        ));
        text
    }
}

#[derive(Component, Debug, PartialEq)]
pub(crate) enum VentReading {
    Unconnected,
    Passage {
        kind: &'static str,
        a: &'static str,
        b: &'static str,
        flow: OpeningFlow,
    },
    // Eager formatting is the native A/B control.
    #[cfg(feature = "profiling")]
    Formatted(String),
}

impl VentReading {
    fn from_flow(passage: Passage, flow: OpeningFlow) -> Self {
        Self::Passage {
            kind: passage.kind,
            a: passage.a_side,
            b: passage.b_side,
            flow,
        }
    }
}

impl std::fmt::Display for VentReading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unconnected => f.write_str("通風面が未接続\n室と屋外・別室を隔てる直線壁に設置"),
            Self::Passage { kind, a, b, flow } => {
                if !flow.connected_to_outdoors {
                    return write!(f, "{kind}　外気経路なし\n外壁に風穴を設けてください");
                }
                writeln!(f, "{kind}　外気経路あり")?;
                if flow.net.abs() < 0.005 {
                    write!(f, "双方向交換　各 {:.2}", flow.exchange_each_way)
                } else {
                    let (from, to) = if flow.net > 0.0 { (a, b) } else { (b, a) };
                    write!(f, "通風 {from}→{to} {:.2}", flow.net.abs())
                }
            }
            #[cfg(feature = "profiling")]
            Self::Formatted(text) => f.write_str(text),
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct AirGeometry<'w> {
    detection: Res<'w, RoomDetectionState>,
    epoch: Option<Res<'w, hw_core::WorldEpoch>>,
    #[cfg(feature = "profiling")]
    probe: Option<ResMut<'w, perf::AirPerf>>,
}
pub(crate) fn update_room_ventilation(
    mut commands: Commands,
    geometry: AirGeometry,
    rooms: Query<(Entity, &Room, Option<&RoomVentilation>)>,
    buildings: Query<(Entity, &Building, &Transform)>,
    readings: Query<&VentReading>,
    mut cache: Local<hw_core::EpochLocal<Option<u64>>>,
) {
    #[cfg(feature = "profiling")]
    let mut probe = geometry.probe.map(|p| (p, perf::cpu_time_ns(false)));
    let previous_revision = cache.get_mut(geometry.epoch.as_deref().copied().unwrap_or_default());
    let revision = geometry.detection.geometry_revision;
    if *previous_revision == Some(revision) || !geometry.detection.dirty_tiles.is_empty() {
        #[cfg(feature = "profiling")]
        if let Some((p, start)) = probe.as_mut() {
            p.sampled(*start);
        }
        return;
    }
    *previous_revision = Some(revision);
    let layout = Layout::build(rooms.iter().map(|(e, r, _)| (e, r)), buildings.iter());
    let vents = services::ConnectedVents::from_layout(&layout);
    let (values, flows, _counts) = ventilation::solve_with_counts(&layout.floors, &vents.openings);
    for (&e, value) in layout.rooms.iter().zip(values) {
        let value = RoomVentilation(value);
        if rooms.get(e).is_ok_and(|(_, _, v)| v != Some(&value)) {
            commands.entity(e).insert(value);
        }
    }
    let active_readings = vents
        .entities
        .iter()
        .zip(&flows)
        .map(|(&(entity, passage), &flow)| (entity, VentReading::from_flow(passage, flow)));
    let inactive_readings = layout
        .ports
        .iter()
        .filter(|port| port.passage.is_none())
        .map(|port| (port.entity, VentReading::Unconnected));
    for (entity, next) in active_readings.chain(inactive_readings) {
        if readings.get(entity).ok() != Some(&next) {
            commands.entity(entity).try_insert(next);
        }
    }
    #[cfg(feature = "profiling")]
    if let Some((mut p, start)) = probe {
        p.rooms = layout.rooms.len();
        p.grates = vents.openings.len();
        p.counts = _counts;
        p.rebuilds += 1;
        p.sampled(start);
        p.build_cpu_ns = p.latest_cpu_ns;
        if p.dimensions_requested && !layout.rooms.is_empty() {
            p.transport = Some(transport_probe::TransportProbe::new(
                &layout.floors,
                &vents.openings,
                &flows,
            ));
        }
    }
    commands.insert_resource(services::Services::build(
        layout,
        vents,
        rooms.iter().map(|(e, r, _)| (e, r)),
        buildings.iter(),
    ));
}

#[derive(Component)]
pub(crate) struct PaintedBorder(VentilationState);

type ChangedRoomBorders<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, Children>,
        &'static RoomVentilation,
        Option<&'static PaintedBorder>,
    ),
    Or<(Changed<Children>, Changed<RoomVentilation>)>,
>;

pub(crate) fn color_room_borders(
    mut commands: Commands,
    rooms: ChangedRoomBorders,
    mut lines: Query<&mut Sprite, With<RoomOverlayTile>>,
) {
    for (entity, children, ventilation, painted) in &rooms {
        let state = ventilation.0.state();
        let same = painted.is_some_and(|p| p.0 == state);
        if same && !children.is_changed() {
            continue;
        }
        if !same {
            commands.entity(entity).try_insert(PaintedBorder(state));
        }
        let color = match state {
            VentilationState::Ventilated => Color::srgb(0.23, 0.84, 0.68),
            VentilationState::Insufficient => Color::srgb(0.98, 0.48, 0.26),
            VentilationState::NoOutdoorExchange => Color::srgb(0.74, 0.45, 0.94),
        };
        for child in children.iter() {
            if let Ok(mut sprite) = lines.get_mut(child) {
                sprite.color = color;
            }
        }
    }
}

#[derive(Component)]
pub(crate) struct PipeDisplay;

pub(crate) fn show_pipe_layer(
    mode: Res<State<hw_core::game_state::PlayMode>>,
    selection: Res<crate::app_contexts::BuildContext>,
    mut pipes: Query<(&mut Visibility, Ref<PipeDisplay>)>,
) {
    let show = *mode.get() == hw_core::game_state::PlayMode::BuildingPlace
        && selection.kind
            == Some(hw_jobs::BuildingType::Equipment(
                hw_jobs::Equipment::Conduit,
            ));
    let visibility = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (mut current, new) in &mut pipes {
        if (new.is_added() || mode.is_changed() || selection.is_changed()) && *current != visibility
        {
            *current = visibility;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use hw_core::world::DoorState;
    use hw_infra::ventilation::AirBoundary;
    use hw_jobs::{BuildingType, Door};
    use hw_world::WorldMap;
    use hw_world::{RoomBoundaryLookup, RoomTileLookup, detect_rooms_immediately_system};

    #[test]
    fn small_deficit_remains_visible() {
        let openings = [
            ventilation::Opening {
                a: 0,
                b: AirBoundary::Outdoor { pressure: 1.0 },
                conductance: 1.0,
            },
            ventilation::Opening {
                a: 0,
                b: AirBoundary::Room(1),
                conductance: 1.0,
            },
            ventilation::Opening {
                a: 1,
                b: AirBoundary::Outdoor { pressure: -1.0 },
                conductance: 1.0,
            },
        ];
        let east = RoomVentilation(ventilation::solve(&[10, 10], &openings).0[1]);
        assert_eq!(east.state_label(), "換気不足");
        assert!(east.balance().contains("不足 0.01未満"));
    }

    #[test]
    fn existing_border_changes_color_when_only_ventilation_changes() {
        let mut app = App::new();
        app.add_systems(Update, color_room_borders);
        let mut value = Ventilation::for_floor_tiles(12);
        let room = app.world_mut().spawn(RoomVentilation(value)).id();
        let line = app
            .world_mut()
            .spawn((
                RoomOverlayTile { grid_pos: (3, 4) },
                Sprite::default(),
                ChildOf(room),
            ))
            .id();
        app.update();
        let first = app.world().get::<Sprite>(line).unwrap().color;
        value = ventilation::solve(
            &[12],
            &[ventilation::Opening {
                a: 0,
                b: AirBoundary::Outdoor { pressure: 1.0 },
                conductance: 1.0,
            }],
        )
        .0[0];
        app.world_mut()
            .entity_mut(room)
            .insert(RoomVentilation(value));
        app.update();
        let second = app.world().get::<Sprite>(line).unwrap().color;
        assert_ne!(first, second);
        value = ventilation::solve(
            &[12],
            &[ventilation::Opening {
                a: 0,
                b: AirBoundary::Outdoor { pressure: 1.0 },
                conductance: 4.0,
            }],
        )
        .0[0];
        app.world_mut()
            .entity_mut(room)
            .insert(RoomVentilation(value));
        app.update();
        let third = app.world().get::<Sprite>(line).unwrap().color;
        assert_ne!(second, third);
        assert_ne!(first, third);
    }

    #[test]
    fn only_completed_vents_connect_rooms_and_courtyards() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<RoomTileLookup>()
            .init_resource::<RoomBoundaryLookup>()
            .init_resource::<RoomDetectionState>()
            .add_systems(Update, update_room_ventilation);
        let world = app.world_mut();
        let mut grille = Entity::PLACEHOLDER;
        let mut door = Entity::PLACEHOLDER;
        let mut pocket_wall = Entity::PLACEHOLDER;
        for x in 0..9 {
            for y in 0..5 {
                let kind = match (x, y) {
                    (0 | 2, 2) => BuildingType::Door,
                    (5, 2) => BuildingType::ScreamingVent,
                    (7, 0) => BuildingType::Door,
                    (0 | 2 | 5 | 8, _) | (_, 0 | 4) => BuildingType::Wall,
                    (1, _) => continue, // Sealed, unfloored cavity west of the test door.
                    _ => BuildingType::Floor,
                };
                let grid = (20 + x, 20 + y);
                let e = world
                    .spawn((
                        Building {
                            kind,
                            is_provisional: false,
                        },
                        Transform::from_translation(
                            WorldMap::grid_to_world(grid.0, grid.1).extend(0.0),
                        ),
                    ))
                    .id();
                if kind == BuildingType::Door {
                    world.entity_mut(e).insert(Door {
                        state: DoorState::Closed,
                    });
                }
                world
                    .resource_mut::<WorldMap>()
                    .register_completed_building_footprint(kind, e, [grid]);
                match (x, y) {
                    (2, 2) => door = e,
                    (0, 2) => pocket_wall = e,
                    (5, 2) => grille = e,
                    _ => {}
                }
            }
        }
        let rebuild = |world: &mut World| {
            world
                .resource_mut::<RoomDetectionState>()
                .mark_dirty((20, 22));
            world
                .run_system_once(detect_rooms_immediately_system)
                .unwrap();
        };
        let rates = |world: &World| {
            [(23, 22), (26, 22)].map(|grid| {
                let room = world.resource::<RoomTileLookup>().tile_to_room[&grid];
                world.get::<RoomVentilation>(room).unwrap().0.supplied()
            })
        };
        rebuild(world);
        app.update();
        assert_eq!(rates(app.world()), [0.0, 0.0]);
        for state in [DoorState::Open, DoorState::Locked, DoorState::Closed] {
            for e in [door, pocket_wall] {
                app.world_mut().get_mut::<Door>(e).unwrap().state = state;
            }
            app.update();
            assert_eq!(rates(app.world()), [0.0, 0.0]);
            // A fresh calculation (including after a load) must also ignore open doors.
            rebuild(app.world_mut());
            app.update();
            assert_eq!(rates(app.world()), [0.0, 0.0]);
            assert!(
                app.world()
                    .get::<VentReading>(grille)
                    .unwrap()
                    .to_string()
                    .contains("外気経路なし")
            );
        }
        // Keep an ordinary door for room detection while connecting the courtyard with grates.
        for (x, original) in [(2, door), (0, pocket_wall)] {
            let w = app.world_mut();
            for y in [2, 3] {
                let grid = (20 + x, 20 + y);
                let e = if y == 2 {
                    original
                } else {
                    w.resource::<WorldMap>().building_entity(grid).unwrap()
                };
                let kind = if y == 2 {
                    BuildingType::ScreamingVent
                } else {
                    BuildingType::Door
                };
                w.entity_mut(e).insert(Building {
                    kind,
                    is_provisional: false,
                });
                if kind == BuildingType::Door {
                    w.entity_mut(e).insert(Door {
                        state: DoorState::Closed,
                    });
                } else {
                    w.entity_mut(e).remove::<Door>();
                }
            }
            rebuild(w);
            app.update();
            assert_eq!(rates(app.world())[1] > 0.0, x == 0);
        }
    }

    #[test]
    fn shared_vent_tracks_room_topology_and_source_removal() {
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        world.init_resource::<RoomTileLookup>();
        world.init_resource::<RoomBoundaryLookup>();
        world.init_resource::<RoomDetectionState>();
        let mut vent = None;
        let mut outer_wall = None;
        for x in 0..9 {
            for y in 0..5 {
                let kind = match (x, y) {
                    (4, 2) => BuildingType::ScreamingVent,
                    (2 | 6, 0) => BuildingType::Door,
                    (0 | 4 | 8, _) | (_, 0 | 4) => BuildingType::Wall,
                    _ => BuildingType::Floor,
                };
                let grid = (20 + x, 20 + y);
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
                world
                    .resource_mut::<WorldMap>()
                    .register_completed_building_footprint(kind, entity, [grid]);
                if (x, y) == (4, 2) {
                    vent = Some(entity);
                }
                if (x, y) == (8, 2) {
                    outer_wall = Some(entity);
                }
            }
        }
        fn inspect(world: &mut World) -> Vec<String> {
            world
                .resource_mut::<RoomDetectionState>()
                .mark_dirty((24, 22));
            world
                .run_system_once(detect_rooms_immediately_system)
                .unwrap();
            world.run_system_once(update_room_ventilation).unwrap();
            world
                .query::<&RoomVentilation>()
                .iter(world)
                .map(|v| v.description())
                .collect()
        }
        let shared = inspect(&mut world);
        assert_eq!(shared.len(), 2);
        assert!(
            shared
                .iter()
                .all(|text| text.contains("有効外気 0.00 / 必要 0.45"))
        );
        world.despawn(outer_wall.unwrap());
        let external = inspect(&mut world);
        assert_eq!(external.len(), 1);
        assert!(external[0].contains("有効外気 0.20"));
        world.get_mut::<Building>(vent.unwrap()).unwrap().kind = BuildingType::Wall;
        assert!(inspect(&mut world)[0].contains("外気交換なし"));
    }
}
