//! Profiling-only state fixture. Seeds domain inputs, never presentation state.
//! Construction and refinement still run through the ordinary game systems.
use super::super::{BuildingAssetKind, BuildingAssetPool};
use super::BuildingArtSession;
use crate::interface::selection::{
    building_place::try_place_bucket_storage_companion, placement_geometry::building_geometry,
};
use crate::world::map::WorldMapRef;
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::{StoredIn, WorkingOn};
use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
use hw_core::visual_mirror::StockpileVisualState;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, Blueprint, Building, BuildingType, RefineData, RefinePhase,
    WorkType,
};
use hw_logistics::{ResourceItem, ResourceType};
use hw_ui::selection::{BuildingPlacementContext, validate_building_placement};
use hw_world::{
    WorldMap, WorldMapWrite,
    layout::RIVER_Y_MIN,
    zones::{Site, Yard},
};

#[derive(Resource, Default)]
pub(super) struct Fixture {
    pub owner: Option<Entity>,
    anchor: Option<(i32, i32)>,
    stage: u8,
    pub since: f64,
}

pub(super) fn configure(app: &mut App, kind: BuildingAssetKind) -> Result<(), String> {
    let leg = std::env::var("HW_M2_ACCEPTANCE_LEG").map_err(|_| "M2 fixture leg is required")?;
    if !matches!(
        leg.as_str(),
        "save-load"
            | "deconstruct"
            | "placement"
            | "catalog"
            | "world-near-far"
            | "move-success"
            | "move-reject-cancel"
    ) && !matches!(
        (kind, leg.as_str()),
        (BuildingAssetKind::Tank, "water" | "companion")
            | (BuildingAssetKind::MudMixer, "refining")
    ) {
        return Err("M2 fixture leg has no deterministic driver".into());
    }
    app.init_resource::<Fixture>()
        .insert_resource(super::m2_lifecycle::Lifecycle::new(leg))
        .add_systems(
            PreUpdate,
            (setup, drive, super::m2_lifecycle::drive)
                .chain()
                .after(bevy::input::InputSystems),
        );
    Ok(())
}

#[derive(SystemParam)]
struct Setup<'w, 's> {
    fixture: ResMut<'w, Fixture>,
    session: Res<'w, BuildingArtSession>,
    pool: Res<'w, BuildingAssetPool>,
    map: WorldMapWrite<'w>,
    commands: Commands<'w, 's>,
    camera: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut PanCamera),
        With<hw_ui::camera::MainCamera>,
    >,
    exit: MessageWriter<'w, AppExit>,
    buildings: Query<'w, 's, &'static Building>,
}

