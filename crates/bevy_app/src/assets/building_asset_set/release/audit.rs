//! Passive normal-startup diagnostics. Never selects inputs or grants authority.
use super::ReleaseBinding;
use crate::assets::building_asset_set::{BuildingAssetKind, BuildingAssetPool};
use crate::systems::visual::building_presentation::{EquipmentRoot, asset_kind};
use bevy::prelude::*;
use hw_infra::lighting::digest_hex;
use hw_jobs::Building;
use hw_visual::{Building3dVisual, TopDownStructuralMaterial};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const MARKER: &str = "BUILDING_DEFAULT_RELEASE_AUDIT";
const OWNER_LIMIT: usize = 256;

#[derive(Resource)]
struct Audit {
    bindings: Vec<ReleaseBinding>,
    inventory_sha256: String,
    selected: bool,
    ready_frames: u32,
    finished: bool,
}

pub(super) fn configure(
    app: &mut App,
    bindings: &[ReleaseBinding],
    inventory: &str,
    implicit: bool,
) {
    if !implicit || bindings.is_empty() {
        return;
    }
    app.insert_resource(Audit {
        bindings: bindings.to_vec(),
        inventory_sha256: digest_hex(&Sha256::digest(inventory.as_bytes()).into()),
        selected: false,
        ready_frames: 0,
        finished: false,
    })
    .add_systems(Last, observe);
}

fn activation_matches(
    expected: &super::super::BuildingAssetSetIdentity,
    active: Option<&super::super::BuildingAssetSetIdentity>,
    descriptor: Option<&super::super::BuildingAssetSetIdentity>,
    pending: bool,
    failed: bool,
    resident: bool,
) -> bool {
    active == Some(expected) && descriptor == Some(expected) && !pending && !failed && resident
}

fn pool_rows(world: &World, bindings: &[ReleaseBinding]) -> (bool, bool, Vec<Value>) {
    let pool = world.resource::<BuildingAssetPool>();
    let meshes = world.get_resource::<Assets<Mesh>>();
    let images = world.get_resource::<Assets<Image>>();
    let mut all_ready = !bindings.is_empty();
    let mut failed = false;
    let rows = bindings.iter().map(|binding| {
        let kind = binding.identity.kind;
        let descriptor = pool.descriptor(kind);
        let mesh_rows: Vec<_> = kind.mesh_roles().iter().map(|role| {
            let handle = descriptor.and_then(|set| set.mesh(role));
            let resident = handle.is_some_and(|h| meshes.is_some_and(|m| m.get(h).is_some()));
            json!({"role": role, "handle": handle.map(|h| format!("{:?}", h.id())), "resident": resident})
        }).collect();
        let image_rows: Vec<_> = kind.image_roles().iter().map(|role| {
            let handle = descriptor.and_then(|set| set.image(role));
            let resident = handle.is_some_and(|h| images.is_some_and(|m| m.get(h).is_some()));
            json!({"role": role, "handle": handle.map(|h| format!("{:?}", h.id())), "resident": resident})
        }).collect();
        let failure = pool.failure(kind);
        failed |= failure.is_some();
        all_ready &= activation_matches(&binding.identity, pool.active(kind),
            descriptor.map(|set| &set.manifest.identity), pool.pending(kind).is_some(), failure.is_some(),
            mesh_rows.iter().chain(&image_rows).all(|row| row["resident"] == true));
        json!({"expected": binding.identity, "locator": binding.locator,
            "active": pool.active(kind), "pending": pool.pending(kind),
            "failure": failure.map(|f| json!({"identity": f.identity, "reason": f.reason})),
            "descriptor_identity": descriptor.map(|set| &set.manifest.identity),
            "meshes": mesh_rows, "images": image_rows})
    }).collect();
    (all_ready, failed, rows)
}

struct SpriteTarget {
    owner: Entity,
    entity: Entity,
    kind: BuildingAssetKind,
    child: bool,
}

