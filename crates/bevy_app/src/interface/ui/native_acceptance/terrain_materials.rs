//! Isolated production-material patches; telemetry and image readback stay passive.
use super::*;
use crate::plugins::startup::{
    Camera3dRtt, RttCompositeMaterial, RttCompositeSprite, RttRuntime, Terrain3dHandles,
};
use bevy::asset::AssetId;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::core_pipeline::prepass::NormalPrepass;
use bevy::diagnostic::FrameCount;
use bevy::light::NotShadowCaster;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor};
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::texture::GpuImage;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::shader::{Shader, ShaderCacheError};
use hw_core::constants::{building_3d_render_layers, topdown_rtt_vertical_compensation};
use std::sync::{Arc, Mutex};

const SHADERS: [&str; 4] = [
    "shaders/terrain_surface_material.wgsl",
    "shaders/terrain_surface_material_lod1_lite.wgsl",
    "shaders/terrain_surface_material_lod2.wgsl",
    "shaders/terrain_surface_material_prepass.wgsl",
];

#[derive(Component)]
struct Patch(usize);

#[derive(Default)]
struct Evidence {
    shaders: Vec<Handle<Shader>>,
    scene: Option<AssetId<Image>>,
    requested: Option<AssetId<Image>>,
    gpu: Value,
    readback: Value,
}

#[derive(Resource, Clone, Default)]
struct Bridge(Arc<Mutex<Evidence>>);

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("terrain-materials")
}

pub(super) fn register(app: &mut App) {
    let bridge = Bridge::default();
    app.insert_resource(bridge.clone());
    app.get_sub_app_mut(RenderApp)
        .expect("terrain acceptance needs RenderApp")
        .insert_resource(bridge)
        .add_systems(
            Render,
            observe_gpu
                .after(RenderSystems::Render)
                .before(RenderSystems::Cleanup),
        );
}

pub(super) fn prepare(world: &mut World) {
    world
        .query::<&mut Window>()
        .single_mut(world)
        .unwrap()
        .resolution
        .set_physical_resolution(1920, 1080);
    // Clear the diagnostic scene so the ordinary ground cannot masquerade as a patch.
    let hidden: Vec<_> = world
        .query_filtered::<Entity, (
            Or<(With<Mesh3d>, With<Mesh2d>, With<Sprite>, With<Text2d>)>,
            Without<RttCompositeSprite>,
        )>()
        .iter(world)
        .collect();
    for entity in hidden {
        world
            .entity_mut(entity)
            .insert((Visibility::Hidden, RenderLayers::none()));
    }
    let cameras: Vec<_> = world
        .query_filtered::<Entity, With<Camera3dRtt>>()
        .iter(world)
        .collect();
    for entity in cameras {
        world.entity_mut(entity).insert(NormalPrepass);
    }
    for (mut transform, mut camera) in world
        .query_filtered::<(&mut Transform, &mut PanCamera), With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        transform.scale = Vec3::ONE;
        camera.zoom_factor = 1.0;
    }
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::default().mesh().size(128.0, 128.0));
    let handles = world.resource::<Terrain3dHandles>();
    let (full, lite, lod2) = (
        handles.lod1.clone(),
        handles.lod1_lite.clone(),
        handles.lod2.clone(),
    );
    for index in 0..3 {
        let mut patch = world.spawn((
            Patch(index),
            Mesh3d(mesh.clone()),
            Transform::from_xyz((index as f32 - 1.0) * 180.0, 1.0, 0.0),
            building_3d_render_layers(),
            NotShadowCaster,
        ));
        match index {
            0 => {
                patch.insert(MeshMaterial3d(full.clone()));
            }
            1 => {
                patch.insert(MeshMaterial3d(lite.clone()));
            }
            _ => {
                patch.insert(MeshMaterial3d(lod2.clone()));
            }
        }
    }
    let shaders = SHADERS
        .map(|path| world.resource::<AssetServer>().load::<Shader>(path))
        .to_vec();
    world.resource::<Bridge>().0.lock().unwrap().shaders = shaders;
    world.resource_mut::<Time<Virtual>>().pause();
}

fn observe_gpu(
    bridge: Res<Bridge>,
    pipelines: Res<PipelineCache>,
    images: Res<RenderAssets<GpuImage>>,
    frame: Res<FrameCount>,
    adapter: Res<RenderAdapterInfo>,
) {
    let mut evidence = bridge.0.lock().unwrap();
    let mut resident = vec![false; evidence.shaders.len()];
    let mut errors = Vec::new();
    for pipeline in pipelines.pipelines() {
        let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor else {
            continue;
        };
        let Some(fragment) = &descriptor.fragment else {
            continue;
        };
        let Some(index) = evidence
            .shaders
            .iter()
            .position(|shader| shader.id() == fragment.shader.id())
        else {
            continue;
        };
        if index == 3 {
            let defs = format!("{:?}", fragment.shader_defs);
            if !defs.contains("NORMAL_PREPASS") || !defs.contains("PREPASS_FRAGMENT") {
                continue;
            }
        }
        match &pipeline.state {
            CachedPipelineState::Ok(_) => resident[index] = true,
            CachedPipelineState::Err(
                ShaderCacheError::ShaderNotLoaded(_)
                | ShaderCacheError::ShaderImportNotYetAvailable,
            ) => {}
            CachedPipelineState::Err(error) => {
                errors.push(format!("{}: {error:?}", SHADERS[index]))
            }
            _ => {}
        }
    }
    let image = evidence.scene.and_then(|id| images.get(id));
    evidence.gpu = json!({"frame": frame.0, "resident": resident, "errors": errors,
        "adapter": adapter.name, "backend": format!("{:?}", adapter.backend),
        "scene": evidence.scene.map(|id| format!("{id:?}")),
        "size": image.map(|image| [image.texture_descriptor.size.width, image.texture_descriptor.size.height])});
}

