//! Passive logical payload accounting at setup/Flush boundaries only.
//! Texture descriptor bytes are an estimate, never a GPU allocation measurement.
use bevy::{mesh::Indices, prelude::*, render::render_resource::TextureDimension};
use hw_visual::TopDownStructuralMaterial;
use serde_json::{Value, json};

const MAX_RESOURCES: usize = 65_536;

/// Observe the real post-resize scene and natural retirement before publishing
/// Ready. Never remove a texture or hold a new strong handle to make it pass.
pub(super) fn rtt_ready(
    images: &Assets<Image>,
    runtime: Option<&crate::plugins::startup::RttRuntime>,
    expected: Option<(u32, u32)>,
) -> bool {
    let (Some(runtime), Some((width, height))) = (runtime, expected) else {
        return false;
    };
    let Some(active) = images.get(&runtime.scene) else {
        return false;
    };
    let descriptor = &active.texture_descriptor;
    descriptor.label == Some("hell-workers-rtt-scene")
        && descriptor.format == bevy::render::render_resource::TextureFormat::Rgba8Unorm
        && descriptor.dimension == TextureDimension::D2
        && descriptor.size.width == width
        && descriptor.size.height == height
        && descriptor.size.depth_or_array_layers == 1
        && descriptor.mip_level_count == 1
        && descriptor.sample_count == 1
        && runtime.viewport.width == width
        && runtime.viewport.height == height
        && runtime.target_scale_factor == 1.0
        && !images.iter().any(|(id, image)| {
            image.texture_descriptor.label == Some("hell-workers-rtt-scene")
                && id != runtime.scene.id()
        })
}

/// Only used once on startup wait, and once at the initial boundary. Label and
/// extent distinguish a real old scene from an unrelated same-sized image.
pub(super) fn rtt_observation(
    images: &Assets<Image>,
    runtime: Option<&crate::plugins::startup::RttRuntime>,
    expected: Option<(u32, u32)>,
) -> Value {
    let describe = |id, image: &Image| {
        let d = &image.texture_descriptor;
        json!({"id": format!("{id:?}"), "label": d.label, "format": format!("{:?}", d.format),
            "extent": [d.size.width, d.size.height, d.size.depth_or_array_layers]})
    };
    json!({"expected_extent": expected,
        "viewport": runtime.map(|r| [r.viewport.width, r.viewport.height]),
        "target_scale_factor": runtime.map(|r| r.target_scale_factor),
        "active": runtime.and_then(|r| images.get(&r.scene).map(|image| describe(r.scene.id(), image))),
        "resident_scenes": images.iter().filter(|(_, image)| image.texture_descriptor.label == Some("hell-workers-rtt-scene"))
            .take(32).map(|(id, image)| describe(id, image)).collect::<Vec<_>>()})
}

fn add(total: &mut usize, bytes: usize) -> Result<(), String> {
    *total = total
        .checked_add(bytes)
        .ok_or("resource byte sum overflow")?;
    Ok(())
}

fn mesh_bytes(mesh: &Mesh) -> Result<usize, String> {
    let mut bytes = 0;
    for (_, attribute) in mesh
        .try_attributes()
        .map_err(|_| "mesh CPU attributes unavailable")?
    {
        add(&mut bytes, attribute.get_bytes().len())?;
    }
    let indices = mesh
        .try_indices_option()
        .map_err(|_| "mesh CPU indices unavailable")?;
    let index_bytes = match indices {
        Some(Indices::U16(values)) => values.len().checked_mul(2),
        Some(Indices::U32(values)) => values.len().checked_mul(4),
        None => Some(0),
    }
    .ok_or("mesh index byte sum overflow")?;
    add(&mut bytes, index_bytes)?;
    Ok(bytes)
}