fn consumers(world: &mut World, bindings: &[ReleaseBinding]) -> Result<Value, String> {
    let mut owners = Vec::new();
    let mut targets = Vec::new();
    for (entity, building, children) in world
        .query::<(Entity, &Building, Option<&Children>)>()
        .iter(world)
    {
        let Some(kind) = asset_kind(building.kind)
            .filter(|kind| bindings.iter().any(|b| b.identity.kind == *kind))
        else {
            continue;
        };
        if owners.len() == OWNER_LIMIT {
            return Err("startup owner cap exceeded".into());
        }
        let mut owner_targets = Vec::new();
        if kind.mesh_roles().is_empty() {
            if world.get::<Sprite>(entity).is_some() {
                owner_targets.push(SpriteTarget {
                    owner: entity,
                    entity,
                    kind,
                    child: false,
                });
            }
            for child in children.into_iter().flat_map(|children| children.iter()) {
                if world
                    .get::<hw_visual::layer::VisualLayerKind>(child)
                    .is_some()
                {
                    // Match the actual preview publisher, even if this target's
                    // Sprite is missing. The receiver must reject that gap.
                    owner_targets.push(SpriteTarget {
                        owner: entity,
                        entity: child,
                        kind,
                        child: true,
                    });
                }
            }
        }
        if owner_targets.len() > 16 || targets.len() + owner_targets.len() > OWNER_LIMIT {
            return Err("startup Sprite target cap exceeded".into());
        }
        let expected: Vec<_> = owner_targets
            .iter()
            .map(|target| {
                json!({"entity": format!("{:?}", target.entity),
            "role": if target.child { "child" } else { "owner" }})
            })
            .collect();
        owners.push(json!({"entity": format!("{entity:?}"), "kind": kind, "provisional": building.is_provisional,
            "sprite_targets": expected}));
        targets.extend(owner_targets);
    }
    let mut roots = Vec::new();
    for (entity, visual, root, children) in world
        .query::<(Entity, &Building3dVisual, &EquipmentRoot, Option<&Children>)>()
        .iter(world)
    {
        let owner_kind = world
            .get::<Building>(visual.owner)
            .and_then(|building| asset_kind(building.kind));
        let observed_kind = owner_kind.or_else(|| root.observed_identity().map(|id| id.kind));
        let Some(binding) = bindings
            .iter()
            .find(|b| Some(b.identity.kind) == observed_kind)
        else {
            continue;
        };
        let parts: Vec<_> = children.into_iter().flat_map(|c| c.iter()).take(17).map(|part| {
            let mesh = world.get::<Mesh3d>(part);
            let material = world.get::<MeshMaterial3d<TopDownStructuralMaterial>>(part)
                .and_then(|h| world.get_resource::<Assets<TopDownStructuralMaterial>>()?.get(&h.0));
            let image = material.and_then(|m| m.base.base_color_texture.as_ref());
            json!({"entity": format!("{part:?}"), "name": world.get::<Name>(part).map(Name::as_str),
                "parent": world.get::<ChildOf>(part).map(|p| format!("{:?}", p.parent())),
                "mesh": mesh.map(|h| format!("{:?}", h.0.id())),
                "mesh_resident": mesh.is_some_and(|h| world.get_resource::<Assets<Mesh>>().is_some_and(|m| m.get(&h.0).is_some())),
                "albedo": image.map(|h| format!("{:?}", h.id())),
                "albedo_resident": image.is_some_and(|h| world.get_resource::<Assets<Image>>().is_some_and(|m| m.get(h).is_some())),
                "visible": world.get::<ViewVisibility>(part).is_some_and(|v| v.get())})
        }).collect();
        if parts.len() > 16 || roots.len() == OWNER_LIMIT {
            return Err("startup consumer cap exceeded".into());
        }
        roots.push(
            json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", visual.owner),
            "kind": binding.identity.kind, "owner_kind": owner_kind, "owner_exists": world.get_entity(visual.owner).is_ok(),
            "identity": root.observed_identity(), "expected": binding.identity, "parts": parts}),
        );
    }
    let mut sprites = Vec::new();
    for target in targets {
        let Some(sprite) = world.get::<Sprite>(target.entity) else {
            continue;
        };
        let descriptor = world
            .resource::<BuildingAssetPool>()
            .descriptor(target.kind);
        let matches = descriptor.is_some_and(|set| {
            target
                .kind
                .image_roles()
                .iter()
                .filter(|role| role.starts_with("world"))
                .any(|role| set.image(role).is_some_and(|h| h.id() == sprite.image.id()))
        });
        sprites.push(json!({"entity": format!("{:?}", target.entity), "owner": format!("{:?}", target.owner),
            "parent": world.get::<ChildOf>(target.entity).map(|parent| format!("{:?}", parent.parent())),
            "layer_kind": world.get::<hw_visual::layer::VisualLayerKind>(target.entity).map(|kind| format!("{kind:?}")),
            "target_role": if target.child { "child" } else { "owner" }, "kind": target.kind,
            "image": format!("{:?}", sprite.image.id()), "matches_released_world_image": matches,
            "resident": world.get_resource::<Assets<Image>>().is_some_and(|m| m.get(&sprite.image).is_some()),
            "visible": world.get::<ViewVisibility>(target.entity).is_some_and(|v| v.get())}));
    }
    Ok(json!({"owners": owners, "roots": roots, "sprites": sprites,
        "scope": "existing-normal-world-consumers-only", "spawned_for_audit": false}))
}

