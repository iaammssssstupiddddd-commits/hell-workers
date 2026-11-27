mod offer;
use bevy::prelude::*;
use hw_core::GameTime;
use hw_jobs::{
    Blueprint, Building, BuildingType,
    reception::{ReceivingPanel, ReceptionContract, ReceptionYard},
};
use hw_ui::notifications::{NotificationRetention, NotificationSeverity, UserFacingNotification};
use hw_world::{WorldMap, Yard};
pub(crate) use offer::{OfferSite, PublishedOffer, accept};

fn service_available(world: &mut World, owner: Entity, contract: &ReceptionContract) -> bool {
    let point = contract.terms.point.into();
    let pos = WorldMap::grid_to_world(contract.terms.point.x, contract.terms.point.y);
    world
        .get::<Building>(owner)
        .is_some_and(|b| b.kind == BuildingType::TemporaryReceivingPanel && !b.is_provisional)
        && world
            .get::<Transform>(owner)
            .is_some_and(|t| WorldMap::world_to_grid(t.translation.truncate()) == point)
        && world.resource::<WorldMap>().building_entity(point) == Some(owner)
        && world
            .query_filtered::<&Yard, With<ReceptionYard>>()
            .single(world)
            .is_ok_and(|y| y.contains(pos))
}

pub(crate) fn update_supply(world: &mut World) {
    let Some((entity, record)) = world
        .query::<(Entity, &ReceptionContract)>()
        .single(world)
        .ok()
        .map(|(e, c)| (e, *c))
    else {
        return;
    };
    let Some(owner) = world
        .resource::<WorldMap>()
        .building_entity(record.terms.point.into())
    else {
        return;
    };
    let Some(mut panel) = world.get::<ReceivingPanel>(owner).copied() else {
        return;
    };
    if !panel.reported || !service_available(world, owner, &record) {
        if panel.live {
            world.get_mut::<ReceivingPanel>(owner).unwrap().live = false;
        }
        return;
    }
    let now = world.resource::<GameTime>().seconds;
    if panel.live || now < record.terms.scheduled_at {
        return;
    }
    let first = world
        .get_mut::<ReceptionContract>(entity)
        .unwrap()
        .try_energize(&mut panel, now);
    world.entity_mut(owner).insert(panel);
    if first {
        world.write_message(UserFacingNotification::new(
            "reception-first",
            NotificationSeverity::Success,
            "指定盤で初回受電",
            "管理 → 契約で受電日時と工程実績を確認できます。",
            NotificationRetention::Important,
        ));
    }
}

pub(crate) fn report(world: &mut World, owner: Entity, designation: u32, epoch: u64) {
    if world.resource::<hw_core::WorldEpoch>().get() != epoch {
        return;
    }
    let Ok(record) = world.query::<&ReceptionContract>().single(world).copied() else {
        return;
    };
    if record.designation != designation || !service_available(world, owner, &record) {
        return;
    }
    if let Some(mut panel) = world.get_mut::<ReceivingPanel>(owner)
        && panel.designation == designation
    {
        panel.reported = true;
        update_supply(world);
    }
}

