//! M2-specific domain/overlap observations for the seeded paused art comparison.
//! No input, presentation repair, numeric/art decision or ordinary lifecycle claim.
use super::*;
use bevy::ecs::relationship::RelationshipTarget;
use hw_core::{relationships::WorkingOn, soul::DamnedSoul};
use hw_jobs::{ActiveTaskIdentity, AssignedTask, RefinePhase};

/// Only exact admitted dependencies, not the asynchronous global asset pool.
/// A missing owned dependency still prevents readiness and fails after ready.
pub(super) fn resident_assets(
    world: &World,
    set: &crate::assets::building_asset_set::BuildingAssetDescriptor,
) -> Option<Value> {
    let mut meshes = Vec::new();
    for role in set.manifest.identity.kind.mesh_roles() {
        let handle = set.mesh(role)?;
        world.resource::<Assets<Mesh>>().get(handle)?;
        meshes.push(json!({"role": role, "handle": format!("{:?}", handle.id())}));
    }
    let mut images = Vec::new();
    for role in set.manifest.identity.kind.image_roles() {
        let handle = set.image(role)?;
        world.resource::<Assets<Image>>().get(handle)?;
        images.push(json!({"role": role, "handle": format!("{:?}", handle.id())}));
    }
    Some(
        json!({"scope": "exact-m2-resident-dependencies", "meshes": meshes, "images": images,
        "global_pool_evidence": false}),
    )
}

