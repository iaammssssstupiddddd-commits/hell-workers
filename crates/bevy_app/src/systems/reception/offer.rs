use bevy::{ecs::system::SystemParam, prelude::*};
use hw_core::{GameTime, soul::DreamPool};
use hw_energy::{ConsumesFrom, PowerSupplyState, YardPowerGrid};
use hw_jobs::{construction::FloorTileBlueprint, reception::ReceptionTerms};
use hw_world::{GridRect, PairedSite, PairedYard, Site, WorldMap, Yard};
use std::collections::{HashSet, VecDeque};

use crate::{systems::ventilation::Services, world::map::GeneratedWorldLayoutResource};

type Tender = Result<(Entity, ReceptionTerms), &'static str>;

#[derive(Resource, Default)]
pub(crate) struct PublishedOffer(pub Option<Result<ReceptionTerms, &'static str>>);

#[derive(SystemParam)]
pub(crate) struct OfferSite<'w, 's> {
    layout: Res<'w, GeneratedWorldLayoutResource>,
    map: Res<'w, WorldMap>,
    clock: Res<'w, GameTime>,
    yards: Query<'w, 's, (Entity, &'static Yard, &'static PairedSite)>,
    sites: Query<'w, 's, (&'static Site, &'static PairedYard)>,
    intakes: Query<'w, 's, (&'static PowerSupplyState, &'static ConsumesFrom)>,
    grids: Query<'w, 's, &'static YardPowerGrid>,
    services: Option<Res<'w, Services>>,
    floors: Query<'w, 's, &'static FloorTileBlueprint>,
}

impl OfferSite<'_, '_> {
    pub fn terms(&self) -> Result<ReceptionTerms, &'static str> {
        self.survey().map(|(_, terms)| terms)
    }

    fn survey(&self) -> Tender {
        let anchors = &self.layout.layout.anchors;
        let yard_at = WorldMap::grid_to_world(anchors.yard.min_x, anchors.yard.min_y);
        let (owner, yard, pair) = self
            .yards
            .iter()
            .find(|(_, y, _)| y.contains(yard_at))
            .ok_or("初期資材置場の工区が必要です")?;
        let (site, _) = self
            .sites
            .get(pair.0)
            .ok()
            .filter(|(_, p)| p.0 == owner)
            .ok_or("対応する施工区が必要です")?;
        let water = self
            .services
            .as_ref()
            .and_then(|s| {
                s.supplied_basin(
                    |e, (x, y)| {
                        // The center of a 3 × 2 intake rounds to the upper middle cell.
                        self.map.building_entity((x, y)) == Some(e)
                            && [y - 2, y + 1].into_iter().any(|bank| {
                                (x - 1..=x + 1).all(|col| self.map.is_river_tile(col, bank))
                            })
                            && self.intakes.get(e).is_ok_and(|(state, grid)| {
                                *state == PowerSupplyState::Supplied
                                    && self.grids.get(grid.0).is_ok_and(|g| g.0 == owner)
                            })
                    },
                    |e, (x, y)| {
                        let pos = WorldMap::grid_to_world(x, y);
                        self.map.building_entity((x, y)) == Some(e)
                            && (site.contains(pos) || yard.contains(pos))
                    },
                )
            })
            .ok_or("河岸の取水門に給電し、導水管で屋内の浄化盤につないでください")?;
        let pending = self.floors.iter().map(|f| f.grid_pos).collect();
        let (point, floor) = work_area(
            &self.map,
            anchors.yard,
            anchors.wheelbarrow_parking,
            yard,
            &pending,
        )
        .ok_or("資材置場に、猫車が通れる受電盤と荷受け場の用地を確保してください")?;
        Ok((
            owner,
            ReceptionTerms::riverfront(
                point,
                floor,
                water.0.into(),
                water.1.into(),
                self.clock.seconds,
            ),
        ))
    }
}

