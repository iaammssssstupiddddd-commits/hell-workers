//! Actual camera/mesh/image bounds, independent of ViewVisibility's coarse culling.

use super::*;
use crate::plugins::startup::{Camera3dRtt, RttCompositeMaterial, RttCompositeSprite, RttRuntime};
use bevy::{
    camera::{RenderTarget, primitives::Aabb, visibility::RenderLayers},
    sprite::Anchor,
    window::{PrimaryWindow, WindowRef},
};
use hw_core::{
    constants::{LAYER_2D, LAYER_3D, LAYER_OVERLAY},
    quality::QualitySettings,
};

#[derive(Clone)]
struct ObservedCamera {
    entity: Entity,
    camera: Camera,
    global: GlobalTransform,
}

impl ObservedCamera {
    fn clip(&self) -> Mat4 {
        self.camera.clip_from_view() * self.global.to_matrix().inverse()
    }
    fn point(&self, position: Vec3) -> Result<Vec2, String> {
        self.camera
            .world_to_viewport(&self.global, position)
            .map_err(|_| "group gallery projection unavailable".into())
    }
}

fn camera(world: &mut World, layer: usize) -> Result<ObservedCamera, String> {
    let rows = world
        .query::<(Entity, &Camera, &GlobalTransform, &RenderLayers)>()
        .iter(world)
        .filter(|(_, _, _, layers)| layers.iter().any(|value| value == layer))
        .map(|(entity, camera, global, _)| ObservedCamera {
            entity,
            camera: camera.clone(),
            global: *global,
        })
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err("group gallery camera count/layers differ".into());
    };
    if !row.camera.is_active || row.camera.viewport.is_some() {
        return Err("group camera inactive or cropped".into());
    }
    Ok(row.clone())
}

fn window_target(world: &World, camera: Entity, window: Entity) -> bool {
    matches!(
        world.get::<RenderTarget>(camera),
        Some(RenderTarget::Window(WindowRef::Primary))
    ) || matches!(world.get::<RenderTarget>(camera), Some(RenderTarget::Window(WindowRef::Entity(id))) if *id == window)
}

fn rect(points: &[Vec2]) -> Result<[f32; 4], String> {
    if points.is_empty() || points.iter().any(|p| !p.is_finite()) {
        return Err("invalid group projected points".into());
    }
    let min = points
        .iter()
        .copied()
        .fold(Vec2::splat(f32::INFINITY), Vec2::min);
    let max = points
        .iter()
        .copied()
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
    Ok([min.x, min.y, max.x, max.y])
}

fn require_area(points: &[Vec2], session: &GalleryCaptureSession) -> Result<[f32; 4], String> {
    let bounds = rect(points)?;
    let (side, area) = if session.view()
        == crate::assets::building_asset_set::acceptance::BuildingArtView::Normal
    {
        (4.0, 64.0)
    } else {
        (2.0, 16.0)
    };
    let width = bounds[2] - bounds[0];
    let height = bounds[3] - bounds[1];
    if bounds[0] < 16.0
        || bounds[1] < 16.0
        || bounds[2] > GROUP_ART_WIDTH as f32 - 16.0
        || bounds[3] > GROUP_ART_HEIGHT as f32 - 16.0
        || width < side
        || height < side
        || width * height < area
    {
        return Err("group target clipped or below minimum physical pixel area".into());
    }
    Ok(bounds)
}

fn alpha_bounds(image: &Image) -> Result<[u32; 4], String> {
    use bevy::render::render_resource::TextureFormat;
    let size = image.texture_descriptor.size;
    if size.depth_or_array_layers != 1
        || !matches!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
        )
    {
        return Err("group alpha requires an ordinary RGBA8 image".into());
    }
    let data = image.data.as_ref().ok_or("group image CPU data missing")?;
    if data.len() != size.width as usize * size.height as usize * 4 {
        return Err("group image byte count differs".into());
    }
    let mut bounds = [size.width, size.height, 0, 0];
    for (index, pixel) in data.chunks_exact(4).enumerate() {
        if pixel[3] != 0 {
            let x = index as u32 % size.width;
            let y = index as u32 / size.width;
            bounds = [
                bounds[0].min(x),
                bounds[1].min(y),
                bounds[2].max(x + 1),
                bounds[3].max(y + 1),
            ];
        }
    }
    if bounds[2] == 0 || bounds[3] == 0 {
        return Err("group image is fully transparent".into());
    }
    Ok(bounds)
}

