use super::asset_catalog::create_game_assets;
use super::rtt_setup::{
    self, Camera3dRtt, Camera3dSoulMaskRtt, RttDirectionalLight, RttExtraDirectionalLight,
};
use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::plugins::startup::Terrain3dHandles;
use crate::plugins::startup::{PerfScenarioConfig, PerfScenarioRandomStreams};
use crate::systems::logistics::{ResourceItem, initial_resource_spawner};
use crate::systems::visual::camera_sync::WorldForeground2dCamera;
use crate::systems::visual::elevation_view::ElevationDirection;
use crate::world::map::{
    GeneratedWorldLayoutResource, WorldMapRead, WorldMapWrite,
    prepare_generated_world_layout_resource, spawn_map, spawn_terrain_chunks,
};
use crate::world::regrowth::{RegrowthManager, configure_regrowth_from_generated_layout};
use bevy::camera::visibility::RenderLayers;
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    LAYER_2D, LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW,
    LAYER_OVERLAY, VIEW_HEIGHT, Z_OFFSET, topdown_sun_direction_world,
};
use hw_core::quality::QualitySettings;
use hw_spatial::{ResourceSpatialGrid, SpatialGridOps};
use hw_ui::camera::MainCamera;

pub(super) fn spawn_map_timed(
    commands: Commands,
    world_map: WorldMapWrite,
    generated_layout: Res<GeneratedWorldLayoutResource>,
) {
    spawn_map(commands, world_map, generated_layout);
}

pub(super) fn spawn_terrain_chunks_timed(
    commands: Commands,
    terrain_handles: Res<Terrain3dHandles>,
    meshes: ResMut<Assets<Mesh>>,
) {
    spawn_terrain_chunks(commands, terrain_handles, meshes);
}

pub(super) fn initial_resource_spawner_timed(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: WorldMapWrite,
    generated_layout: Res<GeneratedWorldLayoutResource>,
    mut regrowth: ResMut<RegrowthManager>,
) {
    configure_regrowth_from_generated_layout(&mut regrowth, &generated_layout.layout);
    initial_resource_spawner(commands, game_assets, world_map, &generated_layout);
}

