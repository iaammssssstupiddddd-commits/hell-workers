//! Explicit, seeded RestArea art comparison. Not ordinary gameplay evidence.
//! Setup writes once; the post-visibility observer only reads and rejects drift.

use super::*;
use crate::assets::building_asset_set::acceptance::BuildingArtLighting;
use crate::plugins::startup::{
    Camera3dRtt, RttCompositeMaterial, RttCompositeSprite, RttDirectionalLight,
    RttExtraDirectionalLight, RttRuntime,
};
use crate::systems::visual::building_presentation::{EquipmentRoot, apply_building_preview};
use bevy::{camera::visibility::RenderLayers, sprite::Anchor};

const SCOPE: &str = "seeded-paused-art-comparison";

#[derive(Resource, Default)]
pub(crate) struct ContextState {
    pub(super) previews: Vec<Entity>,
    pub(super) prepared: bool,
}

pub(super) fn wall_grids(spec: &Specimen) -> [(i32, i32); 2] {
    // Use the real placement footprint: RestArea extends toward negative grid Y.
    let min_y = spec
        .tiles
        .iter()
        .map(|tile| tile.1)
        .min()
        .expect("nonempty footprint");
    let max_y = spec
        .tiles
        .iter()
        .map(|tile| tile.1)
        .max()
        .expect("nonempty footprint");
    // Tank's real two-cell BucketStorage is outside its own footprint.
    // Keep it between the owner and front wall, never occupy its placement.
    let front_y = if spec.kind == BuildingType::Tank {
        max_y.max(spec.companion().1) + 1
    } else {
        max_y + 1
    };
    [(spec.anchor.0, min_y - 1), (spec.anchor.0, front_y)]
}

pub(super) fn validate_layout(specs: &[Specimen], map: &hw_world::WorldMap) -> Result<(), String> {
    let mut occupied = specs
        .iter()
        .flat_map(|spec| spec.tiles.iter().copied())
        .collect::<HashSet<_>>();
    for spec in specs.iter().filter(|spec| spec.kind == BuildingType::Tank) {
        let (x, y) = spec.companion();
        for cell in [(x, y), (x + 1, y)] {
            if !occupied.insert(cell)
                || !map.is_walkable(cell.0, cell.1)
                || map.has_building(cell)
                || map.has_stockpile(cell)
            {
                return Err("occupied/overlapping Tank context companion grid".into());
            }
        }
    }
    for grid in specs.iter().flat_map(wall_grids) {
        if !occupied.insert(grid)
            || !map.is_walkable(grid.0, grid.1)
            || map.has_building(grid)
            || map.has_stockpile(grid)
        {
            return Err("occupied/overlapping context wall grid".into());
        }
    }
    Ok(())
}

fn preview_position(spec: &Specimen) -> Vec3 {
    (spec.center - Vec2::Y * 160.0).extend(hw_core::constants::Z_MAP + 0.2)
}

fn light_values(lighting: BuildingArtLighting) -> (f32, f32) {
    match lighting {
        BuildingArtLighting::Standard => (500.0, 12_000.0),
        BuildingArtLighting::Dim => (100.0, 2_400.0),
    }
}

/// Called only by the pre-readiness settle chain, never after readiness.
pub(super) fn prepare(world: &mut World) {
    if world
        .get_resource::<BuildingArtSession>()
        .is_some_and(BuildingArtSession::is_m5_context)
    {
        super::m5_context::prepare(world);
        return;
    }
    let Some(session) = world.get_resource::<BuildingArtSession>() else {
        return;
    };
    if !(session.is_rest_context() || session.is_m2_context())
        || world.resource::<ContextState>().prepared
    {
        return;
    }
    let lighting = session.lighting;
    let identity = session.identity.clone();
    let state = world.resource::<BuildingArtStaticState>();
    if state.phase != Phase::Seeded {
        return;
    }
    let specs = state.specs.clone();
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
    let (ambient, sun) = light_values(lighting);
    for mut light in world
        .query_filtered::<&mut AmbientLight, With<Camera3dRtt>>()
        .iter_mut(world)
    {
        light.brightness = ambient;
    }
    for mut light in world
        .query_filtered::<&mut DirectionalLight, With<RttDirectionalLight>>()
        .iter_mut(world)
    {
        light.illuminance = sun;
    }
    let mut previews = Vec::new();
    for spec in &specs {
        // Deliberately not PlacementGhost: no ordinary input or validity claim.
        let entity = world
            .spawn((
                Sprite::default(),
                Transform::from_translation(preview_position(spec)),
                RenderLayers::layer(0),
                Name::new("Seeded mesh art descriptor preview"),
            ))
            .id();
        apply_building_preview(world, entity, Some(&descriptor), None);
        previews.push(entity);
    }
    *world.resource_mut::<ContextState>() = ContextState {
        previews,
        prepared: true,
    };
}

fn transform(transform: &Transform) -> Value {
    json!({"translation": transform.translation.to_array(), "rotation": transform.rotation.to_array(),
        "scale": transform.scale.to_array()})
}