pub(super) fn snapshot(world: &mut World, observer: &UiObserver) -> Value {
    let Some(bridge) = world.get_resource::<Bridge>().cloned() else {
        return Value::Null;
    };
    if !observer.prepared {
        return Value::Null;
    }
    let runtime = world.resource::<RttRuntime>();
    let scene = runtime.scene.clone();
    let screenshot_target = runtime.scene_render_target();
    let size = [runtime.viewport.width, runtime.viewport.height];
    let factor = runtime.target_scale_factor;
    let quality_scale = world
        .resource::<hw_core::quality::QualitySettings>()
        .rtt_scale();
    let window = world.query::<&Window>().single(world).unwrap();
    let client = Vec2::new(
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let (camera, camera_transform, target) = world
        .query_filtered::<(&Camera, &GlobalTransform, &RenderTarget), With<Camera3dRtt>>()
        .single(world)
        .unwrap();
    let camera_bound = matches!(target, RenderTarget::Image(target) if target.handle == scene);
    let camera_scale = match target {
        RenderTarget::Image(target) => Some(target.scale_factor),
        _ => None,
    };
    let view = camera.logical_viewport_size().unwrap_or(Vec2::ONE);
    let mut patches: Vec<_> = world
        .iter_entities()
        .filter_map(|entity| {
            Some((
                entity.get::<Patch>()?,
                entity.get::<GlobalTransform>()?,
                entity.get::<ViewVisibility>()?,
            ))
        })
        .map(|(patch, transform, visible)| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for (x, z) in [(-64.0, -64.0), (-64.0, 64.0), (64.0, -64.0), (64.0, 64.0)] {
                if let Ok(point) = camera.world_to_viewport(
                    camera_transform,
                    transform.transform_point(Vec3::new(x, 0.0, z)),
                ) {
                    min = min.min(point);
                    max = max.max(point);
                }
            }
            let physical = |point: Vec2| {
                Vec2::new(
                    point.x / view.x * client.x,
                    ((point.y / view.y - 0.5) * topdown_rtt_vertical_compensation() + 0.5)
                        * client.y,
                )
            };
            (
                patch.0,
                visible.get(),
                [
                    min.x * factor,
                    min.y * factor,
                    max.x * factor,
                    max.y * factor,
                ],
                [
                    physical(min).x,
                    physical(min).y,
                    physical(max).x,
                    physical(max).y,
                ],
            )
        })
        .collect();
    patches.sort_by_key(|patch| patch.0);
    let composite = world
        .query_filtered::<&MeshMaterial2d<RttCompositeMaterial>, With<RttCompositeSprite>>()
        .single(world)
        .ok()
        .map(|material| material.0.clone());
    let composite_bound = composite.is_some_and(|handle| {
        world
            .resource::<Assets<RttCompositeMaterial>>()
            .get(&handle)
            .is_some_and(|material| material.scene_texture == scene)
    });
    let scene_id = format!("{:?}", scene.id());
    let mut evidence = bridge.0.lock().unwrap();
    evidence.scene = Some(scene.id());
    let gpu = evidence.gpu.clone();
    let ready = gpu["scene"] == scene_id
        && gpu["size"] == json!(size)
        && gpu["resident"] == json!([true, true, true, true])
        && gpu["errors"] == json!([]);
    if ready && evidence.requested != Some(scene.id()) && patches.len() == 3 {
        evidence.requested = Some(scene.id());
        let filename = format!("terrain-scene-{}.png", observer.frame);
        let path = observer.root.join(&filename);
        let boxes: Vec<_> = patches.iter().map(|patch| patch.2).collect();
        let captured_scene = scene_id.clone();
        let captured_frame = observer.frame;
        let result = bridge.clone();
        world.spawn(Screenshot(screenshot_target)).observe(save_to_disk(path))
            .observe(move |capture: On<ScreenshotCaptured>| {
                let rgba = capture.image.clone().try_into_dynamic().expect("terrain readback image").to_rgba8();
                let samples: Vec<_> = boxes.iter().map(|rect| {
                    let x = ((rect[0]+rect[2])*0.5) as u32;
                    let y = ((rect[1]+rect[3])*0.5) as u32;
                    let mut opaque = 0;
                    for dy in 0..8 { for dx in 0..8 {
                        if x+dx < rgba.width() && y+dy < rgba.height() && rgba.get_pixel(x+dx,y+dy)[3] >= 250 { opaque += 1; }
                    }}
                    opaque
                }).collect();
                result.0.lock().unwrap().readback = json!({"file": filename, "scene": captured_scene,
                    "frame": captured_frame, "size": [rgba.width(), rgba.height()], "opaque_pixels": samples});
            });
    }
    json!({"scene": scene_id, "size": size, "target_scale": factor,
        "quality_scale": quality_scale, "camera_scale": camera_scale,
        "vertical_compensation": topdown_rtt_vertical_compensation(),
        "camera_bound": camera_bound, "composite_bound": composite_bound,
        "patches": patches.iter().map(|patch| json!({"lod": patch.0, "visible": patch.1, "scene_rect": patch.2, "client_rect": patch.3})).collect::<Vec<_>>(),
        "gpu": gpu, "readback": evidence.readback})
}
