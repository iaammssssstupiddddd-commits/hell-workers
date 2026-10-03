//! Sequential domain operations from the single M2 profiling session.
use super::m2_fixture::Fixture;
use crate::systems::save::{
    SaveCatalogMode, SaveCatalogUi, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult,
};
use bevy::{ecs::message::MessageCursor, prelude::*};
use hw_core::familiar::{
    ActiveCommand, Familiar, FamiliarAiState, FamiliarOperation, FamiliarPolicy,
};
use hw_core::relationships::{CommandedBy, ManagedBy};
use hw_core::soul::{DamnedSoul, Destination, DreamState, IdleState, Path};
use hw_core::{SaveSlotId, WorldEpoch};
use hw_jobs::{
    AssignedTask, DeconstructionCommitOutcome, DeconstructionDesignationRequest,
    DeconstructionPending,
};
use hw_logistics::Inventory;
use hw_ui::UiIntent;
use serde_json::{Value, json};

#[derive(Resource, Default)]
pub(super) struct Lifecycle {
    pub leg: String,
    stage: u8,
    next: f64,
    aim_target: Option<Vec2>,
    rejected_placement: bool,
    pub destination: Option<(i32, i32)>,
    pub placement_blueprint: Option<Entity>,
    pub pulse_observed_since: Option<f64>,
    pub pulse_observation_complete: bool,
    cursor: MessageCursor<SaveLoadOutcome>,
    commits: MessageCursor<DeconstructionCommitOutcome>,
    pub events: Vec<Value>,
}

impl Lifecycle {
    pub(super) fn new(leg: String) -> Self {
        Self { leg, ..default() }
    }
}

pub(super) fn drive(world: &mut World) {
    let fixture = world.resource::<Fixture>();
    if fixture.since == 0.0
        || world.resource::<Time<Real>>().elapsed_secs_f64() - fixture.since < 12.0
    {
        return;
    }
    let Some(owner) = fixture.owner else {
        return;
    };
    world.resource_scope(|world, mut lifecycle: Mut<Lifecycle>| {
        let outcomes = lifecycle.cursor.read(world.resource::<Messages<SaveLoadOutcome>>()).cloned().collect::<Vec<_>>();
        for outcome in &outcomes {
            lifecycle.events.push(json!({"operation": format!("{:?}", outcome.operation), "result": format!("{:?}", outcome.result), "epoch": world.resource::<WorldEpoch>().get()}));
        }
        let commits = lifecycle.commits.read(world.resource::<Messages<DeconstructionCommitOutcome>>()).cloned().collect::<Vec<_>>();
        for outcome in commits {
            if outcome.target == owner {
                lifecycle.events.push(json!({"operation": "Deconstruct", "result": format!("{:?}", outcome.result), "owner": format!("{:?}", outcome.target)}));
            }
        }
        if lifecycle.events.len() > 16 { world.write_message(AppExit::error()); return; }
        if lifecycle.leg == "save-load" {
            if outcomes.iter().any(|outcome| outcome.result != SaveLoadResult::Succeeded) { world.write_message(AppExit::error()); return; }
            let ui = world.resource::<SaveCatalogUi>().clone();
            match lifecycle.stage {
                0 => { world.write_message(UiIntent::SaveGame); lifecycle.stage = 1; }
                1 if ui.mode == SaveCatalogMode::SaveCatalog => {
                    world.write_message(UiIntent::SelectSaveCatalogSlot { slot: SaveSlotId::Manual1, session: ui.session }); lifecycle.stage = 2;
                }
                2 if outcomes.iter().any(|outcome| outcome.operation == SaveLoadOperation::Save) => {
                    world.write_message(UiIntent::RequestLoadGame); lifecycle.stage = 3;
                }
                3 if ui.mode == SaveCatalogMode::LoadCatalog => {
                    world.write_message(UiIntent::SelectLoadCatalogSlot { slot: SaveSlotId::Manual1, session: ui.session }); lifecycle.stage = 4;
                }
                4 if ui.mode == (SaveCatalogMode::LoadConfirm { slot: SaveSlotId::Manual1, recovery: false }) => {
                    world.write_message(UiIntent::ConfirmLoadCatalogSlot { slot: SaveSlotId::Manual1, session: ui.session }); lifecycle.stage = 5;
                }
                5 if outcomes.iter().any(|outcome| outcome.operation == SaveLoadOperation::Load) => { lifecycle.stage = 6; }
                _ => {}
            }
        } else if lifecycle.leg == "deconstruct" {
            match lifecycle.stage {
                0 => {
                    world.write_message(DeconstructionDesignationRequest { request_id: 1, world_epoch: world.resource::<WorldEpoch>().get(), hit: Some(owner) });
                    lifecycle.stage = 1;
                }
                1 => {
                    let Some(order) = world.get::<DeconstructionPending>(owner).map(|pending| pending.order) else { return; };
                    let Some(position) = world.get::<Transform>(owner).map(|transform| transform.translation.truncate()) else { return; };
                    // Same ordinary assignment fixture as the existing native deconstruction recipe.
                    assign_order(world, order, position);
                    lifecycle.stage = 2;
                }
                _ => {}
            }
        } else {
            interaction(world, &mut lifecycle, owner);
        }
    });
}