/// Phase 5: camera/resources 初期化 + asset catalog 生成を呼び出す
pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    quality: Res<QualitySettings>,
    perf_toggles: Res<crate::RenderPerfToggles>,
    perf_config: Res<PerfScenarioConfig>,
) {
    // 4096 は GPU コスト・VRAM 消費が過大なため 2048 に下げる。
    // shadow 品質プリセット化は別タスク（docs/plans/shadow-map-size-2026-04-10.md 参照）。
    commands.insert_resource(DirectionalLightShadowMap { size: 2048 });
    let generated_layout = prepare_generated_world_layout_resource(&perf_config);
    info!(
        "BEVY_STARTUP: Prepared worldgen layout (seed={}, attempt={}, fallback={})",
        generated_layout.master_seed,
        generated_layout.layout.generation_attempt,
        generated_layout.layout.used_fallback
    );
    commands.insert_resource(generated_layout);

    // --- RtT オフスクリーンテクスチャ生成 ---
    let runtime = rtt_setup::initialize_rtt_runtime(q_window.single().ok(), *quality, &mut images);
    let rtt_target = runtime.scene_render_target();
    let soul_mask_target = runtime.soul_mask_render_target();
    commands.insert_resource(runtime);

    // --- Camera2d（既存: メイン描画・スクリーン出力） ---
    commands.spawn((
        Camera2d,
        MainCamera,
        gameplay_pan_camera(),
        RenderLayers::layer(LAYER_2D),
    ));

    // --- Overlay Camera（常時アクティブ: RtT composite sprite 専用）---
    // 矢視モードで MainCamera を無効化しても composite sprite を表示し続ける。
    // order=1: MainCamera(order=0) の後に描画することで上書き合成する。
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        RenderLayers::layer(LAYER_OVERLAY),
    ));

    // --- World Foreground Camera（2D ワールドオブジェクト前面描画）---
    // テレインが Camera3d → RtT に移行したことで composite sprite(order=1) が地面全体を覆う。
    // 木・石・ファミリアなどの 2D Sprite が隠れないよう、composite の後(order=2)で
    // LAYER_2D を再描画する。クリアなしで既存描画に上書きする。
    commands.spawn((
        Camera2d,
        Camera {
            order: 2,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(LAYER_2D),
        WorldForeground2dCamera,
    ));

    // --- Camera3d（RtT: オフスクリーン3D描画）---
    // TopDown の初期値は camera_sync.rs の定数と揃える。
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            is_active: true,
            ..default()
        },
        AmbientLight {
            brightness: 500.0,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            near: -2000.0,
            far: 2000.0,
            ..OrthographicProjection::default_3d()
        }),
        {
            let mut transform = Transform::from_translation(Vec3::new(0.0, VIEW_HEIGHT, Z_OFFSET));
            transform.rotation = ElevationDirection::TopDown.camera_rotation();
            transform
        },
        rtt_target,
        RenderLayers::layer(LAYER_3D),
        Camera3dRtt,
    ));

    // 3D RtT の影確認用ライト。AmbientLight だけでは shadow caster 分離を確認できないので、
    // 壁面にも影が出る向きから DirectionalLight を 1 本入れる。
    let sun_dir = topdown_sun_direction_world();
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: perf_toggles.directional_light_enabled,
            illuminance: if perf_toggles.directional_light_enabled {
                12_000.0
            } else {
                0.0
            },
            ..default()
        },
        Transform::from_translation(sun_dir * 360.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 120.0,
            maximum_distance: 500.0,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]),
        RttDirectionalLight,
    ));

    let extra_sun_dir = Vec3::new(-0.66, 0.46, 0.59).normalize();
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: perf_toggles.extra_directional_light_enabled,
            illuminance: if perf_toggles.extra_directional_light_enabled {
                8_000.0
            } else {
                0.0
            },
            ..default()
        },
        Transform::from_translation(extra_sun_dir * 360.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 120.0,
            maximum_distance: 500.0,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]),
        RttExtraDirectionalLight,
    ));

    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -2,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            is_active: perf_toggles.soul_mask_enabled,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            near: -2000.0,
            far: 2000.0,
            ..OrthographicProjection::default_3d()
        }),
        {
            let mut transform = Transform::from_translation(Vec3::new(0.0, VIEW_HEIGHT, Z_OFFSET));
            transform.rotation = ElevationDirection::TopDown.camera_rotation();
            transform
        },
        soul_mask_target,
        RenderLayers::layer(LAYER_3D_SOUL_MASK),
        Camera3dSoulMaskRtt,
    ));

    // --- asset catalog 生成 ---
    let game_assets = create_game_assets(&asset_server, &mut images);
    commands.insert_resource(game_assets);
}

fn gameplay_pan_camera() -> PanCamera {
    PanCamera {
        // Camera3d/RtT は固定 TopDown 姿勢を正本とするため、Camera2d だけを
        // 回す既定 Q/E 入力は無効化する。
        key_rotate_ccw: None,
        key_rotate_cw: None,
        ..default()
    }
}

pub(super) fn initialize_gizmo_config(mut config_store: ResMut<GizmoConfigStore>) {
    for (_, config, _) in config_store.iter_mut() {
        config.enabled = false;
        config.line.width = 1.0;
    }
}

pub(super) fn populate_resource_spatial_grid(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources: Query<(Entity, &Transform, Option<&Visibility>), With<ResourceItem>>,
) {
    for (entity, transform, visibility) in q_resources.iter() {
        let should_register = visibility
            .map(|v| *v != bevy::prelude::Visibility::Hidden)
            .unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
        }
    }
}

pub(super) fn spawn_entities(
    spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
    world_map: WorldMapRead,
    perf_config: Res<PerfScenarioConfig>,
    perf_rngs: ResMut<PerfScenarioRandomStreams>,
) {
    spawn_damned_souls(spawn_events, world_map, perf_config, perf_rngs);
}

