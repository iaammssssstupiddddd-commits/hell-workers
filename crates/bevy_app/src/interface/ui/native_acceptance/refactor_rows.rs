//! Initial fixture and read-only evidence for task labels and Soul row updates.
use super::*;
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use hw_core::relationships::CommandedBy;
use hw_jobs::*;
use hw_ui::list::SoulRowNodes;

#[derive(Component)]
struct FixtureSoul(&'static str);

pub(super) fn prepare(world: &mut World) {
    let mut souls: Vec<_> = world
        .query_filtered::<Entity, With<DamnedSoul>>()
        .iter(world)
        .collect();
    souls.sort();
    assert!(
        souls.len() >= 3,
        "row acceptance needs three production Souls"
    );
    let familiars: Vec<_> = world
        .query_filtered::<Entity, With<crate::entities::familiar::Familiar>>()
        .iter(world)
        .collect();
    let familiar = familiars[0];
    for entity in &familiars {
        world.entity_mut(*entity).insert(SectionFolded);
    }
    world.entity_mut(familiar).remove::<SectionFolded>();
    for mut policy in world
        .query::<&mut hw_core::familiar::FamiliarPolicy>()
        .iter_mut(world)
    {
        policy.set_all_allowed(false);
    }
    let target = world.spawn_empty().id();
    let tasks = [
        (
            "deconstruct",
            "RefactorDeconstruct",
            AssignedTask::Deconstruct(DeconstructData {
                order: target,
                target,
                phase: DeconstructPhase::GoingToTarget,
            }),
        ),
        (
            "power",
            "RefactorPower",
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: target,
                tile_pos: Vec2::ZERO,
                phase: GeneratePowerPhase::Generating,
            }),
        ),
        (
            "bucket",
            "RefactorBucket",
            AssignedTask::BucketTransport(BucketTransportData {
                bucket: target,
                source: BucketTransportSource::River,
                destination: BucketTransportDestination::Mixer(target),
                amount: 0,
                phase: BucketTransportPhase::GoingToBucket,
            }),
        ),
    ];
    for entity in &souls {
        world.entity_mut(*entity).remove::<CommandedBy>();
        world.get_mut::<Transform>(*entity).unwrap().translation.x = 2500.0;
    }
    for (index, (key, name, task)) in tasks.into_iter().enumerate() {
        let entity = souls[index];
        world
            .entity_mut(entity)
            .insert((FixtureSoul(key), task, CommandedBy(familiar)));
        world.get_mut::<SoulIdentity>(entity).unwrap().name = name.into();
        world.get_mut::<DamnedSoul>(entity).unwrap().fatigue = 0.25;
        let mut transform = world.get_mut::<Transform>(entity).unwrap();
        transform.translation.x = -250.0 + index as f32 * 150.0;
        transform.translation.y = 150.0;
    }
    for mut transform in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
    }
    for entity in world
        .query_filtered::<Entity, With<UnassignedSoulSection>>()
        .iter(world)
        .collect::<Vec<_>>()
    {
        world.entity_mut(entity).insert(UnassignedFolded);
    }
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World, viewport: Vec2) -> Value {
    let mut souls = serde_json::Map::new();
    let camera = world
        .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .next();
    let rows: Vec<_> = world
        .iter_entities()
        .filter_map(|entity| {
            Some((
                entity.id(),
                entity.get::<SoulListItem>()?.0,
                *entity.get::<SoulRowNodes>()?,
            ))
        })
        .collect();
    for entity in world
        .iter_entities()
        .filter(|entity| entity.contains::<FixtureSoul>())
    {
        let key = entity.get::<FixtureSoul>().unwrap().0;
        let row = rows.iter().find(|(_, soul, _)| *soul == entity.id());
        let leaf = |node: Entity| {
            json!({
                "entity": node.to_bits(), "text": world.get::<Text>(node).map(|text| &text.0),
                "rect": visible_rect(world, node, viewport),
                "color": world.get::<TextColor>(node).map(|color| color.0.to_srgba().to_f32_array()),
                "icon": world.get::<ImageNode>(node).map(|image| format!("{:?}", image.image.id())),
            })
        };
        let position = entity.get::<GlobalTransform>().and_then(|transform| {
            camera.and_then(|(camera, global)| {
                camera
                    .world_to_viewport(global, transform.translation())
                    .ok()
            })
        });
        souls.insert(
            key.into(),
            json!({
                "entity": entity.id().to_bits(), "name": entity.get::<SoulIdentity>().unwrap().name,
                "fatigue": entity.get::<DamnedSoul>().unwrap().fatigue,
                "familiar": entity.get::<CommandedBy>().map(|owner| owner.0.to_bits()),
                "world_point": position.map(|position| [position.x, position.y]),
                "row": row.map(|(row, _, nodes)| json!({
                    "entity": row.to_bits(), "rect": visible_rect(world, *row, viewport),
                    "name": leaf(nodes.name_text), "label": leaf(nodes.task_label),
                    "icon": leaf(nodes.task_icon), "stress": leaf(nodes.stress_text),
                    "fatigue": leaf(nodes.fatigue_text), "dream": leaf(nodes.dream_text),
                    "gender": leaf(nodes.gender_icon),
                })),
            }),
        );
    }
    let task = world.resource::<InfoPanelNodes>().soul.task.map(|node| json!({
        "text": world.get::<Text>(node).map(|text| &text.0), "rect": visible_rect(world, node, viewport),
    }));
    json!({"souls": souls, "detail_task": task,
        "hovered": world.resource::<crate::interface::selection::HoveredEntity>().0.map(Entity::to_bits)})
}