fn layers(world: &World, entity: Entity) -> Vec<usize> {
    world
        .get::<RenderLayers>(entity)
        .cloned()
        .unwrap_or_default()
        .iter()
        .collect()
}

fn material_snapshot(material: &TopDownStructuralMaterial) -> Value {
    json!({"white": material.base.base_color == Color::WHITE,
        "opaque": material.base.alpha_mode == AlphaMode::Opaque, "lit": !material.base.unlit,
        "roughness": material.base.perceptual_roughness, "reflectance": material.base.reflectance,
        "metallic": material.base.metallic,
        "emissive_black": material.base.emissive == LinearRgba::BLACK,
        "color_srgba": material.base.base_color.to_srgba().to_f32_array(),
        "emissive_texture_absent": material.base.emissive_texture.is_none(),
        "base": format!("{:?}", material.base), "extension": format!("{:?}", material.extension)})
}

fn preview_artifact(
    set: &crate::assets::building_asset_set::BuildingAssetDescriptor,
) -> Result<&crate::assets::building_asset_set::BuildingArtifact, String> {
    let role = format!("image:{}", set.manifest.world_preview.image_role);
    let mut matching = set
        .manifest
        .artifacts
        .iter()
        .filter(|artifact| artifact.role == role);
    match (matching.next(), matching.next()) {
        (Some(artifact), None) => Ok(artifact),
        _ => Err("context requires exactly one image: preview artifact".into()),
    }
}