// Restrict the access survey to the tendered yard. Keep a clear approach along the five floor cells.
fn work_area(
    map: &WorldMap,
    bounds: GridRect,
    parking: GridRect,
    yard: &Yard,
    pending: &HashSet<(i32, i32)>,
) -> Option<(IVec2, IVec2)> {
    let mut reachable = HashSet::new();
    let mut todo = VecDeque::new();
    let enqueue = |p: (i32, i32), known: &mut HashSet<_>, queue: &mut VecDeque<_>| {
        if bounds.contains(p)
            && yard.contains(WorldMap::grid_to_world(p.0, p.1))
            && map.is_walkable(p.0, p.1)
            && known.insert(p)
        {
            queue.push_back(p);
        }
    };
    for x in parking.min_x..=parking.max_x {
        for y in [parking.min_y - 1, parking.max_y + 1] {
            enqueue((x, y), &mut reachable, &mut todo);
        }
    }
    for y in parking.min_y..=parking.max_y {
        for x in [parking.min_x - 1, parking.max_x + 1] {
            enqueue((x, y), &mut reachable, &mut todo);
        }
    }
    while let Some((x, y)) = todo.pop_front() {
        for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            enqueue(p, &mut reachable, &mut todo);
        }
    }
    let free = |p| {
        reachable.contains(&p)
            && !map.has_stockpile(p)
            && map.building_entity(p).is_none()
            && !map.floors.contains_key(&p)
            && !pending.contains(&p)
    };
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..=bounds.max_x - 4 {
            if (0..5).all(|i| free((x + i, y)) && reachable.contains(&(x + i, y + 1)))
                && [(x, y + 1), (x + 2, y + 1), (x + 4, y + 1)]
                    .into_iter()
                    .all(free)
            {
                return Some((IVec2::new(x + 2, y + 1), IVec2::new(x, y)));
            }
        }
    }
    None
}

