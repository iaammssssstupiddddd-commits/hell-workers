use super::*;

// ─── シーン初期化 ─────────────────────────────────────────────────────────────

#[derive(SystemParam)]
pub struct SceneRenderAssets<'w> {
    asset_server: Res<'w, AssetServer>,
    images: ResMut<'w, Assets<Image>>,
    composite_materials: ResMut<'w, Assets<LocalRttCompositeMaterial>>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

pub fn setup_scene(
    mut commands: Commands,
    mut render_assets: SceneRenderAssets,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });

    let (physical_size, target_scale_factor) = q_window
        .single()
        .map(|win| {
            (
                UVec2::new(win.physical_width().max(1), win.physical_height().max(1)),
                win.scale_factor(),
            )
        })
        .unwrap_or((UVec2::new(1280, 720), 1.0));

    // --- RtT テクスチャ ---
    let rtt_handle = render_assets.images.add(Image::new_target_texture(
        physical_size.x,
        physical_size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    let runtime = VisualTestRttRuntime {
        physical_size,
        target_scale_factor,
        scene: rtt_handle,
    };

    let cam3d_transform =
        Transform::from_xyz(0.0, VIEW_HEIGHT, Z_OFFSET).looking_at(Vec3::ZERO, Vec3::NEG_Z);

    // --- Camera3d (RtT — building/terrain presentation) ---
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -2,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        AmbientLight {
            brightness: 500.0,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        cam3d_transform,
        runtime.scene_target(),
        RenderLayers::layer(LAYER_3D),
        Camera3dRtt,
    ));

    // --- Camera2d (メイン: パン + ズーム) ---
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        RenderLayers::layer(LAYER_2D),
        TestMainCamera,
        PanCamera {
            key_rotate_ccw: None,
            key_rotate_cw: None,
            key_zoom_in: None,
            key_zoom_out: None,
            ..Default::default()
        },
    ));

    // --- Camera2d (オーバーレイ: 合成メッシュ + UI) ---
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(LAYER_OVERLAY),
    ));

    // --- RtT 合成メッシュ ---
    let win_size = q_window
        .single()
        .ok()
        .map(|win| win.size())
        .unwrap_or(Vec2::new(1280.0, 720.0));
    let comp_height = win_size.y * topdown_rtt_vertical_compensation();
    let mesh = render_assets.meshes.add(Rectangle::default().mesh());
    let composite_mat = render_assets
        .composite_materials
        .add(LocalRttCompositeMaterial {
            params: RttCompositeParams {
                pixel_size: runtime.pixel_size(),
                shadow_offset_uv: Vec2::ZERO,
                shadow_width_px: 0.0,
                shadow_strength: 0.0,
            },
            scene_texture: runtime.scene.clone(),
        });
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(composite_mat),
        Transform::from_xyz(0.0, 0.0, Z_RTT_COMPOSITE).with_scale(Vec3::new(
            win_size.x,
            comp_height,
            1.0,
        )),
        RenderLayers::layer(LAYER_OVERLAY),
        LocalRttComposite,
    ));
    commands.insert_resource(runtime);

    let font: Handle<Font> = render_assets.asset_server.load("fonts/NotoSansJP-VF.ttf");

    // --- 指向性ライト (本番相当) ---
    let sun_dir = topdown_sun_direction_world();
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 12_000.0,
            ..default()
        },
        Transform::from_translation(sun_dir * 360.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 120.0,
            maximum_distance: 500.0,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER]),
    ));

    spawn_menu_ui(&mut commands, font);
}