fn snapshot(world: &mut World) -> Result<Option<Value>, String> {
    let session = world.resource::<BuildingArtSession>();
    let identity = session.identity.clone();
    let view = session.view;
    let lighting = session.lighting;
    let variant = session.variant;
    let m2 = session.is_m2_context();
    let config = world.resource::<PerfScenarioConfig>();
    let size = config.size();
    if !config.enabled()
        || config.workload() != PerfWorkload::BuildingArtStatic
        || config.size() != super::super::PerfScenarioSize::Small
        || world.contains_resource::<M6Comparison>()
    {
        return Err("context requires its separate small static workload, without M6".into());
    }
    let state = world.resource::<BuildingArtStaticState>();
    if !world.resource::<ContextState>().prepared || !state.completion_effects_settled {
        return Ok(None);
    }
    if !world.resource::<Time<Virtual>>().is_paused() {
        return Err("context time is not paused".into());
    }
    let specs = state.specs.clone();
    let owners = state.owners.clone();
    let previews = world.resource::<ContextState>().previews.clone();
    if specs.len() != 4
        || owners.len() != 4
        || previews.len() != 4
        || world.query::<&Building>().iter(world).count() != 12
    {
        return Err("context owner/wall/preview inventory differs".into());
    }
    let pool = world.resource::<BuildingAssetPool>();
    let Some(descriptor) = pool.descriptor(identity.kind).cloned() else {
        return Ok(None);
    };
    if descriptor.manifest.identity != identity
        || pool.active(identity.kind) != Some(&identity)
        || pool.generation_counts() != (1, 0)
        || !pool.presentation_allowed(&identity)
    {
        return Err("context exact resident identity differs".into());
    }
    let m2_state = if m2 {
        let Some(state) = super::m2_context::snapshot(world, &specs, &owners)? else {
            return Ok(None);
        };
        Some(state)
    } else {
        None
    };
    let (camera_center, camera_scale) =
        layout::camera_frame(Some(world.resource::<BuildingArtSession>()), size);
    let cameras = world.query_filtered::<(Entity, &Camera, &GlobalTransform, &Transform), With<hw_ui::camera::MainCamera>>()
        .iter(world).map(|(entity, camera, global, local)| (entity, camera.clone(), *global, *local)).collect::<Vec<_>>();
    let [(camera_id, camera, global, local)] = cameras.as_slice() else {
        return Err("context main camera count differs".into());
    };
    if local.translation.truncate() != camera_center
        || local.scale != Vec3::splat(camera_scale)
        || !camera.is_active
        || camera.viewport.is_some()
        || camera.logical_viewport_size() != Some(Vec2::new(1280.0, 720.0))
        || !matches!(
            world.get::<bevy::camera::RenderTarget>(*camera_id),
            Some(bevy::camera::RenderTarget::Window(
                bevy::window::WindowRef::Primary
            ))
        )
    {
        return Err("context main camera changed".into());
    }
    let rtt_cameras = world.query_filtered::<(Entity, &Camera, &GlobalTransform, &Transform, &Projection), With<Camera3dRtt>>()
        .iter(world).map(|(entity, camera, global, local, projection)|
            (entity, camera.clone(), *global, *local, format!("{projection:?}"))).collect::<Vec<_>>();
    let [(rtt_id, rtt_camera, rtt_global, rtt_local, projection)] = rtt_cameras.as_slice() else {
        return Err("context RtT camera count differs".into());
    };
    if !rtt_camera.is_active || layers(world, *rtt_id) != [hw_core::constants::LAYER_3D] {
        return Err("context RtT camera inactive or wrong layers".into());
    }
    let runtime = world.resource::<RttRuntime>();
    let target = world.get::<bevy::camera::RenderTarget>(*rtt_id);
    let expected_target = runtime.scene_render_target();
    if !matches!((target, &expected_target), (Some(bevy::camera::RenderTarget::Image(actual)), bevy::camera::RenderTarget::Image(expected)) if actual == expected)
        || rtt_camera.viewport.is_some()
    {
        return Err("context RtT image target differs".into());
    }
    let scene = runtime.scene.clone();
    let composite = world
        .query_filtered::<(
            Entity,
            &Transform,
            &MeshMaterial2d<RttCompositeMaterial>,
            &ViewVisibility,
        ), With<RttCompositeSprite>>()
        .iter(world)
        .map(|(entity, local, material, visible)| {
            (entity, *local, material.0.clone(), visible.get())
        })
        .collect::<Vec<_>>();
    let [(composite_id, composite_transform, composite_material, composite_visible)] =
        composite.as_slice()
    else {
        return Err("context composite count differs".into());
    };
    let compensation = hw_core::constants::topdown_rtt_vertical_compensation();
    if !composite_visible
        || composite_transform.translation
            != Vec3::new(0.0, 0.0, hw_core::constants::Z_RTT_COMPOSITE)
        || composite_transform.scale != Vec3::new(1280.0, 720.0 * compensation, 1.0)
        || layers(world, *composite_id) != [hw_core::constants::LAYER_OVERLAY]
        || world
            .resource::<Assets<RttCompositeMaterial>>()
            .get(composite_material)
            .is_none_or(|material| material.scene_texture != scene)
    {
        return Err("context actual composite differs".into());
    }
    let mut records = Vec::new();
    for ((spec, owner), preview_id) in specs.iter().zip(&owners).zip(&previews) {
        let building = world
            .get::<Building>(*owner)
            .ok_or("context owner disappeared")?;
        let owner_transform = *world
            .get::<Transform>(*owner)
            .ok_or("missing context owner transform")?;
        if crate::systems::visual::building_presentation::asset_kind(building.kind)
            != Some(identity.kind)
            || building.is_provisional
            || owner_transform.translation.truncate() != spec.center
            || owner_transform.rotation != Quat::IDENTITY
            || owner_transform.scale != Vec3::ONE
            || !m2
                && world
                    .get::<RestAreaOccupants>(*owner)
                    .map_or(0, RestAreaOccupants::len)
                    != spec.rest_count()
        {
            return Err("context owner geometry/state differs".into());
        }
        for grid in &spec.tiles {
            if world
                .resource::<hw_world::WorldMap>()
                .building_entity(*grid)
                != Some(*owner)
            {
                return Err("context footprint differs".into());
            }
        }
        let roots = world
            .query::<(Entity, &Building3dVisual, &EquipmentRoot, &Transform)>()
            .iter(world)
            .filter(|(_, visual, _, _)| visual.owner == *owner)
            .map(|(entity, _, root, transform)| {
                (entity, root.observed_identity().cloned(), *transform)
            })
            .collect::<Vec<_>>();
        let [(root_id, admitted, root_transform)] = roots.as_slice() else {
            return Ok(None);
        };
        if admitted.as_ref() != Some(&identity) {
            return Ok(None);
        }
        if !root_transform
            .translation
            .abs_diff_eq(Vec3::new(spec.center.x, 0.0, -spec.center.y), 0.001)
        {
            return Err("context root feet differ".into());
        }
        if m2
            && (root_transform.rotation != Quat::IDENTITY
                || root_transform.scale != Vec3::ONE
                || world.get::<ChildOf>(*root_id).is_some())
        {
            return Err("M2 context root hierarchy/pose differs".into());
        }
        let parts = world
            .query::<(
                Entity,
                &ChildOf,
                &Mesh3d,
                &MeshMaterial3d<TopDownStructuralMaterial>,
                &Transform,
                &ViewVisibility,
            )>()
            .iter(world)
            .filter(|(_, parent, ..)| parent.parent() == *root_id)
            .map(|(entity, _, mesh, material, local, visible)| {
                (
                    entity,
                    mesh.clone(),
                    material.clone(),
                    *local,
                    visible.get(),
                )
            })
            .collect::<Vec<_>>();
        if parts.len() != descriptor.manifest.parts.len() {
            return Ok(None);
        }
        let mut part_records = Vec::new();
        for part in &descriptor.manifest.parts {
            let matching = parts
                .iter()
                .filter(|(_, mesh, ..)| Some(&mesh.0) == descriptor.mesh(&part.mesh_role))
                .collect::<Vec<_>>();
            let [actual] = matching.as_slice() else {
                return Err("context part roles differ".into());
            };
            let (entity, mesh, material, local, visible) = *actual;
            let original = Transform {
                translation: Vec3::from_array(part.translation_wu),
                rotation: Quat::from_array(part.rotation_xyzw),
                scale: Vec3::from_array(part.scale),
            };
            let (expected_pose, expected_visible, expected_color) = if m2 {
                super::m2_context::part_state(spec, &part.name, &original)
            } else {
                (original, true, Color::WHITE)
            };
            if *visible != expected_visible
                || world.resource::<Assets<Mesh>>().get(&mesh.0).is_none()
            {
                return Ok(None);
            }
            if !layers(world, *entity).contains(&hw_core::constants::LAYER_3D) {
                return Err("context part not visible to RtT camera".into());
            }
            let Some(material_asset) = world
                .resource::<Assets<TopDownStructuralMaterial>>()
                .get(&material.0)
            else {
                return Ok(None);
            };
            if material_asset.base.base_color_texture.as_ref() != descriptor.image("albedo") {
                return Err("context part albedo differs".into());
            }
            let material_values = material_snapshot(material_asset);
            if ["opaque", "lit", "emissive_black", "emissive_texture_absent"]
                .iter()
                .any(|key| material_values[key] != true)
                || material_asset.base.base_color != expected_color
                || m2
                    && (material_asset.base.perceptual_roughness != 1.0
                        || material_asset.base.reflectance != 0.0
                        || material_asset.base.metallic != 0.0)
            {
                return Err(
                    "context RestArea material is tinted, unlit, translucent or emissive".into(),
                );
            }
            if local.translation != expected_pose.translation
                || local.scale != expected_pose.scale
                || !local.rotation.abs_diff_eq(expected_pose.rotation, 0.001)
            {
                return Err("context part transform differs".into());
            }
            let global = world.get::<GlobalTransform>(*entity);
            if m2
                && global.is_none_or(|global| {
                    !global.to_matrix().abs_diff_eq(
                        GlobalTransform::from(*root_transform)
                            .mul_transform(*local)
                            .to_matrix(),
                        0.001,
                    )
                })
            {
                return Err("M2 context part global transform differs".into());
            }
            part_records.push(json!({"entity": entity.to_bits(), "role": part.mesh_role, "transform": transform(local),
                "global_matrix": global.map(|value| value.to_matrix().to_cols_array()),
                "layers": layers(world, *entity), "mesh": format!("{:?}", mesh.0.id()), "material": format!("{:?}", material.0.id()),
                "material_values": material_values, "visible": visible}));
        }
        let preview = &descriptor.manifest.world_preview;
        let sprite = world
            .get::<Sprite>(*preview_id)
            .ok_or("context preview disappeared")?
            .clone();
        let anchor = *world
            .get::<Anchor>(*preview_id)
            .ok_or("context preview anchor missing")?;
        let preview_transform = *world
            .get::<Transform>(*preview_id)
            .ok_or("context preview transform missing")?;
        let expected_anchor = Vec2::new(
            preview.anchor_px[0] / preview.canvas_px[0] as f32 - 0.5,
            0.5 - preview.anchor_px[1] / preview.canvas_px[1] as f32,
        );
        if Some(&sprite.image) != descriptor.image(&preview.image_role)
            || sprite.custom_size != Some(Vec2::from_array(preview.canvas_wu))
            || anchor.0 != expected_anchor
            || preview_transform.translation != preview_position(spec)
            || preview_transform.scale != Vec3::ONE
            || preview_transform.rotation != Quat::IDENTITY
            || sprite.color != Color::WHITE
            || layers(world, *preview_id) != [0]
        {
            return Err("context preview role/anchor/canvas/transform differs".into());
        }
        if world
            .resource::<Assets<Image>>()
            .get(&sprite.image)
            .is_none()
            || !world
                .get::<ViewVisibility>(*preview_id)
                .is_some_and(|visibility| visibility.get())
        {
            return Ok(None);
        }
        let point = |position| {
            camera
                .world_to_viewport(global, position)
                .map(|p| p.to_array())
                .map_err(|_| "context projection unavailable".to_string())
        };
        let mut walls = Vec::new();
        for grid in wall_grids(spec) {
            let wall = world
                .resource::<hw_world::WorldMap>()
                .building_entity(grid)
                .ok_or("context wall missing")?;
            if world
                .get::<Building>(wall)
                .is_none_or(|b| b.kind != BuildingType::Wall || b.is_provisional)
            {
                return Err("context wall owner differs".into());
            }
            let local = *world
                .get::<Transform>(wall)
                .ok_or("context wall transform missing")?;
            if local.translation.truncate() != hw_world::WorldMap::grid_to_world(grid.0, grid.1) {
                return Err("context wall ordering/transform differs".into());
            }
            let visuals = world
                .query::<(Entity, &Building3dVisual, &Transform)>()
                .iter(world)
                .filter(|(_, visual, _)| visual.owner == wall)
                .map(|(entity, _, local)| (entity, *local))
                .collect::<Vec<_>>();
            if visuals.len() != 1 {
                return Ok(None);
            }
            let (visual, visual_transform) = visuals[0];
            let expected =
                crate::systems::visual::building3d_cleanup::building_presentation_transform(
                    BuildingType::Wall,
                    &local,
                );
            if visual_transform != expected
                || !layers(world, visual).contains(&hw_core::constants::LAYER_3D)
            {
                return Err("context wall depth/transform/layers differ".into());
            }
            let Some(mesh) = world.get::<Mesh3d>(visual) else {
                return Ok(None);
            };
            let Some(material) = world.get::<MeshMaterial3d<TopDownStructuralMaterial>>(visual)
            else {
                return Ok(None);
            };
            let Some(material_asset) = world
                .resource::<Assets<TopDownStructuralMaterial>>()
                .get(&material.0)
            else {
                return Ok(None);
            };
            if world.resource::<Assets<Mesh>>().get(&mesh.0).is_none()
                || !world
                    .get::<ViewVisibility>(visual)
                    .is_some_and(|visible| visible.get())
            {
                return Ok(None);
            }
            let screen = rtt_camera
                .world_to_viewport(rtt_global, visual_transform.translation)
                .map_err(|_| "wall projection unavailable")?;
            let depth = rtt_global
                .affine()
                .inverse()
                .transform_point3(visual_transform.translation);
            walls.push(json!({"owner": wall.to_bits(), "grid": grid, "transform": transform(&local), "visuals": [{
                "entity": visual.to_bits(), "transform": transform(&visual_transform), "layers": layers(world, visual),
                "mesh": format!("{:?}", mesh.0.id()), "material": format!("{:?}", material.0.id()),
                "material_values": material_snapshot(material_asset), "visible": true,
                "rtt_screen": screen.to_array(), "view_space": depth.to_array()}]}));
        }
        let owner_screen = Vec2::from_array(point(owner_transform.translation)?);
        let preview_screen = Vec2::from_array(point(preview_transform.translation)?);
        let rtt_screen = rtt_camera
            .world_to_viewport(rtt_global, root_transform.translation)
            .map_err(|_| "context RtT projection unavailable")?;
        let viewport = rtt_camera
            .logical_viewport_size()
            .ok_or("missing RtT viewport")?;
        let composite_screen = Vec2::new(
            rtt_screen.x / viewport.x * 1280.0,
            ((rtt_screen.y / viewport.y - 0.5) * compensation + 0.5) * 720.0,
        );
        if !owner_screen.is_finite()
            || !preview_screen.is_finite()
            || !composite_screen.is_finite()
            || !owner_screen.abs_diff_eq(composite_screen, 1.0)
            || !(preview_screen - owner_screen)
                .abs_diff_eq(Vec2::new(0.0, 160.0 / camera_scale), 0.01)
            || [owner_screen, preview_screen]
                .iter()
                .any(|p| p.x < 0.0 || p.x > 1280.0 || p.y < 0.0 || p.y > 720.0)
        {
            return Err(
                "context completed/preview anchor projection differs or is offscreen".into(),
            );
        }
        records.push(json!({"ordinal": spec.ordinal, "owner": owner.to_bits(), "tiles": spec.tiles,
            "occupants": if m2 { None } else { Some(spec.rest_count()) }, "root": root_id.to_bits(), "root_transform": transform(root_transform),
            "root_layers": layers(world, *root_id), "parts": part_records, "walls": walls,
            "owner_screen": point(owner_transform.translation)?,
            "composite_screen": composite_screen.to_array(), "preview_offset_wu": [0.0, -160.0],
            "root_rtt_screen": rtt_camera.world_to_viewport(rtt_global, root_transform.translation).map_err(|_| "context RtT projection unavailable")?.to_array(),
            "preview": {"entity": preview_id.to_bits(), "descriptor": preview,
                "artifact": preview_artifact(&descriptor)?,
                "anchor": anchor.0.to_array(), "canvas_wu": sprite.custom_size.unwrap().to_array(),
                "transform": transform(&preview_transform), "image": format!("{:?}", sprite.image.id()),
                "screen_anchor": point(preview_transform.translation)?, "color": format!("{:?}", sprite.color), "visible": true}}));
    }
    let (ambient_expected, sun_expected) = light_values(lighting);
    let ambient = world.query::<(Entity, &AmbientLight)>().iter(world).map(|(entity, light)|
        json!({"entity": entity.to_bits(), "brightness": light.brightness, "color": format!("{:?}", light.color),
            "layers": layers(world, entity)})).collect::<Vec<_>>();
    if ambient.len() != 1
        || ambient[0]["entity"] != json!(rtt_id.to_bits())
        || ambient[0]["brightness"] != json!(ambient_expected)
    {
        return Err("context ambient light differs".into());
    }
    let directional = world.query::<(Entity, &DirectionalLight, &Transform)>().iter(world).map(|(entity, light, local)|
        json!({"entity": entity.to_bits(), "illuminance": light.illuminance, "shadows": light.shadow_maps_enabled,
            "color": format!("{:?}", light.color), "transform": transform(local), "layers": layers(world, entity),
            "primary": world.get::<RttDirectionalLight>(entity).is_some(),
            "extra": world.get::<RttExtraDirectionalLight>(entity).is_some()})).collect::<Vec<_>>();
    if directional.len() != 2
        || directional
            .iter()
            .filter(|v| {
                v["primary"] == true
                    && v["illuminance"] == json!(sun_expected)
                    && v["shadows"] == true
            })
            .count()
            != 1
        || directional
            .iter()
            .filter(|v| v["extra"] == true && v["illuminance"] == 0.0 && v["shadows"] == false)
            .count()
            != 1
    {
        return Err("context directional light inventory/settings differ".into());
    }
    Ok(Some(
        json!({"variant": variant, "lighting": lighting, "scope": SCOPE,
        "identity": identity, "target_count": 4, "wall_count": 8, "preview_count": 4,
        "souls": if m2 { 8 + usize::from(identity.kind == BuildingAssetKind::MudMixer) * 2 } else { 6 },
        "m2_state": m2_state,
        "geometry_contract_sha256": descriptor.manifest.geometry_contract_sha256,
        "view": view, "camera_scale": camera_scale, "camera_center": camera_center.to_array(),
        "camera": {"entity": camera_id.to_bits(), "transform": transform(local), "layers": layers(world, *camera_id)},
        "rtt_camera": {"entity": rtt_id.to_bits(), "transform": transform(rtt_local), "projection": projection,
            "layers": layers(world, *rtt_id), "viewport": rtt_camera.logical_viewport_size().map(|size| size.to_array()),
            "target": format!("{:?}", world.get::<bevy::camera::RenderTarget>(*rtt_id))},
        "composite": {"entity": composite_id.to_bits(), "transform": transform(composite_transform),
            "scene": format!("{:?}", scene.id()), "vertical_compensation": compensation},
        "records": records, "ambient": ambient, "directional": directional,
        "resources": {"meshes": world.resource::<Assets<Mesh>>().len(), "images": world.resource::<Assets<Image>>().len(),
            "materials": world.resource::<Assets<TopDownStructuralMaterial>>().len()},
        "accepted": false, "performance_evidence": false, "promotion_authority": false}),
    ))
}

