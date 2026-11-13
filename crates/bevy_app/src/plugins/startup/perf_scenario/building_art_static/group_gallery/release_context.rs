//! Released-storyboard context, prepared once before the first published probe.
//! Existing Door supports stay untouched. One free Floor and one extra ordinary
//! Soul carrying Wood supplement the core fifteen fixture actors. No natural
//! construction, hauling, or running simulation is claimed.
use super::*;
use bevy::camera::{primitives::MeshAabb, visibility::RenderLayers};
use hw_core::soul::DamnedSoul;
use hw_jobs::{AssignedTask, Building, BuildingType};
use hw_logistics::{Inventory, ResourceItem, ResourceType};
use hw_world::WorldMap;

#[derive(Clone, Resource)]
struct Context {
    original_souls: Vec<Entity>,
    floor: Entity,
    floor_grid: (i32, i32),
    walls: Vec<Entity>,
    carrier_grid: (i32, i32),
    carrier: Option<Entity>,
    item: Option<Entity>,
    stable: u64,
    pending: Option<Value>,
}

#[derive(Clone, Copy)]
enum PreparationStep {
    Floor,
    HeldItem,
}

#[derive(Default, Resource)]
struct PreparationSteps {
    floor: bool,
    held_item: bool,
}

// Invalidation is restricted to two declared, one-time, unpublished mutations.
// Once a probe exists, ordinary snapshot/resource drift remains an error.
fn begin_preparation(world: &mut World, step: PreparationStep) -> Result<(), String> {
    let Some(GalleryCaptureSession::Released(session)) =
        world.get_resource::<GalleryCaptureSession>()
    else {
        return Err("context preparation requires the released-only gallery".into());
    };
    let root = session
        .status_path
        .parent()
        .ok_or("missing context output directory")?;
    if world
        .get_resource::<CaptureState>()
        .is_none_or(|capture| capture.ready.is_some() || capture.completed)
        || [
            session.status_path.clone(),
            root.join("capture-ack.json"),
            root.join("final.json"),
        ]
        .iter()
        .any(|path| path.symlink_metadata().is_ok())
    {
        return Err("context preparation cannot mutate published capture state".into());
    }
    let state = world.get_resource::<Context>();
    let steps = world.get_resource::<PreparationSteps>();
    let allowed = match step {
        PreparationStep::Floor => state.is_none() && steps.is_none(),
        PreparationStep::HeldItem => {
            state.is_some_and(|state| state.carrier.is_none() && state.item.is_none())
                && steps.is_some_and(|steps| steps.floor && !steps.held_item)
        }
    };
    if !allowed {
        return Err("duplicate or out-of-order context preparation".into());
    }
    world.init_resource::<PreparationSteps>();
    match step {
        PreparationStep::Floor => world.resource_mut::<PreparationSteps>().floor = true,
        PreparationStep::HeldItem => world.resource_mut::<PreparationSteps>().held_item = true,
    }
    let mut fixture = world.resource_mut::<BuildingArtStaticState>();
    fixture.phase = Phase::Seeded;
    fixture.evidence = None;
    fixture.stable_frames = 0;
    fixture.m6_resources = None;
    fixture.m6_foreground.clear();
    fixture.m6_final_evidence = None;
    fixture.m6_pending_evidence = None;
    fixture.m6_pending_frames = 0;
    Ok(())
}

fn soul_ids(world: &mut World) -> Vec<Entity> {
    let mut ids = world
        .query_filtered::<Entity, With<DamnedSoul>>()
        .iter(world)
        .collect::<Vec<_>>();
    ids.sort_by_key(|id| id.to_bits());
    ids
}

