//! Seeded M5 contrast/occlusion/descriptor comparison, never ordinary input proof.
use super::*;
use crate::plugins::startup::{Camera3dRtt, RttCompositeMaterial, RttCompositeSprite, RttRuntime};
use crate::systems::visual::building_presentation::apply_building_preview;
use bevy::{camera::visibility::RenderLayers, sprite::Anchor};
use hw_core::soul::DamnedSoul;
use hw_jobs::AssignedTask;
use hw_visual::ActorBillboard3d;

fn preview_position(spec: &Specimen) -> Vec3 {
    (spec.center - Vec2::Y * 160.0).extend(hw_core::constants::Z_MAP + 0.2)
}

pub(super) fn prepare(world: &mut World) {
    let Some(session) = world.get_resource::<BuildingArtSession>() else {
        return;
    };
    if !session.is_m5_context()
        || world.resource::<context::ContextState>().prepared
        || world.resource::<BuildingArtStaticState>().phase != Phase::Seeded
    {
        return;
    }
    let identity = session.identity.clone();
    let selected_ordinal = session.lamp_supply_target;
    let Some(descriptor) = world
        .resource::<BuildingAssetPool>()
        .descriptor(identity.kind)
        .cloned()
    else {
        return;
    };
    if descriptor.manifest.identity != identity {
        return;
    }
    let specs = world.resource::<BuildingArtStaticState>().specs.clone();
    let mut previews = Vec::new();
    for spec in &specs {
        if crate::systems::visual::building_presentation::asset_kind(spec.kind)
            != Some(identity.kind)
        {
            continue;
        }
        // Deliberately not PlacementGhost. No validity tint or ordinary-input claim.
        let entity = world
            .spawn((
                Sprite::default(),
                Transform::from_translation(preview_position(spec)),
                RenderLayers::layer(0),
                Name::new("Seeded M5 descriptor preview"),
            ))
            .id();
        apply_building_preview(world, entity, Some(&descriptor), None);
        previews.push(entity);
    }
    if let Some(ordinal) = selected_ordinal {
        let state = world.resource::<BuildingArtStaticState>();
        if let Some(owner) = state
            .specs
            .iter()
            .zip(&state.owners)
            .find(|(spec, _)| {
                spec.kind == BuildingType::OutdoorLamp && spec.ordinal == usize::from(ordinal)
            })
            .map(|(_, owner)| *owner)
        {
            // One-time seeded selection only. Production presentation owns all
            // UI text, state and layout; no per-frame repair or input claim.
            world
                .resource_mut::<crate::interface::selection::SelectedEntity>()
                .0 = Some(owner);
        }
    }
    *world.resource_mut::<context::ContextState>() = context::ContextState {
        previews,
        prepared: true,
    };
}

fn pose(value: &Transform) -> Value {
    json!({"translation": value.translation.to_array(), "rotation": value.rotation.to_array(), "scale": value.scale.to_array()})
}

fn layers(world: &World, entity: Entity) -> Vec<usize> {
    world
        .get::<RenderLayers>(entity)
        .cloned()
        .unwrap_or_default()
        .iter()
        .collect()
}

fn scene_target_matches(
    actual: Option<&bevy::camera::RenderTarget>,
    expected: &bevy::camera::RenderTarget,
) -> bool {
    matches!((actual, expected),
        (Some(bevy::camera::RenderTarget::Image(actual)), bevy::camera::RenderTarget::Image(expected)) if actual == expected)
}

fn composite_scene_matches(material: Option<&RttCompositeMaterial>, scene: &Handle<Image>) -> bool {
    material.is_some_and(|material| material.scene_texture == *scene)
}

