use super::*;
use crate::assets::GameAssets;
use crate::interface::selection::{
    building_place::try_place_bucket_storage_companion, placement_geometry::building_geometry,
    soul_spa_place::spawn_soul_spa,
};
use crate::plugins::startup::Building3dHandles;
use crate::world::map::WorldMapRef;
use bevy::camera_controller::pan_camera::PanCamera;
use hw_core::relationships::{RestingIn, StoredIn, WorkingOn};
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_energy::SoulSpaSite;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, Blueprint, GeneratePowerData, GeneratePowerPhase, RefineData,
    RefinePhase, WorkType,
};
use hw_logistics::{ResourceItem, ResourceType};
use hw_ui::selection::{BuildingPlacementContext, validate_building_placement};
use hw_world::{WorldMap, Yard, layout::RIVER_Y_MIN};

#[derive(SystemParam)]
pub(crate) struct SetupParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    art_session: Option<Res<'w, BuildingArtSession>>,
    gallery:
        Option<Res<'w, crate::assets::building_asset_set::gallery_capture::GalleryCaptureSession>>,
    state: ResMut<'w, BuildingArtStaticState>,
    commands: Commands<'w, 's>,
    world_map: WorldMapWrite<'w>,
    assets: Res<'w, GameAssets>,
    handles: Res<'w, Building3dHandles>,
    actors: Query<'w, 's, Entity, With<DamnedSoul>>,
    camera: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut PanCamera),
        With<hw_ui::camera::MainCamera>,
    >,
    time: ResMut<'w, Time<Virtual>>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn setup_building_art_static_system(mut p: SetupParams) {
    if !p.config.enabled() || p.config.workload() != PerfWorkload::BuildingArtStatic {
        return;
    }
    p.time.pause();
    if p.state.phase != Phase::Inactive {
        return;
    }
    let mut actors = p.actors.iter().collect::<Vec<_>>();
    if actors.len() != p.config.soul_count() as usize {
        return;
    }
    actors.sort_by_key(|entity| entity.to_bits());
    let mut specs = p.gallery.as_ref().map_or_else(
        || layout::layout(p.config.size()),
        |session| {
            session.group().map_or_else(
                || layout::layout(p.config.size()),
                layout::group_gallery_layout,
            )
        },
    );
    if let Some(page) = p
        .art_session
        .as_ref()
        .and_then(|session| session.spa_mask_page)
        && let Err(reason) = layout::apply_spa_mask_page(&mut specs, page)
    {
        fail(&mut p.state, &mut p.exit, reason);
        return;
    }
    if let Some(session) = p
        .art_session
        .as_ref()
        .filter(|session| session.is_context())
    {
        specs.retain(|spec| {
            crate::systems::visual::building_presentation::asset_kind(spec.kind)
                == Some(session.identity.kind)
                || (session.is_m5_context()
                    && session.identity.kind == BuildingAssetKind::OutdoorLamp
                    && spec.kind == BuildingType::SoulSpa
                    && spec.ordinal >= 2)
        });
    }
    let gallery_yards = p
        .gallery
        .as_ref()
        .map(|_| layout::group_gallery_yards(&specs));
    let validation = gallery_yards.as_ref().map_or_else(
        || validate_layout(&specs, p.world_map.as_ref(), p.config.size()),
        |yards| validate_layout_with_yards(&specs, p.world_map.as_ref(), yards),
    );
    if let Err(reason) = validation {
        fail(&mut p.state, &mut p.exit, reason);
        return;
    }
    if p.art_session
        .as_ref()
        .is_some_and(|session| session.is_context())
        && let Err(reason) = super::context::validate_layout(&specs, p.world_map.as_ref())
    {
        fail(&mut p.state, &mut p.exit, reason);
        return;
    }
    let Ok((mut camera, mut controller)) = p.camera.single_mut() else {
        fail(
            &mut p.state,
            &mut p.exit,
            "requires exactly one main camera".into(),
        );
        return;
    };
    initialize_camera(
        &mut camera,
        &mut controller,
        p.gallery.as_ref().map_or_else(
            || layout::camera_frame(p.art_session.as_deref(), p.config.size()),
            |session| layout::group_gallery_camera(session, &specs),
        ),
    );
    p.commands
        .spawn((layout::site(), Name::new("BuildingArtStatic Site")));
    for yard in gallery_yards.unwrap_or_else(|| layout::yards(p.config.size())) {
        p.commands
            .spawn((yard, Name::new("BuildingArtStatic Yard")));
    }
    if p.art_session
        .as_ref()
        .is_some_and(|session| session.is_context())
    {
        for spec in &specs {
            if p.art_session.as_ref().is_some_and(|session| {
                session.is_m5_context()
                    && crate::systems::visual::building_presentation::asset_kind(spec.kind)
                        != Some(session.identity.kind)
            }) {
                continue;
            }
            for grid in super::context::wall_grids(spec) {
                if !p.world_map.as_ref().is_walkable(grid.0, grid.1)
                    || p.world_map.as_ref().has_building(grid)
                    || p.world_map.as_ref().has_stockpile(grid)
                {
                    fail(
                        &mut p.state,
                        &mut p.exit,
                        "occupied context wall grid".into(),
                    );
                    return;
                }
                let wall = crate::systems::jobs::wall_construction::spawn_wall_shell(
                    &mut p.commands,
                    &p.handles,
                    grid,
                    false,
                );
                p.world_map
                    .register_completed_building_footprint(BuildingType::Wall, wall, [grid]);
            }
        }
    }
    // Both support walls and their floors use existing completion factories.
    for spec in specs.iter().filter(|s| s.kind == BuildingType::Door) {
        let floors = spec
            .supports()
            .into_iter()
            .map(|grid| {
                (
                    crate::systems::jobs::floor_construction::spawn_completed_floor_tile(
                        &mut p.commands,
                        &p.handles,
                        grid,
                    ),
                    grid,
                )
            })
            .collect::<Vec<_>>();
        crate::systems::jobs::floor_construction::register_completed_floors(
            &mut p.world_map,
            &floors,
        );
        for grid in spec.supports() {
            let entity = crate::systems::jobs::wall_construction::spawn_wall_shell(
                &mut p.commands,
                &p.handles,
                grid,
                false,
            );
            p.world_map
                .register_completed_building_footprint(BuildingType::Wall, entity, [grid]);
        }
    }
    for spec in &specs {
        if spec.kind == BuildingType::SoulSpa {
            let (owner, _) = spawn_soul_spa(
                &mut p.commands,
                &mut p.world_map,
                &spec.tiles,
                spec.center,
                None,
                &p.assets,
                &p.handles,
            );
            p.commands.entity(owner).insert(operational_spa_state());
            continue;
        }
        let mut blueprint = Blueprint::new(spec.kind, spec.tiles.clone());
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
        let entity = p
            .commands
            .spawn((
                blueprint,
                Transform::from_translation(spec.center.extend(hw_core::constants::Z_MAP)),
            ))
            .id();
        p.world_map
            .reserve_building_footprint(spec.kind, entity, spec.tiles.clone());
        if spec.kind == BuildingType::Tank
            && let Err(rejection) = try_place_bucket_storage_companion(
                &mut p.commands,
                &mut p.world_map,
                entity,
                &spec.tiles,
                spec.companion(),
            )
        {
            fail(
                &mut p.state,
                &mut p.exit,
                format!("Tank companion rejected: {rejection:?}"),
            );
            return;
        }
    }
    p.state.specs = specs;
    p.state.actors = actors;
    p.state.phase = Phase::Spawned;
}