pub(crate) fn validate_candidate(world: &World) -> Result<(), String> {
    let mut contracts = world
        .iter_entities()
        .filter_map(|e| e.get::<ReceptionContract>().map(|c| (e, c)));
    let mut panels = world.iter_entities().filter(|e| {
        e.contains::<ReceivingPanel>()
            || e.get::<Building>()
                .is_some_and(|b| b.kind == BuildingType::TemporaryReceivingPanel)
            || e.get::<Blueprint>()
                .is_some_and(|b| b.kind == BuildingType::TemporaryReceivingPanel)
    });
    let mut yards = world
        .iter_entities()
        .filter(|e| e.contains::<ReceptionYard>());
    let yard = yards.next();
    if yards.next().is_some() || yard.is_some_and(|e| !e.contains::<Yard>()) {
        return Err("受電契約の工区が不正です".into());
    }
    let panel = panels.next();
    let Some((root, c)) = contracts.next() else {
        return if panel.is_some() || yard.is_some() {
            Err("契約のない受電盤".into())
        } else {
            Ok(())
        };
    };
    let clock = world
        .get_resource::<GameTime>()
        .ok_or("受電契約の時計がありません")?
        .seconds;
    let t = c.terms;
    if contracts.next().is_some()
        || root.archetype().components().len() != 1
        || panels.next().is_some()
        || ![t.point, t.intake, t.basin, t.floor_start].iter().all(|p| {
            p.x >= 0
                && p.y >= 0
                && p.x < hw_core::constants::MAP_WIDTH
                && p.y < hw_core::constants::MAP_HEIGHT
        })
        || t.floor_start.x >= hw_core::constants::MAP_WIDTH - 4
        || t.floor_start.y >= hw_core::constants::MAP_HEIGHT - 1
        || t.point != t.floor_start + IVec2::new(2, 1)
        || ![
            t.setup_fee,
            c.accepted_at,
            t.scheduled_at,
            t.deadline,
            t.handover_deadline,
            t.capacity_w,
            clock,
        ]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.)
        || c.accepted_at > clock
        || c.accepted_at > t.deadline
        || t.scheduled_at > t.deadline
        || t.handover_deadline < t.deadline
        || t.capacity_w <= 0.
        || c.first_received_at.is_some_and(|at| {
            !at.is_finite()
                || at < c.accepted_at
                || at < t.scheduled_at
                || at > clock
                || c.designation == 0
        })
    {
        return Err("受電契約の条項または実績が不正です".into());
    }
    if let Some(e) = panel {
        let tag = e
            .get::<ReceivingPanel>()
            .ok_or("受電盤の指名がありません")?;
        let valid = match (e.get::<Blueprint>(), e.get::<Building>()) {
            (Some(b), None) => {
                b.kind == BuildingType::TemporaryReceivingPanel
                    && b.occupied_grids.as_slice() == [t.point.into()]
                    && !tag.live
                    && !tag.reported
            }
            (None, Some(b)) => {
                b.kind == BuildingType::TemporaryReceivingPanel
                    && !b.is_provisional
                    && (!tag.live || (tag.reported && c.first_received_at.is_some()))
            }
            _ => false,
        };
        let map = world
            .get_resource::<WorldMap>()
            .ok_or("受電盤の地図がありません")?;
        if map.building_entity(t.point.into()) != Some(e.id())
            || map
                .buildings
                .values()
                .filter(|&&owner| owner == e.id())
                .count()
                != 1
            || !valid
            || tag.designation == 0
            || tag.designation != c.designation
            || !e.get::<Transform>().is_some_and(|p| {
                p.translation.is_finite()
                    && WorldMap::world_to_grid(p.translation.truncate()) == t.point.into()
            })
        {
            return Err("指定盤と契約図の対応が不正です".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::reception::ReceptionTerms;

    fn site(now: f32) -> (World, Entity, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<hw_core::WorldEpoch>();
        world.init_resource::<Messages<UserFacingNotification>>();
        world.init_resource::<WorldMap>();
        world.insert_resource(GameTime {
            seconds: now,
            ..default()
        });
        let floor = IVec2::splat(32);
        let point = floor + IVec2::new(2, 1);
        let terms = ReceptionTerms::riverfront(point, floor, IVec2::splat(8), IVec2::splat(12), 0.);
        let mut c = ReceptionContract::new(terms, 0.);
        c.designation = 1;
        let contract = world.spawn(c).id();
        let pos = WorldMap::grid_to_world(point.x, point.y);
        let yard = world
            .spawn((Yard { min: pos, max: pos }, ReceptionYard))
            .id();
        let panel = world
            .spawn((
                Building {
                    kind: BuildingType::TemporaryReceivingPanel,
                    is_provisional: false,
                },
                Transform::from_translation(pos.extend(0.)),
                ReceivingPanel {
                    designation: 1,
                    reported: false,
                    live: false,
                },
            ))
            .id();
        world
            .resource_mut::<WorldMap>()
            .set_building(point.into(), panel);
        (world, contract, panel, yard)
    }

    #[test]
    fn reception_checks_identity_and_records_both_arrival_orders_once() {
        for (report_time, supply_time) in [(60., 63000.), (64800., 64800.), (64860., 64860.)] {
            let (mut w, c, p, y) = site(report_time);
            update_supply(&mut w);
            report(&mut w, p, 2, 0);
            report(&mut w, p, 1, 1);
            assert!(!w.get::<ReceivingPanel>(p).unwrap().reported);
            report(&mut w, p, 1, 0);
            assert_eq!(
                w.get::<ReceivingPanel>(p).unwrap().live,
                report_time >= 63000.
            );
            w.resource_mut::<GameTime>().seconds = supply_time;
            update_supply(&mut w);
            report(&mut w, p, 1, 0);
            assert_eq!(
                w.get::<ReceptionContract>(c).unwrap().first_received_at,
                Some(supply_time)
            );
            assert_eq!(w.resource::<Messages<UserFacingNotification>>().len(), 1);
            validate_candidate(&w).unwrap();
            let foreign = w.get::<Yard>(y).unwrap().clone();
            w.despawn(y);
            w.spawn(foreign);
            update_supply(&mut w);
            assert!(!w.get::<ReceivingPanel>(p).unwrap().live);
            assert_eq!(
                w.get::<ReceptionContract>(c).unwrap().first_received_at,
                Some(supply_time)
            );
        }
    }
    #[test]
    fn reception_rebuilt_panel_requires_new_report_and_retains_first_receipt() {
        let (mut w, c, old, _) = site(65000.);
        report(&mut w, old, 1, 0);
        let pos = *w.get::<Transform>(old).unwrap();
        let building = Building {
            kind: BuildingType::TemporaryReceivingPanel,
            is_provisional: false,
        };
        w.despawn(old);
        let replacement = w
            .spawn((
                building,
                pos,
                ReceivingPanel {
                    designation: 2,
                    reported: false,
                    live: false,
                },
            ))
            .id();
        w.get_mut::<ReceptionContract>(c).unwrap().designation = 2;
        w.resource_mut::<WorldMap>().set_building(
            WorldMap::world_to_grid(pos.translation.truncate()),
            replacement,
        );
        w.resource_mut::<GameTime>().seconds += 3600.;
        report(&mut w, old, 1, 0);
        report(&mut w, replacement, 1, 0);
        update_supply(&mut w);
        assert!(!w.get::<ReceivingPanel>(replacement).unwrap().live);
        report(&mut w, replacement, 2, 0);
        assert!(w.get::<ReceivingPanel>(replacement).unwrap().live);
        assert_eq!(
            w.get::<ReceptionContract>(c).unwrap().first_received_at,
            Some(65000.)
        );
        assert_eq!(w.resource::<Messages<UserFacingNotification>>().len(), 1);
    }
    #[test]
    fn reception_rejects_invalid_map_and_overflow_in_candidates() {
        let (mut w, c, p, _) = site(120.);
        let terms = w.get::<ReceptionContract>(c).unwrap().terms;
        w.resource_mut::<WorldMap>()
            .clear_building(terms.point.into());
        assert!(validate_candidate(&w).is_err());
        w.resource_mut::<WorldMap>()
            .set_building(terms.point.into(), p);
        for x in [i32::MIN, i32::MAX] {
            w.get_mut::<ReceptionContract>(c)
                .unwrap()
                .terms
                .floor_start
                .x = x;
            assert!(validate_candidate(&w).is_err());
        }
    }
}