fn setup(mut p: Setup) {
    if let Some(anchor) = p.fixture.anchor {
        if let Some(owner) = p.map.building_entity(anchor)
            && p.buildings.get(owner).is_ok()
        {
            p.fixture.owner = Some(owner);
        }
        return;
    }
    if p.pool.active(p.session.identity.kind) != Some(&p.session.identity) {
        return;
    }
    let kind = match p.session.identity.kind {
        BuildingAssetKind::Tank => BuildingType::Tank,
        BuildingAssetKind::MudMixer => BuildingType::MudMixer,
        _ => unreachable!("validated M2 fixture"),
    };
    // A private test site is an input fixture, not a placement-rule exception.
    let site = Site {
        min: WorldMap::grid_to_world(5, 5),
        max: WorldMap::grid_to_world(70, 65),
    };
    let yard = Yard {
        min: site.min,
        max: site.max,
    };
    let found = (8..60)
        .flat_map(|y| (8..65).map(move |x| (x, y)))
        .find_map(|anchor| {
            let geometry = building_geometry(kind, anchor, RIVER_Y_MIN);
            let read = WorldMapRef(p.map.as_ref());
            let context = BuildingPlacementContext {
                world: &read,
                in_site: site.contains(geometry.draw_pos),
                in_yard: yard.contains(geometry.draw_pos),
                is_wall_or_door_at: &|_| false,
                is_replaceable_wall_at: &|_| false,
            };
            let companion = (anchor.0, anchor.1 + 3);
            let clear = [companion, (companion.0 + 1, companion.1)]
                .into_iter()
                .all(|cell| {
                    p.map.is_walkable(cell.0, cell.1)
                        && !p.map.has_building(cell)
                        && !p.map.has_stockpile(cell)
                });
            (clear && validate_building_placement(&context, kind, anchor, &geometry).can_place)
                .then_some((geometry, companion))
        });
    let Some((geometry, companion)) = found else {
        error!("M2 fixture has no legal placement");
        p.exit.write(AppExit::error());
        return;
    };
    let Ok((mut camera, mut controller)) = p.camera.single_mut() else {
        return;
    };
    camera.translation.x = geometry.draw_pos.x;
    camera.translation.y = geometry.draw_pos.y;
    controller.zoom_factor = 1.0;
    camera.scale = Vec3::ONE;
    p.commands.spawn((site, Name::new("M2 acceptance site")));
    p.commands.spawn((yard, Name::new("M2 acceptance yard")));
    let mut blueprint = Blueprint::new(kind, geometry.occupied_grids.clone());
    for (resource, amount) in blueprint.required_materials.clone() {
        blueprint.deliver_material(resource, amount);
    }
    if blueprint.flexible_material_requirement.is_some() {
        blueprint.deliver_material(
            ResourceType::Wood,
            blueprint.remaining_material_amount(ResourceType::Wood),
        );
    }
    blueprint.progress = 1.0;
    let owner = p
        .commands
        .spawn((
            blueprint,
            Transform::from_translation(geometry.draw_pos.extend(hw_core::constants::Z_MAP)),
            Name::new("M2 acceptance owner"),
        ))
        .id();
    p.map
        .reserve_building_footprint(kind, owner, geometry.occupied_grids.clone());
    if kind == BuildingType::Tank
        && let Err(reason) = try_place_bucket_storage_companion(
            &mut p.commands,
            &mut p.map,
            owner,
            &geometry.occupied_grids,
            companion,
        )
    {
        error!("M2 companion rejected: {reason:?}");
        p.exit.write(AppExit::error());
    }
    p.fixture.owner = Some(owner);
    p.fixture.anchor = geometry.occupied_grids.first().copied();
}

type Actors<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut DamnedSoul,
        &'static mut Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
        &'static mut IdleState,
    ),
>;

#[derive(SystemParam)]
struct Drive<'w, 's> {
    fixture: ResMut<'w, Fixture>,
    lifecycle: Res<'w, super::m2_lifecycle::Lifecycle>,
    real: Res<'w, Time<Real>>,
    time: ResMut<'w, Time<Virtual>>,
    buildings: Query<
        'w,
        's,
        (&'static Building, Option<&'static StockpileVisualState>),
        Without<DamnedSoul>,
    >,
    actors: Actors<'w, 's>,
    commands: Commands<'w, 's>,
    exit: MessageWriter<'w, AppExit>,
}