fn operational_spa_state() -> SoulSpaSite {
    let site = SoulSpaSite::default();
    // This paused reference starts after construction, not during delivery.
    // Setting the count alone does not transition the production phase: the
    // delivery system requires a newly consumed Bone before doing that.
    SoulSpaSite {
        bones_delivered: site.bones_required,
        phase: SoulSpaPhase::Operational,
        ..site
    }
}

fn initialize_camera(
    camera: &mut Transform,
    controller: &mut PanCamera,
    (center, scale): (Vec2, f32),
) {
    camera.translation.x = center.x;
    camera.translation.y = center.y;
    // Bevy 0.19 writes all scale axes from zoom_factor even without input.
    // Seed both once; never overwrite later input to conceal camera drift.
    controller.zoom_factor = scale;
    camera.scale = Vec3::splat(scale);
}

fn validate_layout(
    specs: &[Specimen],
    map: &WorldMap,
    size: super::super::PerfScenarioSize,
) -> Result<(), String> {
    let yards = layout::yards(size);
    validate_layout_with_yards(specs, map, &yards)
}

fn validate_layout_with_yards(
    specs: &[Specimen],
    map: &WorldMap,
    yards: &[Yard],
) -> Result<(), String> {
    let site = layout::site();
    let supports = specs
        .iter()
        .filter(|s| s.kind == BuildingType::Door)
        .flat_map(Specimen::supports)
        .collect::<HashSet<_>>();
    let read = WorldMapRef(map);
    let mut occupied = HashSet::new();
    for spec in specs {
        if !layout::KINDS.contains(&spec.kind) {
            return Err(format!(
                "kind excluded from nine-building reference: {:?}",
                spec.kind
            ));
        }
        let geometry = building_geometry(spec.kind, spec.anchor, RIVER_Y_MIN);
        let context = BuildingPlacementContext {
            world: &read,
            in_site: site.contains(spec.center),
            in_yard: yards.iter().any(|yard| yard.contains(spec.center)),
            is_wall_or_door_at: &|grid| supports.contains(&grid),
            is_replaceable_wall_at: &|_| false,
        };
        let validation = validate_building_placement(&context, spec.kind, spec.anchor, &geometry);
        if !validation.can_place {
            return Err(format!(
                "illegal {:?}/{}: {validation:?}",
                spec.kind, spec.ordinal
            ));
        }
        let mut cells = spec.tiles.clone();
        if spec.kind == BuildingType::Tank {
            let (x, y) = spec.companion();
            cells.extend([(x, y), (x + 1, y)]);
        }
        if spec.kind == BuildingType::Door {
            cells.extend(spec.supports());
        }
        for cell in cells {
            if !occupied.insert(cell) {
                return Err(format!("overlap at {cell:?}"));
            }
            if !map.is_walkable(cell.0, cell.1) || map.has_building(cell) || map.has_stockpile(cell)
            {
                return Err(format!("occupied support/companion at {cell:?}"));
            }
        }
    }
    Ok(())
}

