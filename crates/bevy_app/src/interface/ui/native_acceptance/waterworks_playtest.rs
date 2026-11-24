//! Select two legal camera targets once. All contract and world changes use normal player input.
use super::*;
use crate::interface::selection::placement_geometry::{
    bucket_storage_geometry, live_building_geometry,
};
use crate::systems::waterworks::{ContractDistrict, ContractOutcome, WaterworksContract};
use crate::world::map::GeneratedWorldLayoutResource;
use hw_core::{GameTime, relationships::StoredIn, soul::DreamPool};
use hw_jobs::{Blueprint, BuildingType};
use hw_logistics::{ResourceItem, ResourceType};
use hw_ui::camera::MainCamera;
use hw_world::{Site, WorldMap, Yard};

#[derive(Resource)]
struct Sites([(i32, i32); 2]);

#[derive(Resource, Default)]
struct Witness {
    construction: Option<(u64, Entity, Entity)>,
    before_claim: Option<(u64, f32)>,
    paid: Option<(u64, f32)>,
}

pub(super) fn register(app: &mut App) {
    use crate::interface::ui::interaction::handle_ui_intent;
    use crate::systems::GameSystemSet;
    app.init_resource::<Witness>()
        .init_resource::<hw_soul_ai::water_observations::TankPourReceipts>()
        .add_observer(
            |completion: On<hw_jobs::events::BuildingCompletedEvent>,

             epoch: Res<hw_core::WorldEpoch>,
             mut witness: ResMut<Witness>| {
                if completion.kind == BuildingType::Tank {
                    witness.construction = Some((
                        epoch.get(),
                        completion.blueprint_entity,
                        completion.building_entity,
                    ));
                }
            },
        )
        .add_systems(
            Update,
            before_claim
                .before(handle_ui_intent)
                .in_set(GameSystemSet::Interface),
        )
        .add_systems(
            Update,
            after_claim
                .after(handle_ui_intent)
                .in_set(GameSystemSet::Interface),
        );
}

fn before_claim(
    contracts: Query<&WaterworksContract>,
    pool: Res<DreamPool>,
    epoch: Res<hw_core::WorldEpoch>,
    mut witness: ResMut<Witness>,
) {
    witness.before_claim = contracts
        .single()
        .ok()
        .filter(|c| matches!(c.outcome, ContractOutcome::Inspected { .. }))
        .map(|_| (epoch.get(), pool.points));
}

fn after_claim(
    contracts: Query<&WaterworksContract>,
    pool: Res<DreamPool>,
    epoch: Res<hw_core::WorldEpoch>,
    mut witness: ResMut<Witness>,
) {
    if let Some((e, before)) = witness.before_claim
        && e == epoch.get()
        && contracts
            .single()
            .is_ok_and(|c| matches!(c.outcome, ContractOutcome::Paid { .. }))
    {
        witness.paid = Some((e, pool.points - before));
    }
}

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("first-waterworks")
}