pub(super) fn mesh_rows(world: &mut World, owner: Entity, actor: bool) -> Vec<Value> {
    let roots = if actor {
        world
            .query::<(Entity, &ActorBillboard3d)>()
            .iter(world)
            .filter(|(_, visual)| visual.owner == owner)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    } else {
        world
            .query::<(Entity, &Building3dVisual)>()
            .iter(world)
            .filter(|(_, visual)| visual.owner == owner)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    };
    let mut rows = Vec::new();
    for root in roots {
        let Some(root_pose) = world.get::<Transform>(root).copied() else {
            return Vec::new();
        };
        if world.get::<ChildOf>(root).is_some() {
            return Vec::new();
        }
        let descendants = world
            .get::<Children>(root)
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for entity in std::iter::once(root).chain(descendants) {
            if let (Some(mesh), Some(local), Some(visible)) = (
                world.get::<Mesh3d>(entity),
                world.get::<Transform>(entity),
                world.get::<ViewVisibility>(entity),
            ) {
                if world.resource::<Assets<Mesh>>().get(&mesh.0).is_none() {
                    return Vec::new();
                }
                let Some(global) = world.get::<GlobalTransform>(entity) else {
                    return Vec::new();
                };
                let expected = if entity == root {
                    GlobalTransform::from(root_pose)
                } else {
                    GlobalTransform::from(root_pose).mul_transform(*local)
                };
                if !global.to_matrix().abs_diff_eq(expected.to_matrix(), 0.001) {
                    return Vec::new();
                }
                let material = if let Some(handle) =
                    world.get::<MeshMaterial3d<StandardMaterial>>(entity)
                {
                    let Some(asset) = world.resource::<Assets<StandardMaterial>>().get(&handle.0)
                    else {
                        return Vec::new();
                    };
                    json!({"kind": "standard", "handle": format!("{:?}", handle.0.id()),
                        "base_color": asset.base_color.to_linear().to_f32_array(), "values": format!("{asset:?}")})
                } else if let Some(handle) =
                    world.get::<MeshMaterial3d<TopDownStructuralMaterial>>(entity)
                {
                    let Some(asset) = world
                        .resource::<Assets<TopDownStructuralMaterial>>()
                        .get(&handle.0)
                    else {
                        return Vec::new();
                    };
                    json!({"kind": "structural", "handle": format!("{:?}", handle.0.id()),
                        "base_color": asset.base.base_color.to_linear().to_f32_array(), "values": format!("{asset:?}")})
                } else {
                    return Vec::new();
                };
                rows.push(json!({"entity": entity.to_bits(), "root": root.to_bits(), "owner": owner.to_bits(),
                    "mesh": format!("{:?}", mesh.0.id()), "pose": pose(local), "layers": layers(world, entity),
                    "root_pose": pose(&root_pose), "global_matrix": global.to_matrix().to_cols_array(),
                    "material": material, "visible": visible.get()}));
            }
        }
    }
    rows.sort_by_key(|row| row["entity"].as_u64());
    rows
}

