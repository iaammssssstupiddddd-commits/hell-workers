//! Publish only the air components whose room dilution or vent flows can have changed.
use super::{RoomVentilation, VentReading, services::Services, solver::Solver};
use bevy::prelude::*;

pub(super) fn publish(
    solver: &Solver,
    svc: &Services,
    commands: &mut Commands,
    full: bool,
) -> (usize, usize, usize) {
    let (rates, flows) = solver.readings();
    let rooms = &svc.layout.rooms;
    let mut room_count = 0;
    let mut vent_count = 0;
    let room = |z: usize, commands: &mut Commands| {
        if let Some(&entity) = rooms.get(z) {
            commands
                .entity(entity)
                .try_insert(RoomVentilation(rates[z]));
            true
        } else {
            false // An enclosed courtyard or a junction inside a thick wall.
        }
    };
    let vent = |i: usize, commands: &mut Commands| {
        let (entity, passage) = svc.vents.entities[i];
        let reading = VentReading::from_flow(passage, flows[i]);
        #[cfg(feature = "profiling")]
        let reading = if full {
            VentReading::Formatted(reading.to_string())
        } else {
            reading
        };
        commands.entity(entity).try_insert(reading);
    };
    match solver {
        Solver::Cached(network) if !full => {
            let groups = network.changed_air_groups();
            let group_count = groups.len();
            for (zones, ports) in groups {
                for &z in zones {
                    room_count += usize::from(room(z, commands));
                }
                for i in ports {
                    vent(i, commands);
                    vent_count += 1;
                }
            }
            (room_count, vent_count, group_count)
        }
        _ => {
            for z in 0..rooms.len() {
                room(z, commands);
            }
            for i in 0..flows.len() {
                vent(i, commands);
            }
            #[cfg(feature = "profiling")]
            let groups = match solver {
                Solver::Cached(n) => n.changed_air_groups().len(),
                _ => 0,
            };
            #[cfg(not(feature = "profiling"))]
            let groups = 0;
            (rooms.len(), flows.len(), groups)
        }
    }
}