fn drive(mut p: Drive) {
    let Some(owner) = p.fixture.owner else {
        return;
    };
    let Ok((building, stockpile)) = p.buildings.get(owner) else {
        return;
    };
    let now = p.real.elapsed_secs_f64();
    if p.fixture.stage == 0 {
        p.fixture.stage = 1;
        p.fixture.since = now;
        return;
    }
    let elapsed = now - p.fixture.since;
    if building.kind == BuildingType::Tank {
        let amount = match p.fixture.stage {
            1 if elapsed >= 4.0 => 1,
            2 if elapsed >= 8.0 => {
                let Some(capacity) = stockpile.map(|value| value.capacity) else {
                    return;
                };
                if !(2..=128).contains(&capacity) {
                    p.exit.write(AppExit::error());
                    return;
                }
                capacity - 1
            }
            _ => return,
        };
        for _ in 0..amount {
            p.commands.spawn((
                ResourceItem(ResourceType::Water),
                StoredIn(owner),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
        p.fixture.stage += 1;
    } else if building.kind == BuildingType::MudMixer && p.lifecycle.leg == "refining" {
        match p.fixture.stage {
            1 if elapsed >= 4.0 => {
                let actor = p
                    .actors
                    .iter()
                    .filter(|(_, _, _, _, _, task, _)| matches!(**task, AssignedTask::None))
                    .map(|(entity, ..)| entity)
                    .min_by_key(|entity| entity.to_bits());
                let Some(actor) = actor else {
                    error!("M2 fixture needs an idle real Soul");
                    p.exit.write(AppExit::error());
                    return;
                };
                let Ok((_, mut soul, mut transform, mut destination, mut path, mut task, mut idle)) =
                    p.actors.get_mut(actor)
                else {
                    return;
                };
                let anchor = p.fixture.anchor.expect("constructed fixture has an anchor");
                let pos = WorldMap::grid_to_world(anchor.0, anchor.1 - 1);
                soul.dream = 100.0;
                soul.fatigue = 0.0;
                soul.stress = 0.0;
                transform.translation.x = pos.x;
                transform.translation.y = pos.y;
                destination.0 = pos;
                *path = Path::default();
                *idle = IdleState::default();
                *task = AssignedTask::Refine(RefineData {
                    mixer: owner,
                    phase: RefinePhase::Refining { progress: 0.0 },
                });
                p.commands.entity(actor).insert((
                    WorkingOn(owner),
                    ActiveTaskIdentity::new(owner, owner, WorkType::Refine),
                ));
                p.commands.entity(owner).insert((
                    MudMixerStorage {
                        sand: 1,
                        rock: 1,
                        mud: 0,
                    },
                    hw_jobs::Designation {
                        work_type: WorkType::Refine,
                    },
                    hw_jobs::TaskSlots::new(1),
                ));
                p.commands.spawn((
                    ResourceItem(ResourceType::Water),
                    StoredIn(owner),
                    Transform::default(),
                    Visibility::Hidden,
                ));
                p.fixture.stage = 2;
            }
            2 if elapsed >= 4.5 => {
                p.time.pause();
                p.fixture.stage = 3;
            }
            3 if elapsed >= 6.5 => {
                p.time.unpause();
                p.fixture.stage = 4;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn only_refining_leg_injects_mixer_inputs_tasks_and_pause() {
        for leg in [
            "deconstruct",
            "save-load",
            "placement",
            "catalog",
            "world-near-far",
            "move-success",
            "move-reject-cancel",
            "refining",
        ] {
            let mut app = App::new();
            app.init_resource::<Time<Real>>()
                .init_resource::<Time<Virtual>>()
                .insert_resource(super::super::m2_lifecycle::Lifecycle::new(leg.into()))
                .add_message::<AppExit>()
                .add_systems(Update, drive);
            let owner = app
                .world_mut()
                .spawn((
                    Building {
                        kind: BuildingType::MudMixer,
                        is_provisional: false,
                    },
                    MudMixerStorage {
                        sand: 0,
                        rock: 0,
                        mud: 0,
                    },
                ))
                .id();
            let actor = app
                .world_mut()
                .spawn((
                    DamnedSoul::default(),
                    Transform::default(),
                    Destination(Vec2::ZERO),
                    Path::default(),
                    AssignedTask::None,
                    IdleState::default(),
                ))
                .id();
            app.insert_resource(Fixture {
                owner: Some(owner),
                anchor: Some((12, 15)),
                stage: 1,
                since: 1.0,
            });
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(5));
            app.update();
            let refining = leg == "refining";
            let storage = app.world().get::<MudMixerStorage>(owner).unwrap();
            assert_eq!(
                (storage.sand, storage.rock, storage.mud),
                if refining { (1, 1, 0) } else { (0, 0, 0) },
                "{leg}"
            );
            assert_eq!(
                matches!(
                    app.world().get::<AssignedTask>(actor),
                    Some(AssignedTask::Refine(_))
                ),
                refining,
                "{leg}"
            );
            assert_eq!(
                app.world().get::<WorkingOn>(actor).is_some(),
                refining,
                "{leg}"
            );
            assert_eq!(
                app.world().get::<ActiveTaskIdentity>(actor).is_some(),
                refining,
                "{leg}"
            );
            assert_eq!(
                app.world().get::<hw_jobs::Designation>(owner).is_some(),
                refining,
                "{leg}"
            );
            assert_eq!(
                app.world().get::<hw_jobs::TaskSlots>(owner).is_some(),
                refining,
                "{leg}"
            );
            let water_count = app
                .world_mut()
                .query::<(&ResourceItem, &StoredIn)>()
                .iter(app.world())
                .filter(|(item, stored)| item.0 == ResourceType::Water && stored.0 == owner)
                .count();
            assert_eq!(water_count, usize::from(refining), "{leg}");
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(1));
            app.update();
            assert_eq!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                refining,
                "{leg}"
            );
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(2));
            app.update();
            assert!(
                !app.world().resource::<Time<Virtual>>().is_paused(),
                "{leg}"
            );
        }
    }
}