pub(crate) fn observe(world: &mut World) {
    if !world
        .get_resource::<BuildingArtSession>()
        .is_some_and(BuildingArtSession::is_context)
        || !matches!(
            world.resource::<BuildingArtStaticState>().phase,
            Phase::Seeded | Phase::Ready
        )
    {
        return;
    }
    let result = if world.resource::<BuildingArtSession>().is_m5_context() {
        super::m5_context::snapshot(world)
    } else {
        snapshot(world)
    };
    let update = || -> Result<(), String> {
        let (publish, fixture) = {
            let mut state = world.resource_mut::<BuildingArtStaticState>();
            let publish = checkpoint(&mut state, result?)?;
            if state.phase != Phase::Ready {
                return Ok(());
            }
            (publish, state.evidence.clone())
        };
        world.resource_mut::<PerfScenarioApplied>().workload = true;
        if publish {
            let session = world.resource::<BuildingArtSession>();
            let scope = if session.is_lamp_supply_context() {
                "seeded-paused-lamp-supply-ui"
            } else {
                SCOPE
            };
            let status = json!({"schema_version": 1, "profile": "building-art", "mode": session.mode,
                "identity": session.identity, "nonce": session.nonce, "status": "ready", "stable_frames": 30,
                "fixture": fixture, "variant": session.variant, "lighting": session.lighting,
                "view": session.view, "camera_scale": session.view.scale(), "scope": scope});
            let temporary = session.status_path.with_extension("tmp");
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|e| e.to_string())?;
            serde_json::to_writer(&file, &status).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            std::fs::rename(temporary, &session.status_path).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    if let Err(reason) = update() {
        error!("REST_AREA_ART_CONTEXT: {reason}");
        world.resource_mut::<BuildingArtStaticState>().phase = Phase::Failed;
        world.write_message(AppExit::error());
    }
}

fn checkpoint(state: &mut BuildingArtStaticState, evidence: Option<Value>) -> Result<bool, String> {
    let Some(evidence) = evidence else {
        return if state.phase == Phase::Ready {
            Err("context disappeared after readiness".into())
        } else {
            Ok(false)
        };
    };
    if let Some(initial) = &state.evidence {
        if initial != &evidence {
            return Err(format!(
                "context changed after readiness; no repair permitted: {}",
                first_difference(initial, &evidence, "$")
                    .unwrap_or_else(|| "unknown JSON difference".into())
            ));
        }
    } else {
        state.evidence = Some(evidence);
        state.art_candidate = true;
        state.phase = Phase::Ready;
    }
    state.stable_frames += 1;
    Ok(state.stable_frames == 30)
}

/// Bounded, read-only diagnosis. It does not reseed or accept a changed snapshot.
fn first_difference(before: &Value, after: &Value, path: &str) -> Option<String> {
    if before == after {
        return None;
    }
    match (before, after) {
        (Value::Object(left), Value::Object(right)) if left.keys().eq(right.keys()) => {
            left.iter().find_map(|(key, value)| {
                first_difference(value, &right[key], &format!("{path}.{key}"))
            })
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => left
            .iter()
            .zip(right)
            .enumerate()
            .find_map(|(index, (left, right))| {
                first_difference(left, right, &format!("{path}[{index}]"))
            }),
        _ => {
            let left = before.to_string();
            let right = after.to_string();
            let offset = left
                .chars()
                .zip(right.chars())
                .take_while(|(left, right)| left == right)
                .count()
                .saturating_sub(40);
            let short = |text: &str| text.chars().skip(offset).take(200).collect::<String>();
            Some(format!(
                "{path} (chars {offset}..): before={} after={}",
                short(&left),
                short(&right)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_identifies_exact_nested_field_without_weakening_stability() {
        let initial =
            json!({"records": [{"material_values": {"lit": true}}], "resources": {"images": 100}});
        let mut changed = initial.clone();
        changed["resources"]["images"] = json!(101);
        assert!(
            first_difference(&initial, &changed, "$")
                .unwrap()
                .contains("$.resources.images")
        );
        changed = initial.clone();
        changed["records"][0]["material_values"]["lit"] = json!(false);
        assert!(
            first_difference(&initial, &changed, "$")
                .unwrap()
                .contains("$.records[0].material_values.lit")
        );
        let mut state = BuildingArtStaticState {
            phase: Phase::Seeded,
            ..default()
        };
        checkpoint(&mut state, Some(initial.clone())).unwrap();
        let failure = checkpoint(&mut state, Some(changed)).unwrap_err();
        assert!(failure.contains("no repair permitted: $.records[0].material_values.lit"));
        assert_eq!(state.evidence, Some(initial));
        assert_eq!(state.stable_frames, 1);
        let long = Value::String("a".repeat(5000));
        let changed = Value::String(format!("{}b", "a".repeat(4999)));
        assert!(first_difference(&long, &changed, "$").unwrap().len() < 600);
    }

    #[test]
    fn preview_artifact_uses_validated_image_namespace_and_rejects_missing_or_duplicate() {
        let mut pool = BuildingAssetPool::default();
        let mut set = pool.install_presentation_fixture(
            BuildingAssetKind::RestArea,
            1,
            crate::assets::building_asset_set::BuildingAssetAuthority::ArtPreview,
        );
        let artifact = preview_artifact(&set).unwrap().clone();
        assert_eq!(artifact.role, "image:world_preview");
        set.manifest.artifacts.push(artifact.clone());
        assert!(preview_artifact(&set).is_err());
        set.manifest.artifacts = vec![crate::assets::building_asset_set::BuildingArtifact {
            role: "world_preview".into(),
            ..artifact
        }];
        assert!(preview_artifact(&set).is_err());
        set.manifest.artifacts.clear();
        assert!(preview_artifact(&set).is_err());
    }

    #[test]
    fn seeded_previews_use_shared_descriptor_once_and_are_never_repaired() {
        use crate::assets::building_asset_set::{
            BuildingAssetAuthority, acceptance::BuildingArtView,
        };
        let mut world = World::new();
        let mut pool = BuildingAssetPool::default();
        let set = pool.install_presentation_fixture(
            BuildingAssetKind::RestArea,
            1,
            BuildingAssetAuthority::ArtPreview,
        );
        world.insert_resource(BuildingArtSession {
            mode: "feedback".into(), identity: set.manifest.identity.clone(),
            locator: "manifests/building-rest-area-v1.buildingset".into(), status_path: "/unused/context-test.json".into(),
            nonce: "a".repeat(32), view: BuildingArtView::Normal,
            variant: crate::assets::building_asset_set::acceptance::BuildingArtVariant::RestAreaArtContext,
            lighting: BuildingArtLighting::Standard,
            spa_mask_page: None,
            lamp_supply_target: None,
        });
        world.insert_resource(pool);
        world.init_resource::<ContextState>();
        world.insert_resource(BuildingArtStaticState {
            phase: Phase::Seeded,
            specs: layout::layout(super::super::super::PerfScenarioSize::Small)
                .into_iter()
                .filter(|spec| spec.kind == BuildingType::RestArea)
                .collect(),
            ..default()
        });
        prepare(&mut world);
        let previews = world.resource::<ContextState>().previews.clone();
        assert_eq!(previews.len(), 4);
        for entity in &previews {
            let sprite = world.get::<Sprite>(*entity).unwrap();
            assert_eq!(
                Some(&sprite.image),
                set.image(&set.manifest.world_preview.image_role)
            );
            assert_eq!(
                sprite.custom_size,
                Some(Vec2::from_array(set.manifest.world_preview.canvas_wu))
            );
            let preview = &set.manifest.world_preview;
            assert_eq!(
                world.get::<Anchor>(*entity).unwrap().0,
                Vec2::new(
                    preview.anchor_px[0] / preview.canvas_px[0] as f32 - 0.5,
                    0.5 - preview.anchor_px[1] / preview.canvas_px[1] as f32
                )
            );
        }
        world
            .get_mut::<Transform>(previews[0])
            .unwrap()
            .translation
            .x += 1.0;
        let changed = *world.get::<Transform>(previews[0]).unwrap();
        prepare(&mut world);
        assert_eq!(world.resource::<ContextState>().previews, previews);
        assert_eq!(world.query::<&Sprite>().iter(&world).count(), 4);
        assert_eq!(world.get::<Transform>(previews[0]), Some(&changed));
    }

    #[test]
    fn in_place_material_changes_are_observed_and_rejected_after_readiness() {
        let baseline = TopDownStructuralMaterial::default();
        for mutation in 0..6 {
            let mut state = BuildingArtStaticState {
                phase: Phase::Seeded,
                ..default()
            };
            let initial = material_snapshot(&baseline);
            checkpoint(&mut state, Some(initial.clone())).unwrap();
            let mut changed = baseline.clone();
            match mutation {
                0 => changed.base.unlit = true,
                1 => changed.base.base_color = Color::BLACK,
                2 => changed.base.emissive = LinearRgba::WHITE,
                3 => changed.base.alpha_mode = AlphaMode::Blend,
                4 => changed.base.perceptual_roughness = 0.25,
                _ => changed.extension.uniforms.wall_height = 123.0,
            }
            assert!(checkpoint(&mut state, Some(material_snapshot(&changed))).is_err());
            assert_eq!(state.evidence, Some(initial));
        }
    }

    #[test]
    fn context_tank_walls_clear_real_companion_inventory() {
        let specs = layout::layout(super::super::super::PerfScenarioSize::Small);
        let mut occupied = HashSet::new();
        for spec in specs.iter().filter(|spec| spec.kind == BuildingType::Tank) {
            for tile in &spec.tiles {
                assert!(occupied.insert(*tile));
            }
            let (x, y) = spec.companion();
            for cell in [(x, y), (x + 1, y)] {
                assert!(occupied.insert(cell));
            }
            for wall in wall_grids(spec) {
                assert!(
                    occupied.insert(wall),
                    "wall intersects Tank or companion at {wall:?}"
                );
            }
        }
        assert_eq!(occupied.len(), 32);
    }

    #[test]
    fn m2_context_supports_are_legal_on_actual_terrain_without_mutation() {
        let generated = hw_world::generate_world_layout(20260920);
        let mut map = hw_world::WorldMap::default();
        for (index, &terrain) in generated.terrain_tiles.iter().enumerate() {
            map.set_terrain_at_idx(index, terrain);
        }
        let all = layout::layout(super::super::super::PerfScenarioSize::Small);
        for kind in [BuildingType::Tank, BuildingType::MudMixer] {
            let specs = all
                .iter()
                .filter(|spec| spec.kind == kind)
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(validate_layout(&specs, &map), Ok(()));
        }
        for (index, &terrain) in generated.terrain_tiles.iter().enumerate() {
            assert_eq!(map.terrain_at_idx(index), Some(terrain));
        }
    }

    #[test]
    fn context_walls_are_disjoint_before_and_after_real_footprint() {
        let mut occupied = HashSet::new();
        let specs = layout::layout(super::super::super::PerfScenarioSize::Small);
        for spec in specs
            .iter()
            .filter(|spec| spec.kind == BuildingType::RestArea)
        {
            for tile in &spec.tiles {
                assert!(occupied.insert(*tile));
            }
            let [back, front] = wall_grids(spec);
            assert!(
                spec.tiles
                    .iter()
                    .all(|tile| back.1 < tile.1 && front.1 > tile.1)
            );
            assert!(occupied.insert(back));
            assert!(occupied.insert(front));
            assert_eq!(
                preview_position(spec).truncate(),
                spec.center - Vec2::Y * 160.0
            );
        }
        assert_eq!(occupied.len(), 24);
        assert_eq!(
            layout::layout(super::super::super::PerfScenarioSize::Small).len(),
            36
        );
    }

    #[test]
    fn every_measured_change_or_disappearance_fails_without_repair() {
        let mut state = BuildingArtStaticState {
            phase: Phase::Seeded,
            ..default()
        };
        assert!(!checkpoint(&mut state, None).unwrap());
        let initial = json!({"owner": 1, "root": 2, "preview": 3, "light": 500});
        for frame in 1..=35 {
            assert_eq!(
                checkpoint(&mut state, Some(initial.clone())).unwrap(),
                frame == 30
            );
        }
        assert!(checkpoint(&mut state, None).is_err());
        for key in ["owner", "root", "preview", "light"] {
            let mut changed = initial.clone();
            changed[key] = json!(999);
            assert!(checkpoint(&mut state, Some(changed)).is_err());
            assert_eq!(state.evidence.as_ref(), Some(&initial));
            assert_eq!(state.stable_frames, 35);
        }
    }
}