fn tank_state(world: &mut World, owner: Entity, expected: usize) -> Result<Value, String> {
    let stockpile = world
        .get::<hw_logistics::Stockpile>(owner)
        .ok_or("M2 Tank stockpile missing")?;
    if stockpile.capacity != 50
        || stockpile.resource_type != Some(hw_logistics::ResourceType::Water)
    {
        return Err("M2 Tank storage contract differs".into());
    }
    let mut inverse = world
        .get::<StoredItems>(owner)
        .map(|items| items.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut actual = world
        .query::<(
            Entity,
            &hw_core::relationships::StoredIn,
            Option<&hw_logistics::ResourceItem>,
        )>()
        .iter(world)
        .filter(|(_, stored, _)| stored.0 == owner)
        .map(|(entity, _, item)| {
            if item.is_none_or(|item| item.0 != hw_logistics::ResourceType::Water) {
                return Err("M2 Tank stored item is not Water".to_string());
            }
            Ok(entity)
        })
        .collect::<Result<Vec<_>, _>>()?;
    inverse.sort_by_key(|entity| entity.to_bits());
    actual.sort_by_key(|entity| entity.to_bits());
    if actual.len() != expected || actual != inverse {
        return Err("M2 Tank actual water/inverse differs".into());
    }
    Ok(json!({"stored_water": actual.len(), "capacity": 50,
        "water_items": actual.iter().map(|entity| entity.to_bits()).collect::<Vec<_>>()}))
}

fn mixer_workers(
    world: &mut World,
    owner: Entity,
    expected: Option<Entity>,
) -> Result<Vec<Value>, String> {
    let mut inverse = world
        .get::<TaskWorkers>(owner)
        .map(|workers| workers.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut forward = world
        .query::<(Entity, &WorkingOn)>()
        .iter(world)
        .filter(|(_, target)| target.0 == owner)
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    let mut assigned = world
        .query::<(Entity, &AssignedTask)>()
        .iter(world)
        .filter_map(|(entity, task)| match task {
            AssignedTask::Refine(data) if data.mixer == owner => Some(entity),
            _ => None,
        })
        .collect::<Vec<_>>();
    for set in [&mut inverse, &mut forward, &mut assigned] {
        set.sort_by_key(|entity| entity.to_bits());
    }
    let expected = expected.into_iter().collect::<Vec<_>>();
    if inverse != expected || forward != expected || assigned != expected {
        return Err("M2 Mixer worker sets differ".into());
    }
    let mut records = Vec::new();
    for actor in expected {
        let Some(AssignedTask::Refine(data)) = world.get::<AssignedTask>(actor) else {
            return Err("M2 worker task missing".into());
        };
        let identity = world
            .get::<ActiveTaskIdentity>(actor)
            .ok_or("M2 worker identity missing")?;
        if !matches!(data.phase, RefinePhase::Refining { progress: 0.0 })
            || identity.assignment_entity != owner
            || identity.current_target_entity != owner
            || identity.current_work_type != hw_jobs::WorkType::Refine
            || !identity.matches_working_on(Some(owner))
            || world.get::<DamnedSoul>(actor).is_none()
        {
            return Err("M2 Mixer actual Refining/identity differs".into());
        }
        records.push(json!({"entity": actor.to_bits(), "mixer": owner.to_bits(), "working_on": owner.to_bits(),
            "assignment": identity.assignment_entity.to_bits(), "target": identity.current_target_entity.to_bits(),
            "work_type": "Refine", "phase": "Refining", "progress": 0.0}));
    }
    Ok(records)
}

pub(super) fn part_state(
    spec: &Specimen,
    name: &str,
    original: &Transform,
) -> (Transform, bool, Color) {
    let mut local = *original;
    let water = spec.kind == BuildingType::Tank && name == "water";
    if water {
        local.translation.y = if spec.water_count() == 50 { 22.0 } else { 12.0 };
    }
    (
        local,
        !water || spec.water_count() > 0,
        if water && spec.water_count() == 25 {
            Color::srgb(0.58, 0.70, 0.73)
        } else {
            Color::WHITE
        },
    )
}

pub(super) fn snapshot(
    world: &mut World,
    specs: &[Specimen],
    owners: &[Entity],
) -> Result<Option<Value>, String> {
    let kind = world.resource::<BuildingArtSession>().identity.kind;
    let workers = usize::from(kind == BuildingAssetKind::MudMixer) * 2;
    let actors = world.resource::<BuildingArtStaticState>().actors.clone();
    if specs.len() != 4
        || owners.len() != 4
        || actors.len() != 8 + workers
        || world.query::<&DamnedSoul>().iter(world).count() != 8 + workers
    {
        return Err("M2 context actor/owner inventory differs".into());
    }
    let mut records = Vec::new();
    for (ordinal, (spec, owner)) in specs.iter().zip(owners).enumerate() {
        if spec.ordinal != ordinal
            || crate::systems::visual::building_presentation::asset_kind(spec.kind) != Some(kind)
        {
            return Err("M2 context kind/ordinal differs".into());
        }
        let state = if kind == BuildingAssetKind::Tank {
            tank_state(world, *owner, spec.water_count())?
        } else {
            let active = ordinal >= 2;
            if world
                .get::<MudMixerVisualState>(*owner)
                .is_none_or(|value| value.is_active != active)
            {
                return Ok(None);
            }
            let actual = mixer_workers(world, *owner, active.then(|| actors[ordinal - 2]))?;
            json!({"refining": active, "workers": actual})
        };
        let mut souls = Vec::new();
        for (index, side) in [-1.0, 1.0].into_iter().enumerate() {
            let actor = actors[workers + ordinal * 2 + index];
            let pose = world
                .get::<Transform>(actor)
                .ok_or("M2 idle Soul pose missing")?;
            let point = spec.center + Vec2::Y * (side * 12.0);
            if pose.translation.truncate() != point
                || !matches!(world.get::<AssignedTask>(actor), Some(AssignedTask::None))
                || world.get::<WorkingOn>(actor).is_some()
                || world
                    .get::<hw_core::relationships::RestingIn>(actor)
                    .is_some()
                || world.get::<ActiveTaskIdentity>(actor).is_some()
            {
                return Err("M2 overlap Soul pose/task/relation differs".into());
            }
            let meshes = super::m5_context::mesh_rows(world, actor, true);
            if meshes.len() != 1 || meshes[0]["visible"] != true {
                return Ok(None);
            }
            let rendered = &meshes[0]["root_pose"];
            if rendered["translation"]
                != json!([point.x, hw_core::constants::TILE_SIZE * 0.55, -point.y])
                || !matches!(rendered["scale"][0].as_f64(), Some(1.0 | -1.0))
                || rendered["scale"][1] != 1.0
                || rendered["scale"][2] != 1.0
            {
                return Err("M2 overlap Soul render pose differs".into());
            }
            souls.push(json!({"entity": actor.to_bits(), "side": side, "ground": point.to_array(), "meshes": meshes}));
        }
        records.push(
            json!({"owner": owner.to_bits(), "ordinal": ordinal, "state": state, "souls": souls}),
        );
    }
    Ok(Some(
        json!({"idle_souls": 8, "source_workers": workers, "records": records,
        "ordinary_input_evidence": false, "lifecycle_evidence": false}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m2_residency_ignores_unrelated_assets_but_rejects_missing_owned_dependencies() {
        let mut pool = BuildingAssetPool::default();
        let set = pool.install_presentation_fixture(
            BuildingAssetKind::Tank,
            1,
            crate::assets::building_asset_set::BuildingAssetAuthority::ArtPreview,
        );
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<Image>>();
        for role in set.manifest.identity.kind.mesh_roles() {
            world
                .resource_mut::<Assets<Mesh>>()
                .insert(
                    set.mesh(role).unwrap().id(),
                    Cuboid::new(1.0, 1.0, 1.0).into(),
                )
                .unwrap();
        }
        for role in set.manifest.identity.kind.image_roles() {
            world
                .resource_mut::<Assets<Image>>()
                .insert(set.image(role).unwrap().id(), Image::default())
                .unwrap();
        }
        let initial = resident_assets(&world, &set).unwrap();
        let unrelated = world.resource_mut::<Assets<Image>>().add(Image::default());
        assert_eq!(resident_assets(&world, &set), Some(initial.clone()));
        world.resource_mut::<Assets<Image>>().remove(unrelated.id());
        assert_eq!(resident_assets(&world, &set), Some(initial.clone()));
        for role in set.manifest.identity.kind.image_roles() {
            let handle = set.image(role).unwrap();
            let asset = world
                .resource_mut::<Assets<Image>>()
                .remove(handle.id())
                .unwrap();
            assert!(resident_assets(&world, &set).is_none());
            assert!(world.resource::<Assets<Image>>().get(handle).is_none());
            world
                .resource_mut::<Assets<Image>>()
                .insert(handle.id(), asset)
                .unwrap();
            assert_eq!(resident_assets(&world, &set), Some(initial.clone()));
        }
        let handle = set.mesh("body").unwrap();
        world.resource_mut::<Assets<Mesh>>().remove(handle.id());
        assert!(resident_assets(&world, &set).is_none());
    }

    #[test]
    fn m2_tank_rejects_same_count_foreign_resource_without_repair() {
        use hw_core::relationships::StoredIn;
        use hw_logistics::{ResourceItem, ResourceType, Stockpile};
        let mut world = World::new();
        let owner = world
            .spawn(Stockpile {
                capacity: 50,
                resource_type: Some(ResourceType::Water),
            })
            .id();
        let item = world
            .spawn((ResourceItem(ResourceType::Water), StoredIn(owner)))
            .id();
        assert!(tank_state(&mut world, owner, 1).is_ok());
        world.get_mut::<ResourceItem>(item).unwrap().0 = ResourceType::Rock;
        assert!(tank_state(&mut world, owner, 1).is_err());
        assert_eq!(
            world.get::<ResourceItem>(item).unwrap().0,
            ResourceType::Rock
        );
        world.entity_mut(item).remove::<ResourceItem>();
        assert!(tank_state(&mut world, owner, 1).is_err());
        world
            .entity_mut(item)
            .insert(ResourceItem(ResourceType::Water));
        let foreign = world.spawn_empty().id();
        world.entity_mut(item).insert(StoredIn(foreign));
        assert!(tank_state(&mut world, owner, 1).is_err());
        assert_eq!(world.get::<StoredIn>(item).unwrap().0, foreign);
    }

    #[test]
    fn m2_mixer_rejects_foreign_worker_phase_and_task_identity_without_repair() {
        use hw_jobs::{RefineData, WorkType};
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let foreign = world.spawn_empty().id();
        let task = AssignedTask::Refine(RefineData {
            mixer: owner,
            phase: RefinePhase::Refining { progress: 0.0 },
        });
        let actor = world
            .spawn((
                DamnedSoul::default(),
                task.clone(),
                WorkingOn(owner),
                ActiveTaskIdentity::new(owner, owner, WorkType::Refine),
            ))
            .id();
        let initial = mixer_workers(&mut world, owner, Some(actor)).unwrap();
        world.entity_mut(actor).insert(WorkingOn(foreign));
        let impostor = world.spawn(WorkingOn(owner)).id();
        assert!(mixer_workers(&mut world, owner, Some(actor)).is_err());
        assert_eq!(world.get::<WorkingOn>(actor).unwrap().0, foreign);
        world.despawn(impostor);
        world.entity_mut(actor).insert(WorkingOn(owner));
        world.entity_mut(actor).insert(AssignedTask::None);
        assert!(mixer_workers(&mut world, owner, Some(actor)).is_err());
        world.entity_mut(actor).insert(task);
        world
            .get_mut::<ActiveTaskIdentity>(actor)
            .unwrap()
            .current_target_entity = foreign;
        assert!(mixer_workers(&mut world, owner, Some(actor)).is_err());
        assert_eq!(
            world
                .get::<ActiveTaskIdentity>(actor)
                .unwrap()
                .current_target_entity,
            foreign
        );
        world
            .entity_mut(actor)
            .insert(ActiveTaskIdentity::new(owner, owner, WorkType::Refine));
        assert_eq!(
            mixer_workers(&mut world, owner, Some(actor)).unwrap(),
            initial
        );
        assert!(mixer_workers(&mut world, owner, None).is_err());
        if let Some(mut task) = world.get_mut::<AssignedTask>(actor)
            && let AssignedTask::Refine(data) = &mut *task
        {
            data.phase = RefinePhase::Refining { progress: 0.5 };
        }
        assert!(mixer_workers(&mut world, owner, Some(actor)).is_err());
    }

    #[test]
    fn m2_context_only_allows_known_water_state_differences() {
        let specs = layout::layout(super::super::super::PerfScenarioSize::Small);
        let original = Transform::from_xyz(3.0, 4.0, 5.0).with_scale(Vec3::splat(2.0));
        for spec in specs
            .iter()
            .filter(|spec| matches!(spec.kind, BuildingType::Tank | BuildingType::MudMixer))
        {
            for name in ["body", "water", "rotor"] {
                let (actual, visible, color) = part_state(spec, name, &original);
                let water = spec.kind == BuildingType::Tank && name == "water";
                assert_eq!(actual.translation.x, original.translation.x);
                assert_eq!(actual.translation.z, original.translation.z);
                assert_eq!(actual.scale, original.scale);
                assert_eq!(actual.rotation, original.rotation);
                assert_eq!(visible, !water || spec.water_count() > 0);
                assert_eq!(
                    color,
                    if water && spec.water_count() == 25 {
                        Color::srgb(0.58, 0.70, 0.73)
                    } else {
                        Color::WHITE
                    }
                );
                assert_eq!(
                    actual.translation.y,
                    if water {
                        if spec.water_count() == 50 { 22.0 } else { 12.0 }
                    } else {
                        original.translation.y
                    }
                );
            }
        }
    }
}