fn aim(world: &mut World, position: Vec2) -> bool {
    let mut cameras = world.query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>();
    let Ok(mut camera) = cameras.single_mut(world) else {
        return false;
    };
    camera.translation.x = position.x;
    camera.translation.y = position.y;
    let mut windows = world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
    let Ok(mut window) = windows.single_mut(world) else {
        return false;
    };
    let cursor = Vec2::new(window.width() * 0.5, window.height() * 0.5);
    window.set_cursor_position(Some(cursor));
    true
}

fn legal_anchor(world: &mut World, kind: hw_jobs::BuildingType, anchor: (i32, i32)) -> bool {
    use hw_world::{
        WorldMap,
        layout::RIVER_Y_MIN,
        zones::{Site, Yard},
    };
    let geometry = crate::interface::selection::placement_geometry::building_geometry(
        kind,
        anchor,
        RIVER_Y_MIN,
    );
    let in_site = world
        .query::<&Site>()
        .iter(world)
        .any(|site| site.contains(geometry.draw_pos));
    let in_yard = world
        .query::<&Yard>()
        .iter(world)
        .any(|yard| yard.contains(geometry.draw_pos));
    let map = world.resource::<WorldMap>();
    let read = crate::world::map::WorldMapRef(map);
    let context = hw_ui::selection::BuildingPlacementContext {
        world: &read,
        in_site,
        in_yard,
        is_wall_or_door_at: &|_| false,
        is_replaceable_wall_at: &|_| false,
    };
    hw_ui::selection::validate_building_placement(&context, kind, anchor, &geometry).can_place
        && (kind != hw_jobs::BuildingType::Tank
            || [(anchor.0, anchor.1 + 3), (anchor.0 + 1, anchor.1 + 3)]
                .into_iter()
                .all(|cell| {
                    map.is_walkable(cell.0, cell.1)
                        && !map.has_building(cell)
                        && !map.has_stockpile(cell)
                }))
}

fn projection_matches(target: Vec2, local: Vec2, global: Vec2, projected: Vec2) -> bool {
    [target, local, global, projected]
        .into_iter()
        .all(|point| point.is_finite())
        && [local, global, projected]
            .into_iter()
            .all(|point| point.distance(target) <= 0.01)
        && hw_world::WorldMap::world_to_grid(projected) == hw_world::WorldMap::world_to_grid(target)
}

fn press_at_target(world: &mut World, driver: &mut Lifecycle) -> bool {
    let projected = (|| {
        let target = driver.aim_target?;
        let mut windows = world.query_filtered::<&Window, With<bevy::window::PrimaryWindow>>();
        let cursor = windows.single(world).ok()?.cursor_position()?;
        let mut cameras = world.query_filtered::<(&Camera, &Transform, &GlobalTransform), With<hw_ui::camera::MainCamera>>();
        let (camera, local, global) = cameras.single(world).ok()?;
        let projected = camera.viewport_to_world_2d(global, cursor).ok()?;
        projection_matches(
            target,
            local.translation.truncate(),
            global.translation().truncate(),
            projected,
        )
        .then_some((target, projected))
    })();
    let Some((target, projected)) = projected else {
        error!("M2 fixture cursor/camera projection did not settle at intended anchor");
        world.write_message(AppExit::error());
        return false;
    };
    driver.events.push(
        json!({"operation": "PlacementPress", "target": hw_world::WorldMap::world_to_grid(target),
        "projected": hw_world::WorldMap::world_to_grid(projected), "stage": driver.stage}),
    );
    world
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    true
}

fn assign_order(world: &mut World, order: Entity, position: Vec2) {
    let familiar = world
        .spawn((
            Familiar {
                name: "M2 lifecycle Familiar".into(),
                ..default()
            },
            FamiliarOperation {
                max_controlled_soul: 1,
                ..default()
            },
            FamiliarPolicy::default(),
            ActiveCommand::default(),
            FamiliarAiState::SearchingTask,
            Destination(position),
            Path::default(),
            Transform::from_translation(
                (position - Vec2::new(hw_core::constants::TILE_SIZE * 1.5, 0.0)).extend(0.0),
            ),
        ))
        .id();
    world.spawn((
        DamnedSoul::default(),
        DreamState::default(),
        IdleState::default(),
        AssignedTask::None,
        Destination(position),
        Path::default(),
        Inventory::default(),
        CommandedBy(familiar),
        Visibility::Visible,
        Transform::from_translation(
            (position - Vec2::new(hw_core::constants::TILE_SIZE * 1.5, 0.0)).extend(0.0),
        ),
    ));
    world.entity_mut(order).insert(ManagedBy(familiar));
    world.flush();
}