pub(super) fn prepare(world: &mut World) {
    let district: ContractDistrict = world
        .resource::<GeneratedWorldLayoutResource>()
        .layout
        .anchors
        .yard
        .into();
    let sites: Vec<_> = world.query::<&Site>().iter(world).cloned().collect();
    let yards: Vec<_> = world.query::<&Yard>().iter(world).cloned().collect();
    let map = world.resource::<WorldMap>();
    let anchors = [16, 6].map(|offset| {
        let desired = district.min + IVec2::new(7, offset);
        (district.min.x..=district.max.x)
            .flat_map(|x| (district.min.y..=district.max.y).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let geometry = live_building_geometry(map, BuildingType::Tank, (x, y));
                let companion = bucket_storage_geometry((x + 2, y));
                geometry
                    .occupied_grids
                    .iter()
                    .chain(&companion.occupied_grids)
                    .all(|&cell| district.contains(cell))
                    && super::m5_planning::validate(map, &sites, &yards, BuildingType::Tank, (x, y))
                        .can_place
                    && super::m5_planning::companion_legal(map, (x, y))
            })
            .min_by_key(|&(x, y)| (x - desired.x).abs() + (y - desired.y).abs())
            .expect("initial Yard must support tank with bucket storage")
    });
    assert!(
        anchors[0].1 > anchors[1].1,
        "two distinct sites south of the river required"
    );
    let center = (WorldMap::grid_to_world(anchors[0].0, anchors[0].1)
        + WorldMap::grid_to_world(anchors[1].0, anchors[1].1))
        * 0.5;
    for mut camera in world
        .query_filtered::<&mut Transform, With<MainCamera>>()
        .iter_mut(world)
    {
        camera.translation.x = center.x;
        camera.translation.y = center.y;
    }
    world.insert_resource(Sites(anchors));
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(anchors) = world.get_resource::<Sites>().map(|s| s.0) else {
        return Value::Null;
    };
    let Ok((camera, transform)) = world
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>()
        .single(world)
    else {
        return Value::Null;
    };
    let points = anchors.map(|(x, y)| {
        let project = |x| {
            camera
                .world_to_viewport(transform, WorldMap::grid_to_world(x, y).extend(0.))
                .ok()
                .map(|p| p.to_array())
        };
        json!({"grid":[x,y],"body":project(x),"companion":project(x+2)})
    });
    let contract = world
        .query::<&WaterworksContract>()
        .single(world)
        .ok()
        .copied();
    let blueprint = world
        .query::<(Entity, &Blueprint)>()
        .iter(world)
        .find(|(_, bp)| {
            bp.kind == BuildingType::Tank && anchors.iter().any(|p| bp.occupied_grids.contains(p))
        })
        .map(|(id, _)| id);
    let epoch = world.resource::<hw_core::WorldEpoch>().get();
    let witness = world.resource::<Witness>();
    let built = witness.construction.filter(|(e, _, _)| *e == epoch);
    let credited = witness
        .paid
        .filter(|(e, _)| *e == epoch)
        .map(|(_, amount)| amount);
    let tank = built.map(|(_, _, tank)| tank);
    let site = blueprint
        .or(tank)
        .and_then(|e| world.get::<Transform>(e))
        .map(|t| {
            crate::interface::selection::placement_geometry::existing_movable_building_anchor(
                BuildingType::Tank,
                t.translation.truncate(),
            )
        });
    let receipts = world.resource::<hw_soul_ai::water_observations::TankPourReceipts>();
    let pour = receipts
        .rows
        .iter()
        .find(|r| r.world_epoch == epoch && Some(r.tank) == tank);
    let river_delivery = receipts.failure.is_none()
        && pour.is_some_and(|r| {
            r.outputs.len() == 5
                && receipts.river_fills.iter().any(|f| {
                    f.world_epoch == epoch
                        && f.tank == r.tank
                        && f.worker == r.worker
                        && f.assignment == r.assignment
                        && f.bucket == r.bucket
                })
        });
    let water = world
        .query::<(&ResourceItem, &StoredIn)>()
        .iter(world)
        .filter(|(item, owner)| item.0 == ResourceType::Water && Some(owner.0) == tank)
        .count();
    let (stage, at, reward) = match contract.map(|c| c.outcome) {
        None => ("offer", None, None),
        Some(ContractOutcome::Active) => (
            if blueprint.is_some() {
                "blueprint"
            } else if tank.is_some() {
                "tank"
            } else {
                "site"
            },
            None,
            None,
        ),
        Some(ContractOutcome::Inspected { at, reward }) => ("inspected", Some(at), Some(reward)),
        Some(ContractOutcome::Paid { at, reward }) => ("paid", Some(at), Some(reward)),
    };
    let balance = world.resource::<DreamPool>().points;
    let seconds = world.resource::<GameTime>().seconds;
    let overlay = world
        .resource::<crate::interface::ui::waterworks::ContractOverlay>()
        .0;
    json!({"sites":points,"stage":stage,"accepted":contract.map(|c|c.accepted),"deadline":contract.map(|c|c.deadline),"designation":contract.map(|c|c.designation),"inspected":at,"reward":reward,"water":water,"balance":balance,"seconds":seconds,"overlay":overlay,
        "site":site, "target":blueprint.map(Entity::to_bits), "completed_blueprint":built.map(|(_,bp,_)|bp.to_bits()), "river_delivery":river_delivery, "credited":credited,
        "pending":world.resource::<crate::app_contexts::CompanionPlacementState>().0.is_some()})
}