pub(super) fn spawn_familiar_wrapper(
    spawn_events: MessageWriter<FamiliarSpawnEvent>,
    perf_config: Res<PerfScenarioConfig>,
    perf_rngs: ResMut<PerfScenarioRandomStreams>,
) {
    crate::entities::familiar::spawn_familiar(spawn_events, perf_config, perf_rngs);
}

#[cfg(test)]
mod tests {
    use super::super::rtt_composite::{self, RttCompositeMaterial, RttCompositeSprite};
    use super::*;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::camera::RenderTarget;
    use bevy::render::render_resource::TextureFormat;
    use bevy::sprite_render::MeshMaterial2d;
    use bevy::window::WindowResolution;

    #[test]
    fn gameplay_pan_camera_disables_rotation_that_rtt_cannot_follow() {
        let controller = gameplay_pan_camera();

        assert_eq!(controller.key_rotate_ccw, None);
        assert_eq!(controller.key_rotate_cw, None);
    }

    #[test]
    fn current_rtt_startup_inventory_is_explicit() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Mesh>()
            .init_asset::<Font>()
            .init_asset::<Gltf>()
            .init_asset::<WorldAsset>()
            .init_asset::<RttCompositeMaterial>()
            .insert_resource(QualitySettings::default())
            .insert_resource(crate::RenderPerfToggles::gpu_baseline())
            .insert_resource(PerfScenarioConfig::default())
            .add_systems(
                Update,
                (setup, rtt_composite::spawn_rtt_composite_sprite).chain(),
            );
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1920, 1080).with_scale_factor_override(1.0),
                ..default()
            },
            PrimaryWindow,
        ));

        app.update();

        let world = app.world_mut();
        let camera_2d_count = world
            .query_filtered::<Entity, With<Camera2d>>()
            .iter(world)
            .count();
        let camera_3d_count = world
            .query_filtered::<Entity, With<Camera3d>>()
            .iter(world)
            .count();
        let main_camera_count = world
            .query_filtered::<Entity, With<MainCamera>>()
            .iter(world)
            .count();
        let foreground_camera_count = world
            .query_filtered::<Entity, With<WorldForeground2dCamera>>()
            .iter(world)
            .count();
        let main_rtt_count = world
            .query_filtered::<Entity, With<Camera3dRtt>>()
            .iter(world)
            .count();
        let mask_rtt_count = world
            .query_filtered::<Entity, With<Camera3dSoulMaskRtt>>()
            .iter(world)
            .count();
        let directional_count = world
            .query_filtered::<Entity, With<DirectionalLight>>()
            .iter(world)
            .count();
        let composite_count = world
            .query_filtered::<Entity, With<RttCompositeSprite>>()
            .iter(world)
            .count();

        assert_eq!(camera_2d_count, 3);
        assert_eq!(camera_3d_count, 2);
        assert_eq!(main_camera_count, 1);
        assert_eq!(foreground_camera_count, 1);
        assert_eq!(main_rtt_count, 1);
        assert_eq!(mask_rtt_count, 1);
        assert_eq!(directional_count, 2);
        assert_eq!(composite_count, 1);

        let mut main_2d_query =
            world.query_filtered::<(&Camera, &RenderLayers), (With<Camera2d>, With<MainCamera>)>();
        let (main_2d, main_2d_layers) = main_2d_query.single(world).unwrap();
        assert_eq!(main_2d.order, 0);
        assert!(main_2d.is_active);
        assert_eq!(*main_2d_layers, RenderLayers::layer(LAYER_2D));

        let mut overlay_query = world.query_filtered::<(&Camera, &RenderLayers), (
            With<Camera2d>,
            Without<MainCamera>,
            Without<WorldForeground2dCamera>,
        )>();
        let (overlay, overlay_layers) = overlay_query.single(world).unwrap();
        assert_eq!(overlay.order, 1);
        assert!(overlay.is_active);
        assert_eq!(*overlay_layers, RenderLayers::layer(LAYER_OVERLAY));

        let mut foreground_query = world.query_filtered::<
            (&Camera, &RenderLayers),
            (With<Camera2d>, With<WorldForeground2dCamera>),
        >();
        let (foreground, foreground_layers) = foreground_query.single(world).unwrap();
        assert_eq!(foreground.order, 2);
        assert!(foreground.is_active);
        assert!(matches!(foreground.clear_color, ClearColorConfig::None));
        assert_eq!(*foreground_layers, RenderLayers::layer(LAYER_2D));

        let layer_2d_pass_count = world
            .query_filtered::<&RenderLayers, With<Camera2d>>()
            .iter(world)
            .filter(|layers| **layers == RenderLayers::layer(LAYER_2D))
            .count();
        assert_eq!(layer_2d_pass_count, 2);

        let (viewport, scene, soul_mask) = {
            let runtime = world.resource::<rtt_setup::RttRuntime>();
            (
                runtime.viewport,
                runtime.scene.clone(),
                runtime.soul_mask.clone(),
            )
        };
        assert_eq!(viewport.width, 1920);
        assert_eq!(viewport.height, 1080);
        assert_ne!(scene, soul_mask);

        let mut main_target_query =
            world.query_filtered::<(&Camera, &RenderLayers, &RenderTarget), With<Camera3dRtt>>();
        let (main_camera, main_layers, RenderTarget::Image(main_target)) =
            main_target_query.single(world).unwrap()
        else {
            panic!("main RtT camera must target an image");
        };
        assert_eq!(main_camera.order, -1);
        assert!(main_camera.is_active);
        assert_eq!(*main_layers, RenderLayers::layer(LAYER_3D));
        assert_eq!(main_target.handle, scene);

        let mut mask_target_query = world
            .query_filtered::<(&Camera, &RenderLayers, &RenderTarget), With<Camera3dSoulMaskRtt>>();
        let (mask_camera, mask_layers, RenderTarget::Image(mask_target)) =
            mask_target_query.single(world).unwrap()
        else {
            panic!("mask RtT camera must target an image");
        };
        assert_eq!(mask_camera.order, -2);
        assert!(mask_camera.is_active);
        assert_eq!(*mask_layers, RenderLayers::layer(LAYER_3D_SOUL_MASK));
        assert_eq!(mask_target.handle, soul_mask);

        let expected_light_layers =
            RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]);
        let mut primary_light_query =
            world.query_filtered::<(&DirectionalLight, &RenderLayers), With<RttDirectionalLight>>();
        let (primary_light, primary_light_layers) = primary_light_query.single(world).unwrap();
        assert!(primary_light.shadow_maps_enabled);
        assert_eq!(primary_light.illuminance, 12_000.0);
        assert_eq!(*primary_light_layers, expected_light_layers);

        let mut extra_light_query = world
            .query_filtered::<(&DirectionalLight, &RenderLayers), With<RttExtraDirectionalLight>>();
        let (extra_light, extra_light_layers) = extra_light_query.single(world).unwrap();
        assert!(!extra_light.shadow_maps_enabled);
        assert_eq!(extra_light.illuminance, 0.0);
        assert_eq!(*extra_light_layers, expected_light_layers);

        let mut composite_query = world.query_filtered::<(
            &MeshMaterial2d<RttCompositeMaterial>,
            &RenderLayers,
            &Visibility,
        ), With<RttCompositeSprite>>();
        let (material_handle, composite_layers, visibility) =
            composite_query.single(world).unwrap();
        assert_eq!(*composite_layers, RenderLayers::layer(LAYER_OVERLAY));
        assert_eq!(*visibility, Visibility::Visible);
        let material_handle = material_handle.0.clone();
        let material = world
            .resource::<Assets<RttCompositeMaterial>>()
            .get(&material_handle)
            .unwrap();
        assert_eq!(material.scene_texture, scene);
        assert_eq!(material.soul_mask_texture, soul_mask);

        let images = world.resource::<Assets<Image>>();
        for handle in [&scene, &soul_mask] {
            let image = images.get(handle).unwrap();
            assert_eq!(image.texture_descriptor.size.width, 1920);
            assert_eq!(image.texture_descriptor.size.height, 1080);
            assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 1);
            assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
            assert_eq!(image.data.as_ref().map(Vec::len), Some(8_294_400));
        }
    }
}