fn texture_bytes(image: &Image) -> Result<usize, String> {
    let d = &image.texture_descriptor;
    if d.mip_level_count == 0
        || d.mip_level_count > 32
        || d.sample_count == 0
        || d.size.width == 0
        || d.size.height == 0
        || d.size.depth_or_array_layers == 0
    {
        return Err("invalid texture extent/mip/sample count".into());
    }
    let block = d
        .format
        .block_copy_size(None)
        .ok_or("texture format has no definite block byte size")?;
    let (bw, bh) = d.format.block_dimensions();
    let mut total = 0;
    for level in 0..d.mip_level_count {
        let w = d
            .size
            .width
            .checked_shr(level)
            .unwrap_or(0)
            .max(1)
            .div_ceil(bw);
        let h = d
            .size
            .height
            .checked_shr(level)
            .unwrap_or(0)
            .max(1)
            .div_ceil(bh);
        let depth = if d.dimension == TextureDimension::D3 {
            d.size
                .depth_or_array_layers
                .checked_shr(level)
                .unwrap_or(0)
                .max(1)
        } else {
            d.size.depth_or_array_layers
        };
        let bytes = [w, h, depth, block, d.sample_count]
            .into_iter()
            .try_fold(1_usize, |total, n| total.checked_mul(n as usize))
            .ok_or("texture byte product overflow")?;
        add(&mut total, bytes)?;
    }
    Ok(total)
}

pub(super) fn capture(
    meshes: &Assets<Mesh>,
    materials: &Assets<TopDownStructuralMaterial>,
    images: &Assets<Image>,
    server: &AssetServer,
) -> Result<Value, String> {
    if [meshes.len(), materials.len(), images.len()]
        .into_iter()
        .any(|n| n > MAX_RESOURCES)
    {
        return Err("resource inventory exceeds finite cap".into());
    }
    let mut mesh_entries = Vec::new();
    let mut mesh_payload = 0;
    for (id, mesh) in meshes.iter() {
        let bytes = mesh_bytes(mesh)?;
        add(&mut mesh_payload, bytes)?;
        mesh_entries.push(
            json!({"id": format!("{id:?}"), "path": server.get_path(id).map(|p| p.to_string()),
            "vertex_index_payload_bytes": bytes}),
        );
    }
    let mut image_entries = Vec::new();
    let mut image_cpu_payload = 0;
    let mut texture_estimate = 0;
    let mut images_without_cpu = 0;
    for (id, image) in images.iter() {
        let cpu_bytes = image.data.as_ref().map(Vec::len);
        match cpu_bytes {
            Some(bytes) => add(&mut image_cpu_payload, bytes)?,
            None => images_without_cpu += 1,
        }
        let estimated_bytes = texture_bytes(image)?;
        add(&mut texture_estimate, estimated_bytes)?;
        let d = &image.texture_descriptor;
        image_entries.push(json!({"id": format!("{id:?}"), "path": server.get_path(id).map(|p| p.to_string()),
            "cpu_payload_bytes": cpu_bytes, "declared_texture_payload_bytes_estimate": estimated_bytes,
            "format": format!("{:?}", d.format), "dimension": format!("{:?}", d.dimension),
            "extent": [d.size.width, d.size.height, d.size.depth_or_array_layers],
            "mip_levels": d.mip_level_count, "samples": d.sample_count}));
    }
    let mut material_entries = materials.ids().map(|id|
        json!({"id": format!("{id:?}"), "path": server.get_path(id).map(|p| p.to_string())})).collect::<Vec<_>>();
    for entries in [&mut mesh_entries, &mut image_entries, &mut material_entries] {
        entries.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    }
    let material_size = std::mem::size_of::<TopDownStructuralMaterial>();
    let material_bytes = material_size
        .checked_mul(materials.len())
        .ok_or("material shallow byte product overflow")?;
    Ok(
        json!({"schema_version": 1, "contract_id": "building-resident-payload-inventory-v1",
        "mesh_count": meshes.len(), "image_count": images.len(), "material_count": materials.len(),
        "mesh_vertex_index_payload_bytes": mesh_payload, "image_cpu_payload_bytes": image_cpu_payload,
        "images_without_cpu_backing": images_without_cpu,
        "declared_texture_payload_bytes_estimate": texture_estimate,
        "material_shallow_size": material_size, "material_shallow_bytes": material_bytes,
        "meshes": mesh_entries, "images": image_entries, "materials": material_entries}),
    )
}