fn interaction(world: &mut World, driver: &mut Lifecycle, owner: Entity) {
    use hw_jobs::{Blueprint, Building, BuildingType, MovePlantTask};
    use hw_world::WorldMap;
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    // Keep the synthetic pointer away from edge-pan during transform propagation.
    // Do not overwrite camera transforms or GlobalTransform to conceal drift.
    if driver.aim_target.is_some() && driver.stage < 9 {
        let mut windows = world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
        if let Ok(mut window) = windows.single_mut(world) {
            let center = Vec2::new(window.width() * 0.5, window.height() * 0.5);
            window.set_cursor_position(Some(center));
        }
    }
    if now < driver.next {
        return;
    }
    let Some(kind) = world.get::<Building>(owner).map(|building| building.kind) else {
        return;
    };
    match driver.leg.as_str() {
        "catalog" if driver.stage == 0 => {
            world.write_message(UiIntent::ToggleArchitect);
            driver.stage = 1;
        }
        "world-near-far" if driver.stage == 0 => {
            let mut cameras = world.query_filtered::<(
                &mut Transform,
                &mut bevy::camera_controller::pan_camera::PanCamera,
            ), With<hw_ui::camera::MainCamera>>();
            if let Ok((mut transform, mut camera)) = cameras.single_mut(world) {
                camera.zoom_factor = 2.5;
                transform.scale = Vec3::splat(2.5);
                driver.stage = 1;
            }
        }
        "placement" | "move-success" | "move-reject-cancel" | "companion" => match driver.stage {
            0 => {
                let destination = if driver.leg == "move-reject-cancel" {
                    Some((-10, -10))
                } else {
                    (8..60)
                        .flat_map(|y| (8..65).map(move |x| (x, y)))
                        .find(|&anchor| legal_anchor(world, kind, anchor))
                };
                let Some(destination) = destination else {
                    world.write_message(AppExit::error());
                    return;
                };
                driver.destination = Some(destination);
                if driver.leg == "placement" {
                    world.write_message(UiIntent::SelectBuild(kind));
                } else {
                    world.write_message(UiIntent::MovePlantBuilding(owner));
                }
                let initial = if driver.leg == "placement" {
                    (-10, -10)
                } else {
                    destination
                };
                if aim(world, WorldMap::grid_to_world(initial.0, initial.1)) {
                    driver.aim_target = Some(WorldMap::grid_to_world(initial.0, initial.1));
                    driver.stage = 1;
                    driver.next = now + 2.0;
                } else {
                    world.write_message(AppExit::error());
                }
            }
            1 => {
                if driver.leg == "placement"
                    && driver.rejected_placement
                    && !legal_anchor(world, kind, driver.destination.unwrap())
                {
                    error!("M2 placement destination is no longer legal");
                    world.write_message(AppExit::error());
                    return;
                }
                if !press_at_target(world, driver) {
                    return;
                }
                driver.stage = 2;
                driver.next = now + 0.2;
            }
            2 => {
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .release(MouseButton::Left);
                driver.stage = 3;
                driver.next = now + 2.0;
            }
            3 if driver.leg == "placement" && !driver.rejected_placement => {
                let rejected = world
                    .get_resource::<hw_ui::selection::PlacementFeedbackState>()
                    .and_then(|feedback| feedback.visible(world.resource::<Time<Real>>().elapsed()))
                    .is_some_and(|feedback| feedback.header() == "Cannot place");
                if !rejected {
                    error!("M2 placement rejection was not observed before legal retry");
                    world.write_message(AppExit::error());
                    return;
                }
                driver
                    .events
                    .push(json!({"operation": "PlacementRejected", "target": [-10, -10]}));
                driver.rejected_placement = true;
                let (x, y) = driver.destination.unwrap();
                if aim(world, WorldMap::grid_to_world(x, y)) {
                    driver.aim_target = Some(WorldMap::grid_to_world(x, y));
                    driver.stage = 1;
                    driver.next = now + 2.0;
                } else {
                    world.write_message(AppExit::error());
                }
            }
            3 if driver.leg == "move-reject-cancel" => {
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Right);
                driver.stage = 4;
                driver.next = now + 0.2;
            }
            4 if driver.leg == "move-reject-cancel" => {
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .release(MouseButton::Right);
                driver.stage = 9;
            }
            3 if kind == BuildingType::Tank => {
                let (x, y) = driver.destination.unwrap();
                if aim(world, WorldMap::grid_to_world(x, y + 3)) {
                    driver.aim_target = Some(WorldMap::grid_to_world(x, y + 3));
                    driver.stage = 4;
                    driver.next = now + 2.0;
                } else {
                    world.write_message(AppExit::error());
                }
            }
            3 => {
                driver.stage = 6;
            }
            4 => {
                if !press_at_target(world, driver) {
                    return;
                }
                driver.stage = 5;
                driver.next = now + 0.2;
            }
            5 => {
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .release(MouseButton::Left);
                driver.stage = 6;
                driver.next = now + 3.0;
            }
            6 if driver.leg == "placement" => {
                let Some(entity) = world
                    .resource::<WorldMap>()
                    .building_entity(driver.destination.unwrap())
                else {
                    error!("M2 placement did not create an owner at the intended anchor");
                    world.write_message(AppExit::error());
                    return;
                };
                let Some(mut blueprint) = world.get_mut::<Blueprint>(entity) else {
                    error!("M2 placement destination is not a Blueprint");
                    world.write_message(AppExit::error());
                    return;
                };
                for (resource, amount) in blueprint.required_materials.clone() {
                    blueprint.deliver_material(resource, amount);
                }
                if blueprint.flexible_material_requirement.is_some() {
                    let remaining =
                        blueprint.remaining_material_amount(hw_logistics::ResourceType::Wood);
                    blueprint.deliver_material(hw_logistics::ResourceType::Wood, remaining);
                }
                // Fixture input only: let production visuals observe Building before completion.
                blueprint.progress = 0.25;
                driver.placement_blueprint = Some(entity);
                driver.pulse_observed_since = None;
                driver.pulse_observation_complete = false;
                driver.stage = 7;
            }
            7 if driver.leg == "placement" => {
                if !driver.pulse_observation_complete {
                    return;
                }
                let Some(entity) = driver.placement_blueprint else {
                    return;
                };
                if world
                    .resource::<WorldMap>()
                    .building_entity(driver.destination.unwrap())
                    != Some(entity)
                {
                    return;
                }
                let Some(mut blueprint) = world.get_mut::<Blueprint>(entity) else {
                    return;
                };
                blueprint.progress = 1.0;
                driver.stage = 9;
            }
            6 => {
                let mut query = world.query::<(Entity, &MovePlantTask)>();
                let task = query
                    .iter(world)
                    .find(|(_, task)| task.building == owner)
                    .map(|(entity, _)| entity);
                if let Some(task) = task {
                    let position = world
                        .get::<Transform>(owner)
                        .unwrap()
                        .translation
                        .truncate();
                    assign_order(world, task, position);
                    driver.stage = 9;
                }
            }
            _ => {}
        },
        _ => {}
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    fn both_kind_destinations_use_production_zone_bounds_and_occupancy_rules() {
        use hw_world::{
            WorldMap,
            zones::{Site, Yard},
        };
        for kind in [hw_jobs::BuildingType::Tank, hw_jobs::BuildingType::MudMixer] {
            let mut world = World::new();
            world.insert_resource(WorldMap::default());
            assert!(!legal_anchor(&mut world, kind, (12, 15)));
            let min = WorldMap::grid_to_world(5, 5);
            let max = WorldMap::grid_to_world(70, 65);
            world.spawn((Site { min, max }, Yard { min, max }));
            assert!(legal_anchor(&mut world, kind, (12, 15)));
            assert!(!legal_anchor(&mut world, kind, (-10, -10)));
            let occupant = world.spawn_empty().id();
            world
                .resource_mut::<WorldMap>()
                .set_building((12, 15), occupant);
            assert!(!legal_anchor(&mut world, kind, (12, 15)));
        }
    }

    #[test]
    fn both_kind_anchors_require_propagated_camera_and_exact_cursor() {
        for kind in [hw_jobs::BuildingType::Tank, hw_jobs::BuildingType::MudMixer] {
            for anchor in [(-10, -10), (12, 15)] {
                let target = hw_world::WorldMap::grid_to_world(anchor.0, anchor.1);
                let geometry = crate::interface::selection::placement_geometry::building_geometry(
                    kind,
                    anchor,
                    hw_world::layout::RIVER_Y_MIN,
                );
                assert!(geometry.occupied_grids.contains(&anchor));
                assert!(projection_matches(target, target, target, target));
                let drifted = target + Vec2::splat(hw_core::constants::TILE_SIZE);
                assert!(!projection_matches(target, drifted, target, target));
                assert!(!projection_matches(target, target, drifted, target));
                assert!(!projection_matches(target, target, target, drifted));
                assert!(!projection_matches(
                    target,
                    target,
                    target,
                    Vec2::splat(f32::NAN)
                ));
            }
        }
    }
}