type SeedActors<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut DamnedSoul,
        &'static mut Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
        &'static mut IdleState,
    ),
>;

type ActorRelations<'w, 's> =
    Query<'w, 's, (), Or<(With<WorkingOn>, With<RestingIn>, With<ActiveTaskIdentity>)>>;

#[derive(SystemParam)]
pub(crate) struct SeedParams<'w, 's> {
    art_session: Option<Res<'w, BuildingArtSession>>,
    state: ResMut<'w, BuildingArtStaticState>,
    world_map: WorldMapRead<'w>,
    commands: Commands<'w, 's>,
    buildings: Query<'w, 's, &'static Building>,
    tiles: Query<'w, 's, (Entity, &'static SoulSpaTile)>,
    actors: SeedActors<'w, 's>,
    actor_relations: ActorRelations<'w, 's>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn seed_building_art_static_system(mut p: SeedParams) {
    if p.state.phase != Phase::Spawned {
        return;
    }
    let specs = p.state.specs.clone();
    let mut owners = Vec::new();
    for spec in &specs {
        let Some(owner) = p.world_map.building_entity(spec.tiles[0]) else {
            return;
        };
        if p.buildings.get(owner).is_err() {
            return;
        }
        owners.push(owner);
    }
    let mut actor_index = 0;
    for (spec, &owner) in specs.iter().zip(&owners) {
        let water = match spec.kind {
            BuildingType::Tank => spec.water_count(),
            BuildingType::MudMixer if spec.quarter() >= 2 => 1,
            _ => 0,
        };
        for _ in 0..water {
            p.commands.spawn((
                ResourceItem(ResourceType::Water),
                StoredIn(owner),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
        let targets: Vec<(Entity, Vec2, AssignedTask)> = match spec.kind {
            BuildingType::MudMixer if spec.quarter() >= 2 => {
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
                vec![(
                    owner,
                    WorldMap::grid_to_world(spec.anchor.0, spec.anchor.1 - 1),
                    AssignedTask::Refine(RefineData {
                        mixer: owner,
                        phase: RefinePhase::Refining { progress: 0.0 },
                    }),
                )]
            }
            BuildingType::RestArea => (0..spec.rest_count())
                .map(|_| {
                    (
                        owner,
                        WorldMap::grid_to_world(spec.anchor.0, spec.anchor.1 - 1),
                        AssignedTask::None,
                    )
                })
                .collect(),
            BuildingType::SoulSpa => {
                let mut targets = Vec::new();
                for (index, &grid) in spec.tiles.iter().enumerate() {
                    if spec.spa_mask() & (1 << index) == 0 {
                        continue;
                    }
                    let matches = p
                        .tiles
                        .iter()
                        .filter(|(_, tile)| tile.parent_site == owner && tile.grid_pos == grid)
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        fail(
                            &mut p.state,
                            &mut p.exit,
                            "Spa tile missing/duplicated during seed".into(),
                        );
                        return;
                    }
                    let tile = matches[0].0;
                    let tile_pos = WorldMap::grid_to_world(grid.0, grid.1);
                    targets.push((
                        tile,
                        tile_pos,
                        AssignedTask::GeneratePower(GeneratePowerData {
                            tile,
                            tile_pos,
                            phase: GeneratePowerPhase::Generating,
                        }),
                    ));
                }
                targets
            }
            _ => Vec::new(),
        };
        for (target, position, task) in targets {
            let Some(&actor) = p.state.actors.get(actor_index) else {
                fail(&mut p.state, &mut p.exit, "insufficient real Souls".into());
                return;
            };
            actor_index += 1;
            let Ok((mut soul, mut transform, mut destination, mut path, mut assigned, mut idle)) =
                p.actors.get_mut(actor)
            else {
                fail(
                    &mut p.state,
                    &mut p.exit,
                    "incomplete real Soul shell".into(),
                );
                return;
            };
            if p.art_session
                .as_ref()
                .is_some_and(|session| session.is_m5_context() || session.is_m2_context())
                && (!matches!(*assigned, AssignedTask::None)
                    || p.actor_relations.get(actor).is_ok())
            {
                fail(
                    &mut p.state,
                    &mut p.exit,
                    "M5 source cannot replace an existing Soul task or relation".into(),
                );
                return;
            }
            soul.dream = 100.0;
            soul.fatigue = 0.0;
            soul.stress = 0.0;
            transform.translation.x = position.x;
            transform.translation.y = position.y;
            destination.0 = position;
            *path = Path::default();
            *assigned = task;
            *idle = IdleState::default();
            if spec.kind == BuildingType::RestArea {
                idle.behavior = IdleBehavior::Resting;
                p.commands
                    .entity(actor)
                    .insert((RestingIn(owner), Visibility::Hidden));
            } else {
                let work = if spec.kind == BuildingType::MudMixer {
                    WorkType::Refine
                } else {
                    WorkType::GeneratePower
                };
                p.commands.entity(actor).insert((
                    WorkingOn(target),
                    ActiveTaskIdentity::new(target, target, work),
                ));
            }
        }
    }
    if p.art_session
        .as_ref()
        .is_some_and(|session| session.is_m5_context() || session.is_m2_context())
    {
        let m2 = p
            .art_session
            .as_ref()
            .is_some_and(|session| session.is_m2_context());
        let source_workers = if m2 {
            usize::from(
                p.art_session
                    .as_ref()
                    .is_some_and(|session| session.identity.kind == BuildingAssetKind::MudMixer),
            ) * 2
        } else if p
            .art_session
            .as_ref()
            .is_some_and(|session| session.identity.kind == BuildingAssetKind::OutdoorLamp)
        {
            6
        } else {
            0
        };
        if actor_index != source_workers
            || p.state.actors.len() != 8 + source_workers
            || specs.len() != 4 + usize::from(!m2 && source_workers > 0) * 2
        {
            fail(
                &mut p.state,
                &mut p.exit,
                "M5 context requires eight idle Souls plus its declared source workers".into(),
            );
            return;
        }
        for spec in &specs {
            if crate::systems::visual::building_presentation::asset_kind(spec.kind)
                != p.art_session.as_ref().map(|session| session.identity.kind)
            {
                continue;
            }
            for side in [-1.0, 1.0] {
                let actor = p.state.actors[actor_index];
                actor_index += 1;
                let Ok((_, mut transform, mut destination, mut path, assigned, mut idle)) =
                    p.actors.get_mut(actor)
                else {
                    fail(
                        &mut p.state,
                        &mut p.exit,
                        "M5 context Soul shell missing".into(),
                    );
                    return;
                };
                if !matches!(*assigned, AssignedTask::None) || p.actor_relations.get(actor).is_ok()
                {
                    fail(
                        &mut p.state,
                        &mut p.exit,
                        "M5 context cannot replace an existing Soul task or relation".into(),
                    );
                    return;
                }
                let point = spec.center + Vec2::Y * (side * 12.0);
                transform.translation.x = point.x;
                transform.translation.y = point.y;
                destination.0 = point;
                *path = Path::default();
                *idle = IdleState::default();
            }
        }
    }
    if actor_index != p.state.actors.len() {
        fail(&mut p.state, &mut p.exit, "unused real Souls".into());
        return;
    }
    p.state.owners = owners;
    p.state.phase = Phase::Seeded;
}

#[cfg(test)]
mod tests {
    use super::super::super::PerfScenarioSize;
    use super::*;

    #[test]
    fn completed_spa_seed_activates_real_tiles_while_paused() {
        use crate::systems::jobs::soul_spa_construction::soul_spa_tile_activate_system;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, soul_spa_tile_activate_system);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let site = app.world_mut().spawn(SoulSpaSite::default()).id();
        let tiles = (0..4)
            .map(|index| {
                app.world_mut()
                    .spawn(SoulSpaTile {
                        parent_site: site,
                        grid_pos: (index % 2, index / 2),
                    })
                    .id()
            })
            .collect::<Vec<_>>();

        // Reproduce the old setup: completed counts do not activate a site.
        let mut old_seed = SoulSpaSite::default();
        old_seed.bones_delivered = old_seed.bones_required;
        app.world_mut().entity_mut(site).insert(old_seed);
        app.update();
        assert!(
            tiles
                .iter()
                .all(|&tile| app.world().get::<hw_jobs::Designation>(tile).is_none())
        );

        app.world_mut()
            .entity_mut(site)
            .insert(operational_spa_state());
        app.update();
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        let state = app.world().get::<SoulSpaSite>(site).unwrap();
        assert_eq!(state.phase, SoulSpaPhase::Operational);
        assert_eq!(state.bones_delivered, state.bones_required);
        for tile in tiles {
            assert_eq!(
                app.world()
                    .get::<hw_jobs::Designation>(tile)
                    .unwrap()
                    .work_type,
                WorkType::GeneratePower
            );
            assert_eq!(app.world().get::<hw_jobs::TaskSlots>(tile).unwrap().max, 1);
        }
    }

    #[test]
    fn camera_initialization_survives_production_controller_without_repair() {
        use bevy::camera_controller::pan_camera::PanCameraPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, PanCameraPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>();
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_scale(Vec3::new(5.0, 5.0, 1.0)),
                PanCamera::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().scale,
            Vec3::ONE
        );
        {
            let world = app.world_mut();
            let mut query = world.query::<(&mut Transform, &mut PanCamera)>();
            let (mut transform, mut controller) = query.single_mut(world).unwrap();
            initialize_camera(
                &mut transform,
                &mut controller,
                layout::camera_frame(None, crate::plugins::startup::PerfScenarioSize::Small),
            );
        }
        for _ in 0..3 {
            app.update();
            let transform = app.world().get::<Transform>(camera).unwrap();
            assert_eq!(
                transform.translation.truncate(),
                WorldMap::grid_to_world(46, 37)
            );
            assert_eq!(transform.scale, Vec3::splat(layout::CAMERA_SCALE));
        }
        app.world_mut()
            .get_mut::<PanCamera>(camera)
            .unwrap()
            .zoom_factor = 4.0;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().scale,
            Vec3::splat(4.0)
        );
    }

    #[test]
    fn gallery_groups_preserve_legal_terrain_and_isolated_lamp_power_yards() {
        let generated = hw_world::generate_world_layout(20260920);
        let mut map = WorldMap::default();
        for (index, &terrain) in generated.terrain_tiles.iter().enumerate() {
            map.set_terrain_at_idx(index, terrain);
        }
        for group in [
            crate::assets::building_asset_set::BuildingArtGroup::M2,
            crate::assets::building_asset_set::BuildingArtGroup::M3,
            crate::assets::building_asset_set::BuildingArtGroup::M5,
        ] {
            let specs = layout::group_gallery_layout(group);
            let yards = layout::group_gallery_yards(&specs);
            assert_eq!(validate_layout_with_yards(&specs, &map, &yards), Ok(()));
            for lamp in specs.iter().filter(|s| s.kind == BuildingType::OutdoorLamp) {
                let spa = specs
                    .iter()
                    .find(|s| s.kind == BuildingType::SoulSpa && s.ordinal == lamp.ordinal)
                    .unwrap();
                let contains = |yard: &Yard, position: Vec2| {
                    position.x >= yard.min.x
                        && position.x <= yard.max.x
                        && position.y >= yard.min.y
                        && position.y <= yard.max.y
                };
                assert_eq!(
                    yards
                        .iter()
                        .any(|yard| contains(yard, lamp.center) && contains(yard, spa.center)),
                    lamp.ordinal >= 2
                );
            }
        }
        // No gallery operation may change the static performance specimen contract.
        assert_eq!(
            layout::layout(PerfScenarioSize::Small)
                .iter()
                .find(|s| s.kind == BuildingType::Door)
                .unwrap()
                .anchor,
            (8, 43)
        );
    }

    #[test]
    fn nine_kinds_place_on_actual_generated_terrain_without_rewriting_it() {
        let generated = hw_world::generate_world_layout(20260920);
        let mut map = WorldMap::default();
        for (index, &terrain) in generated.terrain_tiles.iter().enumerate() {
            map.set_terrain_at_idx(index, terrain);
        }
        for size in [PerfScenarioSize::Small, PerfScenarioSize::Medium] {
            let specs = layout::layout(size);
            assert_eq!(validate_layout(&specs, &map, size), Ok(()));
            let mut with_bridge = specs;
            with_bridge[0].kind = BuildingType::Bridge;
            assert!(
                validate_layout(&with_bridge, &map, size)
                    .unwrap_err()
                    .contains("excluded from nine-building reference")
            );
        }
    }
}