fn initialize(world: &mut World) -> Result<(), String> {
    let original_souls = soul_ids(world);
    if original_souls != world.resource::<BuildingArtStaticState>().actors {
        return Err("released context requires the unchanged fifteen core Souls".into());
    }
    let door = world
        .resource::<BuildingArtStaticState>()
        .specs
        .iter()
        .find(|spec| spec.kind == BuildingType::Door && spec.ordinal == 0)
        .cloned()
        .ok_or("released context Door specimen missing")?;
    let floor_grid = (door.anchor.0 + 2, door.anchor.1);
    // A real free cell next to SandPile, separated from its neighbouring owner.
    let carrier_grid = (11, 34);
    let map = world.resource::<WorldMap>();
    if [floor_grid, carrier_grid].into_iter().any(|grid| {
        !map.is_walkable(grid.0, grid.1) || map.has_building(grid) || map.has_stockpile(grid)
    }) || map.floor_entity(floor_grid).is_some()
    {
        return Err("released context requires its declared empty legal cells".into());
    }
    let walls = door
        .supports()
        .into_iter()
        .map(|grid| {
            map.building_entity(grid)
                .ok_or_else(|| "released context support wall missing".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    begin_preparation(world, PreparationStep::Floor)?;
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let floor = world.resource_scope(
        |world, handles: Mut<crate::plugins::startup::Building3dHandles>| {
            crate::systems::jobs::floor_construction::spawn_completed_floor_tile(
                &mut Commands::new(&mut queue, world),
                &handles,
                floor_grid,
            )
        },
    );
    queue.apply(world);
    // Context preparation is still unpublished. Only this just-created Floor
    // may introduce another completion effect; never settle foreign changes.
    let effects = world
        .query_filtered::<Entity, Or<(
            With<hw_visual::blueprint::BuildingBounceEffect>,
            With<hw_visual::blueprint::CompletionText>,
        )>>()
        .iter(world)
        .collect::<Vec<_>>();
    if effects != vec![floor] {
        return Err("released context encountered a foreign completion effect".into());
    }
    {
        let mut fixture = world.resource_mut::<BuildingArtStaticState>();
        fixture.completion_effects_settled = false;
    }
    super::super::settle_completion_effects(world);
    crate::systems::jobs::floor_construction::register_completed_floors(
        &mut world.resource_mut::<WorldMap>(),
        &[(floor, floor_grid)],
    );
    world.write_message(crate::entities::damned_soul::DamnedSoulSpawnEvent {
        position: WorldMap::grid_to_world(carrier_grid.0, carrier_grid.1),
        simulation_random_key: None,
    });
    world.insert_resource(Context {
        original_souls,
        floor,
        floor_grid,
        walls,
        carrier_grid,
        carrier: None,
        item: None,
        stable: 0,
        pending: None,
    });
    Ok(())
}

fn initialize_inventory(world: &mut World, state: &Context) -> Result<bool, String> {
    let ids = soul_ids(world);
    if ids
        .iter()
        .filter(|id| !state.original_souls.contains(id))
        .count()
        == 0
    {
        return Ok(false);
    }
    let extra = ids
        .iter()
        .copied()
        .filter(|id| !state.original_souls.contains(id))
        .collect::<Vec<_>>();
    if ids.len() != state.original_souls.len() + 1
        || extra.len() != 1
        || state.original_souls.iter().any(|id| !ids.contains(id))
    {
        return Err("released context Soul creation was missing or duplicated".into());
    }
    let carrier = extra[0];
    let point = WorldMap::grid_to_world(state.carrier_grid.0, state.carrier_grid.1);
    if world
        .get::<Transform>(carrier)
        .is_none_or(|pose| pose.translation.truncate() != point)
        || world
            .get::<Inventory>(carrier)
            .is_none_or(|inventory| inventory.0.is_some())
        || world
            .get::<AssignedTask>(carrier)
            .is_none_or(|task| !matches!(task, AssignedTask::None))
        || world
            .get::<hw_core::relationships::WorkingOn>(carrier)
            .is_some()
        || world
            .get::<hw_core::relationships::RestingIn>(carrier)
            .is_some()
    {
        return Err("released context cannot replace an existing Soul state or inventory".into());
    }
    begin_preparation(world, PreparationStep::HeldItem)?;
    let item = world
        .spawn((
            ResourceItem(ResourceType::Wood),
            Transform::from_translation(point.extend(0.6)),
            Visibility::Visible,
            Name::new("Released gallery held Wood"),
        ))
        .id();
    let mut inventory = Inventory::default();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    hw_soul_ai::soul_ai::execute::task_execution::common::pickup_item(
        &mut Commands::new(&mut queue, world),
        carrier,
        item,
        &mut inventory,
    );
    queue.apply(world);
    world.entity_mut(carrier).insert(inventory);
    let mut context = world.resource_mut::<Context>();
    context.carrier = Some(carrier);
    context.item = Some(item);
    Ok(true)
}

fn projected_anchor(world: &mut World, owner: Entity) -> Result<Value, String> {
    let pose = *world
        .get::<Transform>(owner)
        .ok_or("context owner pose missing")?;
    let cameras = world
        .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .map(|(camera, transform)| (camera.clone(), *transform))
        .collect::<Vec<_>>();
    let [(camera, transform)] = cameras.as_slice() else {
        return Err("context main camera count differs".into());
    };
    let windows = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .map(|window| (window.physical_size(), window.scale_factor()))
        .collect::<Vec<_>>();
    let [(extent, dpi)] = windows.as_slice() else {
        return Err("context primary window count differs".into());
    };
    let point = camera
        .world_to_viewport(transform, pose.translation)
        .map_err(|error| error.to_string())?
        * *dpi;
    if !point.is_finite()
        || point.x < 16.0
        || point.y < 16.0
        || point.x > extent.x as f32 - 16.0
        || point.y > extent.y as f32 - 16.0
    {
        return Err("context anchor lies outside the owned image".into());
    }
    Ok(
        json!({"entity": owner.to_bits(), "position": pose.translation.to_array(),
        "rotation": pose.rotation.to_array(), "scale": pose.scale.to_array(), "physical_anchor": point.to_array()}),
    )
}

fn visible_meshes(world: &mut World, owner: Entity) -> Result<Vec<Value>, String> {
    use hw_visual::visual3d::{ActorBillboard3d, Building3dVisual};
    let mut query = world.query::<(
        Entity,
        Option<&Building3dVisual>,
        Option<&ActorBillboard3d>,
        Option<&ChildOf>,
        &Mesh3d,
        &GlobalTransform,
        &ViewVisibility,
    )>();
    let mut result = Vec::new();
    for (entity, building, actor, parent, mesh, global, visible) in query.iter(world) {
        let actual_owner = building
            .map(|visual| visual.owner)
            .or_else(|| actor.map(|visual| visual.owner))
            .or_else(|| {
                parent
                    .and_then(|parent| world.get::<Building3dVisual>(parent.parent()))
                    .map(|visual| visual.owner)
            });
        if actual_owner != Some(owner) || !visible.get() {
            continue;
        }
        if result.len() >= 256 {
            return Err("context owned visual cap exceeded".into());
        }
        let aabb = world
            .resource::<Assets<Mesh>>()
            .get(&mesh.0)
            .and_then(MeshAabb::compute_aabb)
            .ok_or("context actual resident mesh bounds missing")?;
        let material = world
            .get::<MeshMaterial3d<hw_visual::TopDownStructuralMaterial>>(entity)
            .map(|material| format!("{:?}", material.0.id()));
        let standard = world
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .map(|material| format!("{:?}", material.0.id()));
        if world
            .get::<Building>(owner)
            .is_some_and(|building| building.kind == BuildingType::Wall)
            && world
                .get::<hw_visual::Wall3dPresentationState>(entity)
                .is_none_or(|state| state.mode != hw_visual::Wall3dPresentationMode::Production)
        {
            continue;
        }
        result.push(json!({"entity": entity.to_bits(), "mesh": format!("{:?}", mesh.0.id()),
            "structural_material": material, "standard_material": standard,
            "world_matrix": global.to_matrix().to_cols_array(), "aabb_min": aabb.min().to_array(),
            "aabb_max": aabb.max().to_array(), "layers": world.get::<RenderLayers>(entity).cloned().unwrap_or_default().iter().collect::<Vec<_>>(),
            "visible": true}));
    }
    result.sort_by_key(|row| row["entity"].as_u64());
    Ok(result)
}

fn snapshot_inner(world: &mut World, state: &Context) -> Result<Option<Value>, String> {
    let Some(carrier) = state.carrier else {
        return Ok(None);
    };
    let item = state.item.ok_or("context held item identity missing")?;
    let ids = soul_ids(world);
    if ids.len() != 16
        || state.original_souls.iter().any(|id| !ids.contains(id))
        || !ids.contains(&carrier)
        || world
            .get::<Inventory>(carrier)
            .is_none_or(|inventory| inventory.0 != Some(item))
        || world
            .get::<ResourceItem>(item)
            .is_none_or(|resource| resource.0 != ResourceType::Wood)
        || world.get::<Visibility>(item) != Some(&Visibility::Hidden)
        || world
            .get::<AssignedTask>(carrier)
            .is_none_or(|task| !matches!(task, AssignedTask::None))
        || world.resource::<WorldMap>().floor_entity(state.floor_grid) != Some(state.floor)
    {
        return Err("released context population/inventory/floor changed".into());
    }
    use crate::assets::wall_asset_set::{
        ProductionWallAssetPool, ProductionWallMaterialPool, WallAssetAuthority,
    };
    let wall_pool = world.resource::<ProductionWallAssetPool>();
    let Some(wall_assets) = wall_pool.resolved.as_ref() else {
        return Ok(None);
    };
    if wall_assets.identity.authority != WallAssetAuthority::ReleaseApproved {
        return Err("released context cannot use candidate Wall authority".into());
    }
    let wall_identity = json!({"asset_set_generation": wall_assets.identity.asset_set_generation,
        "authority": wall_assets.identity.authority, "manifest_sha256": wall_assets.identity.manifest_sha256});
    let wall_meshes = wall_assets
        .meshes
        .iter()
        .map(|mesh| format!("{:?}", mesh.id()))
        .collect::<Vec<_>>();
    let materials = world.resource::<ProductionWallMaterialPool>();
    if materials.identity.as_ref() != Some(&wall_assets.identity) {
        return Ok(None);
    }
    let Some(wall_material) = materials
        .complete
        .as_ref()
        .map(|material| format!("{:?}", material.id()))
    else {
        return Ok(None);
    };
    let handles = world.resource::<crate::plugins::startup::Building3dHandles>();
    let floor_mesh = format!("{:?}", handles.floor_mesh.id());
    let floor_material = format!("{:?}", handles.floor_material.id());
    let carrier_mesh = format!("{:?}", handles.soul_billboards.mesh.id());
    let support_grids = [
        (state.floor_grid.0 - 3, state.floor_grid.1),
        (state.floor_grid.0 - 1, state.floor_grid.1),
    ];
    if state.walls.len() != 2
        || support_grids
            .into_iter()
            .zip(&state.walls)
            .any(|(grid, owner)| world.resource::<WorldMap>().building_entity(grid) != Some(*owner))
    {
        return Err("released context lost its original wall supports".into());
    }
    let mut structures = Vec::new();
    for (kind, owner) in state
        .walls
        .iter()
        .map(|owner| (BuildingType::Wall, *owner))
        .chain([(BuildingType::Floor, state.floor)])
    {
        if world
            .get::<Building>(owner)
            .is_none_or(|building| building.kind != kind || building.is_provisional)
        {
            return Err("released context structure kind changed".into());
        }
        let meshes = visible_meshes(world, owner)?;
        if meshes.is_empty() {
            return Ok(None);
        }
        if meshes.iter().any(|mesh| {
            if kind == BuildingType::Wall {
                !wall_meshes.iter().any(|handle| mesh["mesh"] == *handle)
                    || mesh["structural_material"] != wall_material
            } else {
                mesh["mesh"] != floor_mesh || mesh["structural_material"] != floor_material
            }
        }) {
            return Err(
                "released context used a foreign or fallback structural mesh/material".into(),
            );
        }
        let mut row = projected_anchor(world, owner)?;
        row["kind"] = json!(format!("{kind:?}"));
        row["visible_meshes"] = json!(meshes);
        structures.push(row);
    }
    let meshes = visible_meshes(world, carrier)?;
    if meshes.is_empty() {
        return Ok(None);
    }
    if meshes.len() != 1
        || meshes[0]["mesh"] != carrier_mesh
        || meshes[0]["standard_material"].as_str().is_none()
    {
        return Err("released context carrier is not its production shared billboard".into());
    }
    let mut soul = projected_anchor(world, carrier)?;
    soul["visible_meshes"] = json!(meshes);
    let icons = world
        .query::<(
            Entity,
            &hw_visual::haul::CarryingItemVisual,
            &Sprite,
            &GlobalTransform,
            &ViewVisibility,
        )>()
        .iter(world)
        .filter(|(_, visual, _, _, _)| visual.worker == carrier)
        .map(|(entity, _, sprite, global, visible)| {
            (entity, sprite.clone(), *global, visible.get())
        })
        .collect::<Vec<_>>();
    if icons.is_empty() {
        return Ok(None);
    }
    let [(entity, sprite, global, visible)] = icons.as_slice() else {
        return Err("context carrying icon duplicated".into());
    };
    if !visible
        || sprite.image
            != world
                .resource::<hw_visual::handles::MaterialIconHandles>()
                .wood_small
        || sprite.custom_size != Some(Vec2::splat(hw_visual::haul::CARRIED_ITEM_ICON_SIZE))
        || world
            .get::<hw_core::visual_mirror::logistics::InventoryItemVisual>(carrier)
            .is_none_or(|mirror| mirror.resource_type != Some(ResourceType::Wood))
    {
        return Err("context icon disagrees with the actual held Wood".into());
    }
    let mut icon = projected_anchor(world, *entity)?;
    icon["image"] = json!(format!("{:?}", sprite.image.id()));
    icon["size"] = json!(sprite.custom_size.map(|size| size.to_array()));
    icon["color"] = json!(sprite.color.to_srgba().to_f32_array());
    icon["world_matrix"] = json!(global.to_matrix().to_cols_array());
    icon["worker"] = json!(carrier.to_bits());
    icon["item"] = json!(item.to_bits());
    Ok(Some(
        json!({"scope": "seeded-release-storyboard-context", "projection_scope": "context-anchors-only; full appearance independently inspected",
        "core_souls": state.original_souls.iter().map(|id| id.to_bits()).collect::<Vec<_>>(), "total_souls": ids.len(),
        "structures": structures, "wall_identity": wall_identity, "wall_meshes": wall_meshes,
        "wall_material": wall_material, "floor_mesh": floor_mesh, "floor_material": floor_material,
        "carrier_mesh": carrier_mesh, "carrier_grid": state.carrier_grid, "floor_grid": state.floor_grid,
        "carrier": soul, "held_resource": "Wood", "item": item.to_bits(), "item_hidden": true, "icon": icon}),
    ))
}

pub(super) fn prepare(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Context>() {
        initialize(world)?;
        return Ok(false);
    }
    let state = world.resource::<Context>().clone();
    if state.carrier.is_none() {
        initialize_inventory(world, &state)?;
        return Ok(false);
    }
    let Some(value) = snapshot_inner(world, &state)? else {
        return Ok(false);
    };
    let mut state = world.resource_mut::<Context>();
    if state.pending.as_ref() == Some(&value) {
        state.stable += 1;
    } else {
        state.pending = Some(value);
        state.stable = 0;
    }
    Ok(state.stable >= 60)
}

pub(super) fn snapshot(world: &mut World) -> Result<Value, String> {
    let state = world
        .get_resource::<Context>()
        .cloned()
        .ok_or("released context not prepared")?;
    snapshot_inner(world, &state)?.ok_or("released context is no longer visible".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (World, std::path::PathBuf) {
        let mut world = World::new();
        let root = std::env::temp_dir().join(format!(
            "released-context-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut session = crate::assets::building_asset_set::release_gallery::test_session();
        session.status_path = root.join("probe.json");
        world.insert_resource(GalleryCaptureSession::Released(session));
        world.init_resource::<CaptureState>();
        let owner = world.spawn_empty().id();
        world.insert_resource(BuildingArtStaticState {
            phase: Phase::Ready,
            actors: vec![owner],
            owners: vec![owner],
            evidence: Some(json!({"old": true})),
            stable_frames: 60,
            m6_resources: Some(ResourceEvidence {
                resident_mesh_count: 1,
                resident_structural_material_count: 1,
                resident_image_count: 1,
                resident_image_cpu_bytes: 4,
                structural_material_shallow_bytes: 1,
            }),
            m6_pending_evidence: Some(json!({"old": true})),
            m6_pending_frames: 30,
            m6_final_evidence: Some(json!({"old": true})),
            ..default()
        });
        (world, root)
    }

    #[test]
    fn unpublished_context_invalidates_old_resource_baseline_once_then_reobserves() {
        let (mut world, root) = world();
        let original = world.resource::<BuildingArtStaticState>().actors.clone();
        begin_preparation(&mut world, PreparationStep::Floor).unwrap();
        {
            let state = world.resource::<BuildingArtStaticState>();
            assert!(matches!(state.phase, Phase::Seeded));
            assert!(
                state.evidence.is_none()
                    && state.m6_resources.is_none()
                    && state.m6_final_evidence.is_none()
                    && state.m6_pending_evidence.is_none()
            );
            assert_eq!(state.m6_pending_frames, 0);
            assert_eq!(state.actors, original);
            assert_eq!(state.owners, original);
        }
        assert!(begin_preparation(&mut world, PreparationStep::Floor).is_err());
        // Different resources must earn the unchanged thirty-observation gate.
        let changed = json!({"resources": {"resident_image_count": 2}});
        {
            let mut state = world.resource_mut::<BuildingArtStaticState>();
            for _ in 0..29 {
                assert!(!m6_readiness(&mut state, Some(&changed)));
            }
            assert!(m6_readiness(&mut state, Some(&changed)));
        }
        world.insert_resource(Context {
            original_souls: original.clone(),
            floor: original[0],
            floor_grid: (10, 43),
            walls: Vec::new(),
            carrier_grid: (11, 34),
            carrier: None,
            item: None,
            stable: 0,
            pending: None,
        });
        begin_preparation(&mut world, PreparationStep::HeldItem).unwrap();
        assert!(begin_preparation(&mut world, PreparationStep::HeldItem).is_err());
        assert_eq!(world.resource::<BuildingArtStaticState>().actors, original);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn published_or_wrong_profile_context_never_invalidates_existing_evidence() {
        for name in ["probe.json", "capture-ack.json", "final.json"] {
            let (mut world, root) = world();
            std::fs::write(root.join(name), b"{}").unwrap();
            assert!(begin_preparation(&mut world, PreparationStep::Floor).is_err());
            assert!(
                world
                    .resource::<BuildingArtStaticState>()
                    .m6_resources
                    .is_some()
            );
            std::fs::remove_file(root.join(name)).unwrap();
            std::fs::remove_dir(root).unwrap();
        }
        let (mut world, root) = world();
        world.resource_mut::<CaptureState>().ready = Some(json!({"published": true}));
        assert!(begin_preparation(&mut world, PreparationStep::Floor).is_err());
        assert!(
            world
                .resource::<BuildingArtStaticState>()
                .m6_resources
                .is_some()
        );
        world.resource_mut::<CaptureState>().ready = None;
        world.insert_resource(GalleryCaptureSession::Group(
            crate::assets::building_asset_set::group_art::test_session(),
        ));
        assert!(begin_preparation(&mut world, PreparationStep::Floor).is_err());
        assert!(
            world
                .resource::<BuildingArtStaticState>()
                .m6_resources
                .is_some()
        );
        std::fs::remove_dir(root).unwrap();
    }
}