pub(crate) fn accept(world: &mut World) {
    let displayed = world
        .get_resource::<PublishedOffer>()
        .and_then(|o| o.0.and_then(Result::ok));
    let Some(displayed) = displayed else { return };
    if world
        .query::<&hw_jobs::reception::ReceptionContract>()
        .iter(world)
        .next()
        .is_some()
    {
        return;
    }
    let surveyed = world
        .run_system_cached(current_terms)
        .expect("offer survey");
    let (yard, current) = match surveyed {
        Ok(current) => current,
        Err(reason) => {
            world.resource_mut::<PublishedOffer>().0 = Some(Err(reason));
            reject(world, reason);
            return;
        }
    };
    if displayed != current {
        world.resource_mut::<PublishedOffer>().0 = Some(Ok(current));
        reject(
            world,
            "現場条件または期限が変わりました。更新した契約図を確認してください。",
        );
        return;
    }
    let now = world.resource::<GameTime>().seconds;
    let mut pool = world.resource_mut::<DreamPool>();
    if pool.points < current.setup_fee {
        reject(world, "共有Dreamが不足しています。");
        return;
    }
    pool.points -= current.setup_fee;
    world
        .entity_mut(yard)
        .insert(hw_jobs::reception::ReceptionYard);
    world.spawn(hw_jobs::reception::ReceptionContract::new(current, now));
}
fn reject(world: &mut World, reason: &'static str) {
    use hw_ui::notifications::{
        NotificationRetention, NotificationSeverity, UserFacingNotification,
    };
    world.write_message(UserFacingNotification::new(
        "reception-offer",
        NotificationSeverity::Warning,
        "受注条件を確認してください",
        reason,
        NotificationRetention::ToastOnly,
    ));
}
fn current_terms(site: OfferSite) -> Tender {
    site.survey()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use hw_jobs::{Building, BuildingType, Equipment, reception::ReceptionContract};
    use hw_world::{RoomBoundaryLookup, RoomDetectionState, RoomTileLookup};
    #[test]
    fn reception_offer_surveys_existing_water_and_freezes_one_paid_award() {
        let mut w = World::new();
        w.init_resource::<WorldMap>();
        w.init_resource::<GameTime>();
        w.init_resource::<DreamPool>();
        w.init_resource::<PublishedOffer>();
        w.init_resource::<Messages<hw_ui::notifications::UserFacingNotification>>();
        let layout = hw_world::GeneratedWorldLayout::stub(42);
        let a = layout.anchors.clone();
        w.insert_resource(GeneratedWorldLayoutResource {
            master_seed: 42,
            layout,
        });
        let pos = WorldMap::grid_to_world;
        let yard = w
            .spawn(Yard {
                min: pos(a.yard.min_x, a.yard.min_y),
                max: pos(a.yard.max_x, a.yard.max_y),
            })
            .id();
        let site = w
            .spawn((
                Site {
                    min: pos(a.site.min_x, a.site.min_y),
                    max: pos(a.site.max_x, a.site.max_y),
                },
                PairedYard(yard),
            ))
            .id();
        w.entity_mut(yard).insert(PairedSite(site));
        assert!(w.run_system_cached(current_terms).unwrap().is_err());
        let (x, y) = (a.site.min_x + 4, a.site.min_y + 4);
        let spawn = |w: &mut World, kind, grid: (i32, i32)| {
            let e = w
                .spawn((
                    Building {
                        kind,
                        is_provisional: false,
                    },
                    Transform::from_translation(pos(grid.0, grid.1).extend(0.)),
                ))
                .id();
            if kind == BuildingType::Floor {
                w.resource_mut::<WorldMap>().set_floor(grid, e);
            } else {
                w.resource_mut::<WorldMap>()
                    .register_completed_building_footprint(kind, e, [grid]);
            }
            e
        };
        for dx in 0..5 {
            for dy in 0..5 {
                let kind = match (dx, dy) {
                    (2, 0) => BuildingType::Door,
                    (0 | 4, _) | (_, 0 | 4) => BuildingType::Wall,
                    _ => BuildingType::Floor,
                };
                spawn(&mut w, kind, (x + dx, y + dy));
            }
        }
        let intake = spawn(
            &mut w,
            BuildingType::Equipment(Equipment::Intake),
            (x + 2, y - 2),
        );
        for col in x + 1..=x + 3 {
            let mut map = w.resource_mut::<WorldMap>();
            let index = map.pos_to_idx(col, y - 4).unwrap();
            map.set_terrain_at_idx(index, hw_world::terrain::TerrainType::River);
        }
        spawn(
            &mut w,
            BuildingType::Equipment(Equipment::Basin),
            (x + 2, y + 2),
        );
        for row in y - 1..=y + 1 {
            spawn(
                &mut w,
                BuildingType::Equipment(Equipment::Conduit),
                (x + 2, row),
            );
        }
        w.init_resource::<RoomDetectionState>();
        w.init_resource::<RoomTileLookup>();
        w.init_resource::<RoomBoundaryLookup>();
        w.resource_mut::<RoomDetectionState>().mark_dirty((x, y));
        w.run_system_once(hw_world::detect_rooms_immediately_system)
            .unwrap();
        w.run_system_once(crate::systems::ventilation::update_room_ventilation)
            .unwrap();
        assert!(w.run_system_cached(current_terms).unwrap().is_err());
        let grid = w.spawn(YardPowerGrid(yard)).id();
        w.entity_mut(intake)
            .insert((PowerSupplyState::Supplied, ConsumesFrom(grid)));
        let (_, first) = w.run_system_cached(current_terms).unwrap().unwrap();
        w.resource_mut::<PublishedOffer>().0 = Some(Ok(first));
        w.resource_mut::<GameTime>().seconds = 18. * 3600.;
        accept(&mut w);
        assert!(w.query::<&ReceptionContract>().iter(&w).next().is_none());
        let revised = w.resource::<PublishedOffer>().0.unwrap().unwrap();
        assert_eq!(revised.deadline - first.deadline, 86400.);
        w.resource_mut::<DreamPool>().points = 0.;
        accept(&mut w);
        assert!(w.query::<&ReceptionContract>().iter(&w).next().is_none());
        w.resource_mut::<DreamPool>().points = 10.;
        accept(&mut w);
        accept(&mut w);
        assert_eq!(
            w.query::<&ReceptionContract>().single(&w).unwrap().terms,
            revised
        );
        assert_eq!(w.resource::<DreamPool>().points, 10. - revised.setup_fee);
        assert!(w.get::<hw_jobs::reception::ReceptionYard>(yard).is_some());
    }
}