pub(super) fn snapshot(world: &mut World) -> Result<Option<Value>, String> {
    let session = world.resource::<BuildingArtSession>();
    if !session.is_m5_context() {
        return Err("M5 context session differs".into());
    }
    let identity = session.identity.clone();
    let lamp_ui = session.is_lamp_supply_context();
    let variant = session.variant;
    let view = session.view;
    let selected_ordinal = session.lamp_supply_target;
    let config = world.resource::<PerfScenarioConfig>();
    let size = config.size();
    if !config.enabled()
        || config.workload() != PerfWorkload::BuildingArtStatic
        || config.size() != super::super::PerfScenarioSize::Small
        || world.contains_resource::<M6Comparison>()
        || !world.resource::<Time<Virtual>>().is_paused()
    {
        return Err("M5 context requires a separate paused small static workload".into());
    }
    let state = world.resource::<BuildingArtStaticState>();
    if !state.completion_effects_settled || !world.resource::<context::ContextState>().prepared {
        return Ok(None);
    }
    let (specs, owners): (Vec<_>, Vec<_>) = state
        .specs
        .iter()
        .zip(&state.owners)
        .filter(|(spec, _)| {
            crate::systems::visual::building_presentation::asset_kind(spec.kind)
                == Some(identity.kind)
        })
        .map(|(spec, owner)| (spec.clone(), *owner))
        .unzip();
    let sources = usize::from(identity.kind == BuildingAssetKind::OutdoorLamp) * 2;
    let source_workers = sources * 3;
    let all_actors = state.actors.clone();
    if all_actors.len() != 8 + source_workers {
        return Err("M5 context actor inventory differs".into());
    }
    let actors = all_actors[source_workers..].to_vec();
    let previews = world.resource::<context::ContextState>().previews.clone();
    if specs.len() != 4
        || owners.len() != 4
        || actors.len() != 8
        || previews.len() != 4
        || world.query::<&Building>().iter(world).count() != 12 + sources
        || world.query::<&DamnedSoul>().iter(world).count() != 8 + source_workers
    {
        return Err("M5 context owner/wall/Soul/preview inventory differs".into());
    }
    let power = if sources > 0 {
        let Some(power) = super::m5_power::snapshot(world, &all_actors[..source_workers])? else {
            return Ok(None);
        };
        power
    } else {
        Value::Null
    };
    let pool = world.resource::<BuildingAssetPool>();
    let Some(descriptor) = pool.descriptor(identity.kind).cloned() else {
        return Ok(None);
    };
    if descriptor.manifest.identity != identity
        || pool.active(identity.kind) != Some(&identity)
        || pool.generation_counts() != (1, 0)
        || !pool.presentation_allowed(&identity)
    {
        return Err("M5 context resident identity differs".into());
    }
    let (camera_center, camera_scale) =
        layout::camera_frame(Some(world.resource::<BuildingArtSession>()), size);
    let cameras = world
        .query_filtered::<(Entity, &Camera, &GlobalTransform, &Transform), With<hw_ui::camera::MainCamera>>(
        )
        .iter(world)
        .map(|(entity, camera, global, local)| (entity, camera.clone(), *global, *local))
        .collect::<Vec<_>>();
    let [(camera_entity, camera, global, local)] = cameras.as_slice() else {
        return Err("M5 context requires one main camera".into());
    };
    if local.translation.truncate() != camera_center
        || local.scale != Vec3::splat(camera_scale)
        || !camera.is_active
        || camera.viewport.is_some()
        || camera.logical_viewport_size() != Some(Vec2::new(1280.0, 720.0))
        || !matches!(
            world.get::<bevy::camera::RenderTarget>(*camera_entity),
            Some(bevy::camera::RenderTarget::Window(
                bevy::window::WindowRef::Primary
            ))
        )
    {
        return Err("M5 context main camera changed".into());
    }
    let rtt_cameras = world
        .query_filtered::<(Entity, &Camera, &Transform, &Projection), With<Camera3dRtt>>()
        .iter(world)
        .map(|(entity, camera, transform, projection)| {
            (
                entity,
                camera.clone(),
                *transform,
                format!("{projection:?}"),
            )
        })
        .collect::<Vec<_>>();
    let [(rtt_entity, rtt_camera, rtt_pose, rtt_projection)] = rtt_cameras.as_slice() else {
        return Err("M5 context RTT camera differs".into());
    };
    if !rtt_camera.is_active || layers(world, *rtt_entity) != [hw_core::constants::LAYER_3D] {
        return Err("M5 context RTT layers differ".into());
    }
    let runtime = world.resource::<RttRuntime>();
    let scene = runtime.scene.clone();
    if !scene_target_matches(
        world.get::<bevy::camera::RenderTarget>(*rtt_entity),
        &runtime.scene_render_target(),
    ) || rtt_camera.viewport.is_some()
    {
        return Err("M5 context RTT output target differs".into());
    }
    let composites = world
        .query_filtered::<(
            Entity,
            &Transform,
            &MeshMaterial2d<RttCompositeMaterial>,
            &ViewVisibility,
        ), With<RttCompositeSprite>>()
        .iter(world)
        .map(|(entity, transform, material, visible)| {
            (entity, *transform, material.0.clone(), visible.get())
        })
        .collect::<Vec<_>>();
    let [(composite_entity, composite_pose, composite_material, true)] = composites.as_slice()
    else {
        return Err("M5 context composite missing/invisible".into());
    };
    let compensation = hw_core::constants::topdown_rtt_vertical_compensation();
    if composite_pose.translation != Vec3::new(0.0, 0.0, hw_core::constants::Z_RTT_COMPOSITE)
        || composite_pose.rotation != Quat::IDENTITY
        || composite_pose.scale != Vec3::new(1280.0, 720.0 * compensation, 1.0)
        || layers(world, *composite_entity) != [hw_core::constants::LAYER_OVERLAY]
        || !composite_scene_matches(
            world
                .resource::<Assets<RttCompositeMaterial>>()
                .get(composite_material),
            &scene,
        )
    {
        return Err("M5 context composite output binding differs".into());
    }
    let project = |point: Vec2| -> Result<[f32; 2], String> {
        let value = camera
            .world_to_viewport(global, point.extend(0.0))
            .map_err(|error| error.to_string())?;
        if !value.is_finite()
            || value.x < 0.0
            || value.x > 1280.0
            || value.y < 0.0
            || value.y > 720.0
        {
            return Err("M5 context projection outside viewport".into());
        }
        Ok(value.to_array())
    };
    let mut records = Vec::new();
    for (ordinal, ((spec, owner), preview)) in specs.iter().zip(&owners).zip(&previews).enumerate()
    {
        let building = world
            .get::<Building>(*owner)
            .ok_or("M5 context owner missing")?;
        if crate::systems::visual::building_presentation::asset_kind(building.kind)
            != Some(identity.kind)
            || building.is_provisional
            || spec.ordinal != ordinal
        {
            return Err("M5 context owner kind/ordinal differs".into());
        }
        let owner_pose = *world
            .get::<Transform>(*owner)
            .ok_or("M5 context owner transform missing")?;
        if owner_pose.translation.truncate() != spec.center
            || owner_pose.rotation != Quat::IDENTITY
            || owner_pose.scale != Vec3::ONE
        {
            return Err("M5 context owner pose differs".into());
        }
        let mut terrain = Vec::new();
        for &(x, y) in &spec.tiles {
            let map = world.resource::<hw_world::WorldMap>();
            if map.building_entity((x, y)) != Some(*owner) {
                return Err("M5 context footprint differs".into());
            }
            let tile = map
                .pos_to_idx(x, y)
                .and_then(|index| map.terrain_at_idx(index))
                .ok_or("M5 context terrain missing")?;
            terrain.push(json!({"grid": [x, y], "terrain": format!("{tile:?}")}));
        }
        let foregrounds = world
            .query_filtered::<(Entity, &ChildOf), With<hw_visual::layer::VisualLayerKind>>()
            .iter(world)
            .filter(|(_, parent)| parent.parent() == *owner)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        let [sprite_id] = foregrounds.as_slice() else {
            return Ok(None);
        };
        let sprite = world
            .get::<Sprite>(*sprite_id)
            .ok_or("M5 foreground sprite missing")?;
        let local = world
            .get::<Transform>(*sprite_id)
            .ok_or("M5 foreground transform missing")?;
        let global = world
            .get::<GlobalTransform>(*sprite_id)
            .ok_or("M5 foreground global missing")?;
        let role = if identity.kind == BuildingAssetKind::OutdoorLamp && ordinal >= 2 {
            "world_on"
        } else {
            &descriptor.manifest.world_preview.image_role
        };
        if descriptor.image(role) != Some(&sprite.image)
            || (identity.kind == BuildingAssetKind::OutdoorLamp
                && world
                    .get::<PoweredVisualState>(*owner)
                    .is_none_or(|state| state.is_powered != (ordinal >= 2)))
        {
            return Err("M5 context actual image/power differs".into());
        }
        let visible = world
            .get::<ViewVisibility>(*sprite_id)
            .is_some_and(|visibility| visibility.get());
        if !visible {
            return Ok(None);
        }
        let artifact_role = format!("image:{role}");
        let artifact = descriptor
            .manifest
            .artifacts
            .iter()
            .find(|entry| entry.role == artifact_role)
            .ok_or("M5 image artifact missing")?;
        let mut foreground = foreground::measured_values(sprite, &owner_pose, local, global)?;
        foreground.as_object_mut().expect("measured object").extend(json!({"schema": 1, "owner": owner.to_bits(),
            "entity": sprite_id.to_bits(), "parent": owner.to_bits(), "identity": identity, "descriptor": descriptor.manifest.world_preview,
            "image_role": role, "artifact": artifact, "image_handle": format!("{:?}", sprite.image.id()), "image_matches": true,
            "anchor": world.get::<Anchor>(*sprite_id).map_or(Vec2::ZERO, |anchor| anchor.0).to_array(),
            "layers": layers(world, *sprite_id), "visible": visible}).as_object().expect("metadata object").clone());
        let mut walls = Vec::new();
        for grid in context::wall_grids(spec) {
            let entity = world
                .resource::<hw_world::WorldMap>()
                .building_entity(grid)
                .ok_or("M5 context wall missing")?;
            if world.get::<Building>(entity).is_none_or(|building| {
                building.kind != BuildingType::Wall || building.is_provisional
            }) {
                return Err("M5 context wall kind differs".into());
            }
            let meshes = mesh_rows(world, entity, false);
            if meshes.is_empty() || !meshes.iter().any(|row| row["visible"] == true) {
                return Ok(None);
            }
            let wall_pose = world
                .get::<Transform>(entity)
                .ok_or("M5 wall pose missing")?;
            let expected =
                crate::systems::visual::building3d_cleanup::building_presentation_transform(
                    BuildingType::Wall,
                    wall_pose,
                );
            if meshes.iter().any(|row| row["root_pose"] != pose(&expected)) {
                return Err("M5 wall renderer pose differs from owner".into());
            }
            walls.push(json!({"entity": entity.to_bits(), "grid": [grid.0, grid.1],
                "pose": pose(wall_pose),
                "ground_projection": project(hw_world::WorldMap::grid_to_world(grid.0, grid.1))?, "meshes": meshes}));
        }
        let mut souls = Vec::new();
        for (side, entity) in [-1.0, 1.0]
            .into_iter()
            .zip(&actors[ordinal * 2..ordinal * 2 + 2])
        {
            let position = spec.center + Vec2::Y * (side * 12.0);
            let actual = world
                .get::<Transform>(*entity)
                .ok_or("M5 context Soul disappeared")?;
            if actual.translation.truncate() != position
                || !matches!(world.get::<AssignedTask>(*entity), Some(AssignedTask::None))
                || world
                    .get::<hw_core::relationships::WorkingOn>(*entity)
                    .is_some()
                || world
                    .get::<hw_core::relationships::RestingIn>(*entity)
                    .is_some()
            {
                return Err("M5 context Soul pose/task/relation changed".into());
            }
            let actor_pose = pose(actual);
            let meshes = mesh_rows(world, *entity, true);
            if meshes.len() != 1 || meshes[0]["visible"] != true {
                return Ok(None);
            }
            let rendered = &meshes[0]["root_pose"];
            if rendered["translation"]
                != json!([
                    position.x,
                    hw_core::constants::TILE_SIZE * 0.55,
                    -position.y
                ])
                || rendered["rotation"] != json!(rtt_pose.rotation.to_array())
                || !matches!(rendered["scale"][0].as_f64(), Some(1.0 | -1.0))
                || rendered["scale"][1] != 1.0
                || rendered["scale"][2] != 1.0
            {
                return Err("M5 Soul renderer pose differs from owner/camera".into());
            }
            souls.push(
                json!({"entity": entity.to_bits(), "side": side, "pose": actor_pose,
                "ground_projection": project(position)?, "meshes": meshes}),
            );
        }
        let sprite = world
            .get::<Sprite>(*preview)
            .ok_or("M5 descriptor preview missing")?;
        let actual = world
            .get::<Transform>(*preview)
            .ok_or("M5 descriptor preview transform missing")?;
        if descriptor.image(&descriptor.manifest.world_preview.image_role) != Some(&sprite.image)
            || actual.translation != preview_position(spec)
            || actual.rotation != Quat::IDENTITY
            || actual.scale != Vec3::ONE
            || sprite.color != Color::WHITE
            || layers(world, *preview) != [0]
        {
            return Err("M5 descriptor preview changed".into());
        }
        if !world
            .get::<ViewVisibility>(*preview)
            .is_some_and(|visibility| visibility.get())
        {
            return Ok(None);
        }
        records.push(json!({"ordinal": ordinal, "kind": format!("{:?}", spec.kind), "owner": owner.to_bits(),
            "anchor": [spec.anchor.0, spec.anchor.1], "center": spec.center.to_array(), "tiles": terrain,
            "foreground": foreground, "walls": walls, "souls": souls,
            "preview": {"entity": preview.to_bits(), "image_handle": format!("{:?}", sprite.image.id()),
                "canvas_wu": sprite.custom_size.map(|size| size.to_array()),
                "anchor": world.get::<Anchor>(*preview).map_or(Vec2::ZERO, |anchor| anchor.0).to_array(),
                "pose": pose(actual), "ground_projection": project(actual.translation.truncate())?, "visible": true}}));
    }
    let ui = if lamp_ui {
        let ordinal = selected_ordinal.ok_or("Lamp UI ordinal missing")?;
        let Some(ui) =
            super::lamp_supply_ui::snapshot(world, owners[usize::from(ordinal)], ordinal)?
        else {
            return Ok(None);
        };
        Some(ui)
    } else {
        None
    };
    let mut fixture = json!({"variant": variant, "lighting": "standard", "scope": if lamp_ui { "seeded-paused-lamp-supply-ui" } else { "seeded-paused-art-comparison" },
        "identity": identity, "target_count": 4, "wall_count": 8, "preview_count": 4, "souls": 8 + source_workers, "idle_souls": 8,
        "source_workers": source_workers, "source_count": sources, "power": power,
        "foreground_schema": 1, "view": view, "camera_scale": camera_scale, "camera_center": camera_center.to_array(),
        "main_camera": {"entity": camera_entity.to_bits(), "target": "primary-window", "viewport": [1280, 720], "pose": pose(local)},
        "rtt_camera": {"entity": rtt_entity.to_bits(), "active": true, "layers": layers(world, *rtt_entity), "pose": pose(rtt_pose),
            "projection": rtt_projection, "viewport": rtt_camera.logical_viewport_size().map(|value| value.to_array()), "scene_image": format!("{:?}", scene.id())},
        "composite": {"entity": composite_entity.to_bits(), "pose": pose(composite_pose), "visible": true, "layers": layers(world, *composite_entity),
            "scene_image": format!("{:?}", scene.id()), "material": format!("{:?}", composite_material.id()), "vertical_compensation": compensation},
        "records": records, "accepted": false, "performance_evidence": false, "promotion_authority": false});
    if let Some(ui) = ui {
        fixture["lamp_supply_ui"] = ui;
    }
    Ok(Some(fixture))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::building_asset_set::{
        BuildingAssetAuthority,
        acceptance::{BuildingArtLighting, BuildingArtVariant, BuildingArtView},
    };

    #[test]
    fn foreign_initial_render_target_and_composite_image_are_rejected() {
        use bevy::camera::{ImageRenderTarget, RenderTarget};
        let mut images = Assets::<Image>::default();
        let scene = images.add(Image::default());
        let foreign = images.add(Image::default());
        let expected = RenderTarget::Image(ImageRenderTarget {
            handle: scene.clone(),
            scale_factor: 1.0,
        });
        let other = RenderTarget::Image(ImageRenderTarget {
            handle: foreign.clone(),
            scale_factor: 1.0,
        });
        let scaled = RenderTarget::Image(ImageRenderTarget {
            handle: scene.clone(),
            scale_factor: 0.5,
        });
        assert!(scene_target_matches(Some(&expected), &expected));
        assert!(!scene_target_matches(Some(&other), &expected));
        assert!(!scene_target_matches(Some(&scaled), &expected));
        assert!(!scene_target_matches(None, &expected));
        assert!(!scene_target_matches(
            Some(&RenderTarget::Window(bevy::window::WindowRef::Primary)),
            &expected
        ));
        let material = RttCompositeMaterial {
            descriptor_layout_anchor: Vec4::ZERO,
            scene_texture: scene.clone(),
        };
        assert!(composite_scene_matches(Some(&material), &scene));
        assert!(!composite_scene_matches(Some(&material), &foreign));
        assert!(!composite_scene_matches(None, &scene));
    }

    #[test]
    fn descriptor_previews_are_prepared_once_without_repairing_later_changes() {
        let mut world = World::new();
        let mut pool = BuildingAssetPool::default();
        let set = pool.install_presentation_fixture(
            BuildingAssetKind::SandPile,
            1,
            BuildingAssetAuthority::ArtPreview,
        );
        world.insert_resource(BuildingArtSession {
            mode: "feedback".into(),
            identity: set.manifest.identity.clone(),
            locator: "manifests/building-sand-pile-v1.buildingset".into(),
            status_path: "/unused/m5-context.json".into(),
            nonce: "a".repeat(32),
            view: BuildingArtView::Normal,
            variant: BuildingArtVariant::M5ArtContext,
            lighting: BuildingArtLighting::Standard,
            spa_mask_page: None,
            lamp_supply_target: None,
            window_contract: None,
            scale_factor: None,
        });
        world.insert_resource(pool);
        world.init_resource::<context::ContextState>();
        world.insert_resource(BuildingArtStaticState {
            phase: Phase::Seeded,
            specs: layout::layout(super::super::super::PerfScenarioSize::Small)
                .into_iter()
                .filter(|spec| spec.kind == BuildingType::SandPile)
                .collect(),
            ..default()
        });
        prepare(&mut world);
        let previews = world.resource::<context::ContextState>().previews.clone();
        assert_eq!(previews.len(), 4);
        for entity in &previews {
            assert_eq!(
                Some(&world.get::<Sprite>(*entity).unwrap().image),
                set.image(&set.manifest.world_preview.image_role)
            );
        }
        world.get_mut::<Sprite>(previews[0]).unwrap().color = Color::BLACK;
        world.despawn(previews[1]);
        prepare(&mut world);
        assert_eq!(world.resource::<context::ContextState>().previews, previews);
        assert_eq!(
            world.get::<Sprite>(previews[0]).unwrap().color,
            Color::BLACK
        );
        assert!(world.get::<Sprite>(previews[1]).is_none());
    }
}