fn emit(audit: &Audit, status: &str, detail: Value) {
    let value = json!({"schema_version": 1, "source": "compiled_default", "status": status,
        "process_id": std::process::id(), "inventory_sha256": audit.inventory_sha256,
        "bindings": audit.bindings, "detail": detail, "accepted": false,
        "promotion_authority": false, "performance_evidence": false});
    info!("{MARKER}: {value}");
}

fn observe(world: &mut World) {
    let Some(audit) = world.get_resource::<Audit>() else {
        return;
    };
    if audit.finished {
        return;
    }
    if !audit.selected {
        emit(audit, "selected", json!({}));
        world.resource_mut::<Audit>().selected = true;
    }
    let (ready, failed, rows) = pool_rows(world, &world.resource::<Audit>().bindings);
    if !ready && !failed && world.resource::<Time<Real>>().elapsed_secs_f64() < 120.0 {
        world.resource_mut::<Audit>().ready_frames = 0;
        return;
    }
    if ready {
        let mut audit = world.resource_mut::<Audit>();
        audit.ready_frames += 1;
        if audit.ready_frames < 60 {
            return;
        }
    }
    let bindings = world.resource::<Audit>().bindings.clone();
    let (status, detail) = if ready {
        match consumers(world, &bindings) {
            Ok(consumers) => (
                "active",
                json!({"generations": rows, "consumers": consumers}),
            ),
            Err(reason) => (
                "diagnostic_failure",
                json!({"generations": rows, "reason": reason}),
            ),
        }
    } else {
        (
            if failed { "load_failed" } else { "timeout" },
            json!({"generations": rows}),
        )
    };
    let mut audit = world.resource_mut::<Audit>();
    emit(&audit, status, detail);
    audit.finished = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::building_asset_set::{
        BuildingAssetAuthority, BuildingAssetKind, BuildingAssetSetIdentity,
    };

    fn binding() -> ReleaseBinding {
        ReleaseBinding {
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::Tank,
                generation: 1,
                authority: BuildingAssetAuthority::ReleaseApproved,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-tank-v1.buildingset".into(),
        }
    }

    #[test]
    fn empty_or_explicit_or_isolated_inputs_never_enable_default_audit() {
        let mut app = App::new();
        configure(&mut app, &[], "[]", true);
        assert!(!app.world().contains_resource::<Audit>());
        configure(&mut app, &[binding()], "unused", false);
        assert!(!app.world().contains_resource::<Audit>());
        assert_eq!(
            app.world_mut()
                .query::<&Building>()
                .iter(app.world())
                .count(),
            0
        );
        configure(&mut app, &[binding()], "compiled bytes", true);
        let audit = app.world().resource::<Audit>();
        assert!(!audit.selected && !audit.finished);
        assert_eq!(audit.bindings.len(), 1);
        assert_eq!(
            app.world_mut()
                .query::<&Building>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn activation_requires_exact_active_descriptor_no_pending_failure_and_real_residency() {
        let expected = binding().identity;
        assert!(activation_matches(
            &expected,
            Some(&expected),
            Some(&expected),
            false,
            false,
            true
        ));
        let mut foreign = expected.clone();
        foreign.generation += 1;
        for (active, descriptor, pending, failed, resident) in [
            (None, None, false, false, true),
            (Some(&foreign), Some(&expected), false, false, true),
            (Some(&expected), Some(&foreign), false, false, true),
            (Some(&expected), Some(&expected), true, false, true),
            (Some(&expected), Some(&expected), false, true, true),
            (Some(&expected), Some(&expected), false, false, false),
        ] {
            assert!(!activation_matches(
                &expected, active, descriptor, pending, failed, resident
            ));
        }
    }

    #[test]
    fn existing_owner_is_recorded_even_when_its_root_or_sprite_is_missing() {
        for kind in [
            super::super::super::BuildingAssetKind::Tank,
            super::super::super::BuildingAssetKind::SandPile,
        ] {
            let mut world = World::new();
            world.init_resource::<BuildingAssetPool>();
            let building_kind = if kind == super::super::super::BuildingAssetKind::Tank {
                hw_jobs::BuildingType::Tank
            } else {
                hw_jobs::BuildingType::SandPile
            };
            let owner = world
                .spawn(Building {
                    kind: building_kind,
                    is_provisional: false,
                })
                .id();
            let mut input = binding();
            input.identity.kind = kind;
            let observed = consumers(&mut world, &[input]).unwrap();
            assert_eq!(observed["owners"].as_array().unwrap().len(), 1);
            assert_eq!(observed["owners"][0]["entity"], format!("{owner:?}"));
            assert_eq!(observed["roots"], json!([]));
            assert_eq!(observed["sprites"], json!([]));
        }
    }

    #[test]
    fn layer_target_is_recorded_even_when_its_sprite_is_missing() {
        let mut world = World::new();
        world.init_resource::<BuildingAssetPool>();
        let owner = world
            .spawn(Building {
                kind: hw_jobs::BuildingType::SandPile,
                is_provisional: false,
            })
            .id();
        let child = world
            .spawn((ChildOf(owner), hw_visual::layer::VisualLayerKind::Struct))
            .id();
        let mut input = binding();
        input.identity.kind = BuildingAssetKind::SandPile;
        let observed = consumers(&mut world, &[input.clone()]).unwrap();
        assert_eq!(
            observed["owners"][0]["sprite_targets"][0]["entity"],
            format!("{child:?}")
        );
        assert_eq!(observed["owners"][0]["sprite_targets"][0]["role"], "child");
        assert_eq!(observed["sprites"], json!([]));
        world.entity_mut(child).insert(Sprite::default());
        let observed = consumers(&mut world, &[input]).unwrap();
        assert_eq!(observed["sprites"].as_array().unwrap().len(), 1);
        assert_eq!(observed["sprites"][0]["owner"], format!("{owner:?}"));
        assert_eq!(observed["sprites"][0]["parent"], format!("{owner:?}"));
    }

    #[test]
    fn no_existing_consumers_are_not_reported_as_generated_successes() {
        let mut world = World::new();
        world.init_resource::<BuildingAssetPool>();
        let observed = consumers(&mut world, &[binding()]).unwrap();
        assert_eq!(observed["roots"], json!([]));
        assert_eq!(observed["sprites"], json!([]));
        assert_eq!(observed["spawned_for_audit"], false);
        assert_eq!(world.query::<&Building>().iter(&world).count(), 0);
        let (ready, failed, rows) = pool_rows(&world, &[binding()]);
        assert!(!ready && !failed);
        assert_eq!(rows[0]["active"], Value::Null);
        assert!(
            rows[0]["meshes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["resident"] == false)
        );
    }
}