fn owner_meshes(
    world: &mut World,
    owner: Entity,
) -> Vec<(Entity, AssetId<Mesh>, GlobalTransform, Option<Aabb>)> {
    let mut query = world.query::<(
        Entity,
        Option<&Building3dVisual>,
        Option<&ChildOf>,
        &Mesh3d,
        &GlobalTransform,
        Option<&Aabb>,
        &ViewVisibility,
    )>();
    query
        .iter(world)
        .filter(|(_, visual, parent, _, _, _, visible)| {
            visible.get()
                && visual
                    .or_else(|| parent.and_then(|p| world.get::<Building3dVisual>(p.parent())))
                    .is_some_and(|visual| visual.owner == owner)
        })
        .map(|(id, _, _, mesh, global, aabb, _)| (id, mesh.0.id(), *global, aabb.copied()))
        .collect()
}

pub(super) fn collect(world: &mut World, session: &GalleryCaptureSession) -> Result<Value, String> {
    let windows = world
        .query_filtered::<(Entity, &Window), With<PrimaryWindow>>()
        .iter(world)
        .map(|(id, w)| {
            (
                id,
                w.physical_width(),
                w.physical_height(),
                w.scale_factor(),
                w.size(),
                w.decorations,
            )
        })
        .collect::<Vec<_>>();
    let [(window_id, width, height, dpi, logical, decorated)] = windows.as_slice() else {
        return Err("group primary window count differs".into());
    };
    if (*width, *height) != (GROUP_ART_WIDTH, GROUP_ART_HEIGHT)
        || *dpi != session.scale_factor()
        || *logical != Vec2::new(*width as f32 / dpi, *height as f32 / dpi)
        || *decorated
    {
        return Err(format!(
            "group actual window/DPI differs: observed={width}x{height} dpi={dpi} logical={logical:?} decorations={decorated}; expected={}x{} dpi={} decorations=false",
            GROUP_ART_WIDTH,
            GROUP_ART_HEIGHT,
            session.scale_factor()
        ));
    }
    let ui_roots = world
        .query_filtered::<Entity, With<Node>>()
        .iter(world)
        .collect::<Vec<_>>();
    if ui_roots.iter().any(|id| {
        world
            .get::<InheritedVisibility>(*id)
            .is_none_or(|v| v.get())
    }) {
        return Err("group gallery UI occludes the capture".into());
    }
    let main = camera(world, LAYER_2D)?;
    let rtt = camera(world, LAYER_3D)?;
    let overlay = camera(world, LAYER_OVERLAY)?;
    if !world
        .entity(main.entity)
        .contains::<hw_ui::camera::MainCamera>()
        || !world.entity(rtt.entity).contains::<Camera3dRtt>()
        || !window_target(world, main.entity, *window_id)
        || !window_target(world, overlay.entity, *window_id)
        || main.camera.logical_viewport_size() != Some(*logical)
        || overlay.camera.logical_viewport_size() != Some(*logical)
    {
        return Err("group camera target/viewport differs".into());
    }
    let runtime = world.resource::<RttRuntime>();
    let quality = world.resource::<QualitySettings>().rtt;
    let pixels = Vec2::new(
        runtime.viewport.width as f32,
        runtime.viewport.height as f32,
    );
    let target_scale = runtime.target_scale_factor;
    let scene = runtime.scene.clone();
    if quality.as_str() != session.quality()
        || target_scale != quality.rtt_scale() * dpi
        || pixels != Vec2::new(*width as f32, *height as f32) * quality.rtt_scale()
        || rtt.camera.logical_viewport_size() != Some(*logical)
        || !matches!((world.get::<RenderTarget>(rtt.entity), runtime.scene_render_target()),
            (Some(RenderTarget::Image(actual)), RenderTarget::Image(expected)) if *actual == expected)
    {
        return Err("group actual quality/RTT target differs".into());
    }
    let image = world
        .resource::<Assets<Image>>()
        .get(&scene)
        .ok_or("group Scene image missing")?;
    if Vec2::new(
        image.texture_descriptor.size.width as f32,
        image.texture_descriptor.size.height as f32,
    ) != pixels
    {
        return Err("group Scene texture extent differs".into());
    }
    let composites = world
        .query_filtered::<(
            Entity,
            &GlobalTransform,
            &MeshMaterial2d<RttCompositeMaterial>,
            &ViewVisibility,
        ), With<RttCompositeSprite>>()
        .iter(world)
        .map(|(id, global, material, visible)| (id, *global, material.0.clone(), visible.get()))
        .collect::<Vec<_>>();
    let [(composite_id, composite, material, visible)] = composites.as_slice() else {
        return Err("group composite count differs".into());
    };
    if !visible
        || world
            .resource::<Assets<RttCompositeMaterial>>()
            .get(material)
            .is_none_or(|m| m.scene_texture != scene)
        || world
            .get::<RenderLayers>(*composite_id)
            .is_none_or(|l| !l.iter().eq([LAYER_OVERLAY]))
    {
        return Err("group composite not bound/visible".into());
    }
    let (scale, rotation, translation) = composite.to_scale_rotation_translation();
    if scale
        != Vec3::new(
            logical.x,
            logical.y * hw_core::constants::topdown_rtt_vertical_compensation(),
            1.0,
        )
        || rotation != Quat::IDENTITY
        || translation != Vec3::new(0.0, 0.0, hw_core::constants::Z_RTT_COMPOSITE)
    {
        return Err("group composite geometry differs".into());
    }
    let state = world.resource::<BuildingArtStaticState>();
    let selected = state
        .specs
        .iter()
        .cloned()
        .zip(state.owners.iter().copied())
        .filter(|(spec, _)| layout::group_gallery_visible(session, spec.kind))
        .collect::<Vec<_>>();
    let fixture = state
        .m6_final_evidence
        .clone()
        .ok_or("missing actual group fixture")?;
    let mut records = Vec::new();
    for (spec, owner) in selected {
        let kind = format!("{:?}", spec.kind);
        let logical_record = fixture["records"]
            .as_array()
            .ok_or("missing group records")?
            .iter()
            .find(|r| r["kind"] == kind && r["ordinal"] == spec.ordinal)
            .ok_or("missing group owner record")?;
        let structural = spec.kind == BuildingType::Door
            || crate::systems::visual::building_presentation::asset_kind(spec.kind)
                .is_some_and(|kind| !kind.mesh_roles().is_empty());
        let mut shapes = Vec::new();
        let mut screen_points = Vec::new();
        let mut texel_points = Vec::new();
        if structural {
            let meshes = owner_meshes(world, owner);
            let expected = logical_record
                .get("parts")
                .and_then(Value::as_array)
                .map_or(1, |parts| {
                    parts.iter().filter(|p| p["visible"] == true).count()
                });
            if meshes.len() != expected {
                return Err("group visible mesh/bounds inventory differs".into());
            }
            for (entity, mesh, global, aabb) in meshes {
                let aabb = aabb.ok_or("group actual mesh AABB missing")?;
                let mut corners = Vec::new();
                let mut texels = Vec::new();
                let min: Vec3 = aabb.min().into();
                let max: Vec3 = aabb.max().into();
                for x in [min.x, max.x] {
                    for y in [min.y, max.y] {
                        for z in [min.z, max.z] {
                            let point = rtt.point(global.transform_point(Vec3::new(x, y, z)))?;
                            let uv = point / *logical;
                            if !uv.is_finite() || uv.min_element() < 0.0 || uv.max_element() > 1.0 {
                                return Err("group mesh part outside RTT texture".into());
                            }
                            let composed =
                                composite.transform_point(Vec3::new(uv.x - 0.5, 0.5 - uv.y, 0.0));
                            corners.push(overlay.point(composed)? * dpi);
                            texels.push(uv * pixels);
                        }
                    }
                }
                let bounds = rect(&corners)?;
                if bounds[0] < 16.0
                    || bounds[1] < 16.0
                    || bounds[2] > *width as f32 - 16.0
                    || bounds[3] > *height as f32 - 16.0
                {
                    return Err("group mesh part is clipped".into());
                }
                screen_points.extend(corners.iter().copied());
                texel_points.extend(texels.iter().copied());
                shapes.push(json!({"type": "mesh3d", "entity": entity.to_bits(), "mesh": format!("{mesh:?}"),
                    "aabb_min": min.to_array(), "aabb_max": max.to_array(), "world_matrix": global.to_matrix().to_cols_array(),
                    "physical_corners": corners.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
                    "rtt_texel_corners": texels.iter().map(|p| p.to_array()).collect::<Vec<_>>()}));
            }
            let t = rect(&texel_points)?;
            if (t[2] - t[0]) * (t[3] - t[1]) < 16.0 {
                return Err("group 3D footprint below minimum RTT texel area".into());
            }
        } else {
            let sprites = world
                .query_filtered::<(
                    Entity,
                    &ChildOf,
                    &Sprite,
                    &GlobalTransform,
                    Option<&Anchor>,
                    &ViewVisibility,
                ), With<hw_visual::layer::VisualLayerKind>>()
                .iter(world)
                .filter(|(_, parent, _, _, _, _)| parent.parent() == owner)
                .map(|(id, _, sprite, global, anchor, visible)| {
                    (id, sprite.clone(), *global, anchor.copied(), visible.get())
                })
                .collect::<Vec<_>>();
            let [(entity, sprite, global, anchor, visible)] = sprites.as_slice() else {
                return Err("group sprite inventory differs".into());
            };
            if !visible
                || sprite.flip_x
                || sprite.flip_y
                || sprite.rect.is_some()
                || sprite.texture_atlas.is_some()
            {
                return Err("group sprite hidden, flipped or cropped".into());
            }
            let anchor = anchor.unwrap_or(Anchor::CENTER).0;
            let size = sprite.custom_size.ok_or("group sprite canvas absent")?;
            let image = world
                .resource::<Assets<Image>>()
                .get(&sprite.image)
                .ok_or("group sprite image missing")?;
            let opaque = alpha_bounds(image)?;
            let image_size = Vec2::new(
                image.texture_descriptor.size.width as f32,
                image.texture_descriptor.size.height as f32,
            );
            for x in [opaque[0], opaque[2]] {
                for y in [opaque[1], opaque[3]] {
                    let local = Vec3::new(
                        (x as f32 / image_size.x - 0.5 - anchor.x) * size.x,
                        (0.5 - y as f32 / image_size.y - anchor.y) * size.y,
                        0.0,
                    );
                    screen_points.push(main.point(global.transform_point(local))? * dpi);
                }
            }
            shapes.push(json!({"type": "sprite", "entity": entity.to_bits(), "image": format!("{:?}", sprite.image.id()),
                "world_matrix": global.to_matrix().to_cols_array(), "canvas_wu": size.to_array(), "anchor": anchor.to_array(),
                "image_size": image_size.to_array(), "opaque_bounds_px": opaque,
                "physical_corners": screen_points.iter().map(|p| p.to_array()).collect::<Vec<_>>() }));
        }
        let bounds = require_area(&screen_points, session)?;
        records.push(json!({"kind": kind, "ordinal": spec.ordinal, "owner": owner.to_bits(), "physical_bounds": bounds, "shapes": shapes}));
    }
    Ok(
        json!({"window": {"physical": [width, height], "logical": logical.to_array(), "dpi": dpi, "decorations": decorated},
        "rtt": {"physical": pixels.to_array(), "logical": rtt.camera.logical_viewport_size().map(|v| v.to_array()),
            "target_scale_factor": target_scale, "quality": quality.as_str()},
        "main_clip_from_world": main.clip().to_cols_array(), "rtt_clip_from_world": rtt.clip().to_cols_array(),
        "overlay_clip_from_world": overlay.clip().to_cols_array(), "composite_world": composite.to_matrix().to_cols_array(),
        "visible_ui_nodes": 0, "records": records}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_meshes_are_children_of_equipment_roots_not_duplicate_world_followers() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let root = world.spawn(Building3dVisual { owner }).id();
        let visible = ViewVisibility::VISIBLE;
        let mesh = world
            .spawn((
                ChildOf(root),
                Mesh3d::default(),
                GlobalTransform::IDENTITY,
                visible,
            ))
            .id();
        let other = world.spawn_empty().id();
        assert!(world.get::<Building3dVisual>(mesh).is_none());
        assert_eq!(
            owner_meshes(&mut world, owner)
                .iter()
                .map(|r| r.0)
                .collect::<Vec<_>>(),
            vec![mesh]
        );
        assert!(owner_meshes(&mut world, other).is_empty());
    }
}