pub(super) fn write_boundaries(
    directory: &std::path::Path,
    initial: Option<&Value>,
    final_value: Option<&Value>,
) -> std::io::Result<()> {
    let initial =
        initial.ok_or_else(|| std::io::Error::other("initial resource inventory missing"))?;
    let final_value = final_value
        .ok_or_else(|| std::io::Error::other("actual final resource inventory missing"))?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("building_resource_inventory.json"))?;
    serde_json::to_writer_pretty(
        file,
        &json!({"schema_version": 1,
            "contract_id": "building-resource-boundaries-v1",
            "initial_boundary": "ready-before-warmup", "final_boundary": "post-measure-flush",
            "gpu_allocation_measurement": false, "initial": initial, "final": final_value
        }),
    )
    .map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        mesh::{PrimitiveTopology, VertexAttributeValues},
        render::render_resource::{Extent3d, TextureFormat},
    };

    #[test]
    fn actual_vertex_index_payload_and_missing_cpu_are_distinct() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            VertexAttributeValues::Float32x3(vec![[0.; 3]; 3]),
        );
        mesh.insert_indices(Indices::U16(vec![0, 1, 2]));
        assert_eq!(mesh_bytes(&mesh).unwrap(), 42);
        mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
        assert_eq!(mesh_bytes(&mesh).unwrap(), 48);
        let _render_copy = mesh.take_gpu_data().unwrap();
        assert!(mesh_bytes(&mesh).is_err());
        let mut sum = usize::MAX;
        assert!(add(&mut sum, 1).is_err());
    }

    #[test]
    fn descriptor_estimate_keeps_layers_mips_samples_and_no_cpu_backing_explicit() {
        let mut image = Image::new_uninit(
            Extent3d {
                width: 8,
                height: 4,
                depth_or_array_layers: 2,
            },
            TextureDimension::D2,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.mip_level_count = 2;
        assert!(image.data.is_none());
        assert_eq!(texture_bytes(&image).unwrap(), (8 * 4 + 4 * 2) * 2 * 4);
        image.texture_descriptor.sample_count = 4;
        assert_eq!(texture_bytes(&image).unwrap(), (8 * 4 + 4 * 2) * 2 * 4 * 4);
        image.texture_descriptor.sample_count = 1;
        image.texture_descriptor.dimension = TextureDimension::D3;
        assert_eq!(texture_bytes(&image).unwrap(), (8 * 4 * 2 + 4 * 2) * 4);
        image.texture_descriptor.format = TextureFormat::Depth24Plus;
        assert!(texture_bytes(&image).is_err());
    }
    #[test]
    fn full_capture_distinguishes_cpu_absence_and_rejects_unavailable_mesh_and_format() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<TopDownStructuralMaterial>();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_uninit(
                Extent3d {
                    width: 8,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            ));
        let snapshot = |world: &World| {
            capture(
                world.resource::<Assets<Mesh>>(),
                world.resource::<Assets<TopDownStructuralMaterial>>(),
                world.resource::<Assets<Image>>(),
                world.resource::<AssetServer>(),
            )
        };
        let value = snapshot(app.world()).unwrap();
        assert_eq!(value["image_count"], 1);
        assert_eq!(value["images_without_cpu_backing"], 1);
        assert!(value["images"][0]["cpu_payload_bytes"].is_null());
        assert_eq!(value["declared_texture_payload_bytes_estimate"], 128);
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&image)
            .unwrap()
            .data = Some(vec![0; 128]);
        let value = snapshot(app.world()).unwrap();
        assert_eq!(value["image_cpu_payload_bytes"], 128);
        assert_eq!(value["images_without_cpu_backing"], 0);
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&image)
            .unwrap()
            .texture_descriptor
            .format = TextureFormat::Depth24Plus;
        assert!(snapshot(app.world()).is_err());
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&image)
            .unwrap()
            .texture_descriptor
            .format = TextureFormat::Rgba8Unorm;
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.; 3]; 3]);
        let _render_copy = mesh.take_gpu_data().unwrap();
        let _mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        assert!(snapshot(app.world()).is_err());
    }

    #[test]
    fn actual_final_is_required_before_any_sidecar_write() {
        let directory = std::env::temp_dir().join(format!(
            "tak14-resource-final-missing-{}",
            std::process::id()
        ));
        assert!(!directory.exists());
        assert!(write_boundaries(&directory, Some(&json!({"initial": true})), None).is_err());
        assert!(!directory.exists());
        assert!(write_boundaries(&directory, None, Some(&json!({"final": true}))).is_err());
        assert!(!directory.exists());
    }

    #[test]
    fn texture_arithmetic_overflow_is_not_zero() {
        let image = Image::new_uninit(
            Extent3d {
                width: u32::MAX,
                height: u32::MAX,
                depth_or_array_layers: u32::MAX,
            },
            TextureDimension::D2,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        assert!(texture_bytes(&image).is_err());
    }
    #[test]
    fn rtt_readiness_waits_for_natural_old_scene_retirement_without_retaining_it() {
        use crate::plugins::startup::{RttRuntime, RttViewportSize};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>();
        let runtime = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            let mut runtime = RttRuntime::new(
                RttViewportSize {
                    width: 256,
                    height: 144,
                },
                1.0,
                &mut images,
            );
            runtime.recreate(
                RttViewportSize {
                    width: 128,
                    height: 72,
                },
                1.0,
                &mut images,
            );
            runtime
        };
        app.insert_resource(runtime);
        // Ready inspection does not delete the old asset itself or create a new
        // strong handle. Bevy's normal dropped-handle processing must retire it.
        assert!(!rtt_ready(
            app.world().resource::<Assets<Image>>(),
            Some(app.world().resource::<RttRuntime>()),
            Some((128, 72))
        ));
        let observation = rtt_observation(
            app.world().resource::<Assets<Image>>(),
            Some(app.world().resource::<RttRuntime>()),
            Some((128, 72)),
        );
        assert_eq!(observation["resident_scenes"].as_array().unwrap().len(), 2);
        for _ in 0..3 {
            app.update();
        }
        assert!(rtt_ready(
            app.world().resource::<Assets<Image>>(),
            Some(app.world().resource::<RttRuntime>()),
            Some((128, 72))
        ));
    }

    #[test]
    fn rtt_readiness_requires_exact_active_target_and_waits_for_same_sized_old_generation() {
        use crate::plugins::startup::{RttRuntime, RttViewportSize};
        let mut images = Assets::<Image>::default();
        let mut runtime = RttRuntime::new(
            RttViewportSize {
                width: 128,
                height: 72,
            },
            1.0,
            &mut images,
        );
        assert!(rtt_ready(&images, Some(&runtime), Some((128, 72))));
        assert!(!rtt_ready(&images, None, Some((128, 72))));
        assert!(!rtt_ready(&images, Some(&runtime), None));
        assert!(!rtt_ready(&images, Some(&runtime), Some((256, 144))));
        runtime.target_scale_factor = 2.0;
        assert!(!rtt_ready(&images, Some(&runtime), Some((128, 72))));
        runtime.target_scale_factor = 1.0;
        runtime.viewport.width = 129;
        assert!(!rtt_ready(&images, Some(&runtime), Some((128, 72))));
        runtime.viewport.width = 128;
        images
            .get_mut(&runtime.scene)
            .unwrap()
            .texture_descriptor
            .label = None;
        assert!(!rtt_ready(&images, Some(&runtime), Some((128, 72))));
        images
            .get_mut(&runtime.scene)
            .unwrap()
            .texture_descriptor
            .label = Some("hell-workers-rtt-scene");
        images
            .get_mut(&runtime.scene)
            .unwrap()
            .texture_descriptor
            .format = TextureFormat::Rgba8UnormSrgb;
        assert!(!rtt_ready(&images, Some(&runtime), Some((128, 72))));
        images
            .get_mut(&runtime.scene)
            .unwrap()
            .texture_descriptor
            .format = TextureFormat::Rgba8Unorm;
        let old = runtime.scene.clone();
        runtime.recreate(
            RttViewportSize {
                width: 128,
                height: 72,
            },
            1.0,
            &mut images,
        );
        assert!(!rtt_ready(&images, Some(&runtime), Some((128, 72))));
        images.remove(old.id());
        let unrelated = images.add(Image::new_target_texture(
            256,
            144,
            TextureFormat::Rgba8Unorm,
            None,
        ));
        assert!(images.get(&unrelated).is_some());
        assert!(rtt_ready(&images, Some(&runtime), Some((128, 72))));
        images.remove(runtime.scene.id());
        assert!(!rtt_ready(&images, Some(&runtime), Some((128, 72))));
    }
}
