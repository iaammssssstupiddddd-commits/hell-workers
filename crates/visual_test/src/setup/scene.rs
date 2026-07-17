use super::*;

// ─── シーン初期化 ─────────────────────────────────────────────────────────────

#[derive(SystemParam)]
pub struct SceneRenderAssets<'w> {
    asset_server: Res<'w, AssetServer>,
    images: ResMut<'w, Assets<Image>>,
    character_materials: ResMut<'w, Assets<CharacterMaterial>>,
    standard_materials: ResMut<'w, Assets<StandardMaterial>>,
    soul_shadow_materials: ResMut<'w, Assets<SoulShadowMaterial>>,
    soul_mask_materials: ResMut<'w, Assets<SoulMaskMaterial>>,
    composite_materials: ResMut<'w, Assets<LocalRttCompositeMaterial>>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

pub fn setup_scene(
    mut commands: Commands,
    mut render_assets: SceneRenderAssets,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<TestState>,
) {
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });

    let (w, h) = q_window
        .single()
        .map(|win| (win.physical_width().max(1), win.physical_height().max(1)))
        .unwrap_or((1280, 720));

    // --- RtT テクスチャ ---
    let rtt_handle = render_assets.images.add(Image::new_target_texture(
        w,
        h,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    let mask_handle = render_assets.images.add(Image::new_target_texture(
        w,
        h,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));

    let cam3d_transform =
        Transform::from_xyz(0.0, VIEW_HEIGHT, Z_OFFSET).looking_at(Vec3::ZERO, Vec3::NEG_Z);

    // --- Camera3d (RtT — ソウル本体/顔) ---
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
        RenderTarget::Image(rtt_handle.clone().into()),
        RenderLayers::layer(LAYER_3D),
        Camera3dRtt,
    ));

    // --- Camera3d (マスク RtT — シルエット) ---
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        cam3d_transform,
        RenderTarget::Image(mask_handle.clone().into()),
        RenderLayers::layer(LAYER_3D_SOUL_MASK),
        Camera3dSoulMaskTest,
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
                pixel_size: Vec2::new(1.0 / win_size.x.max(1.0), 1.0 / win_size.y.max(1.0)),
                mask_radius_px: 2.25,
                mask_feather: 0.28,
                shadow_offset_uv: Vec2::ZERO,
                shadow_width_px: 0.0,
                shadow_strength: 0.0,
            },
            scene_texture: rtt_handle,
            soul_mask_texture: mask_handle,
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

    // --- アセット ---
    let soul_scene = render_assets
        .asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/characters/soul.glb"));
    let gltf_handle = render_assets
        .asset_server
        .load("models/characters/soul.glb");
    let face_atlas = render_assets
        .asset_server
        .load("textures/character/soul_face_atlas.png");
    let white_pixel = render_assets.images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ));
    let font: Handle<Font> = render_assets.asset_server.load("fonts/NotoSansJP-VF.ttf");
    let blob_shadow_mesh = render_assets.meshes.add(build_blob_shadow_mesh());
    let blob_shadow_material = render_assets.standard_materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let soul_shadow_material = render_assets
        .soul_shadow_materials
        .add(SoulShadowMaterial::default());
    let soul_mask_material = render_assets
        .soul_mask_materials
        .add(SoulMaskMaterial::solid_white());

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
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]),
    ));

    let test_assets = TestAssets {
        soul_scene: soul_scene.clone(),
        face_atlas: face_atlas.clone(),
        white_pixel: white_pixel.clone(),
        gltf_handle,
        blob_shadow_mesh,
        blob_shadow_material,
        soul_shadow_material: soul_shadow_material.clone(),
        soul_mask_material: soul_mask_material.clone(),
    };
    rebuild_soul_test_layout(
        &mut commands,
        &mut render_assets.character_materials,
        &test_assets,
        &mut state,
        SoulRebuildEntities::default(),
        SoulLayout::Default,
    );
    commands.insert_resource(test_assets);

    spawn_menu_ui(&mut commands, font);
}

fn build_blob_shadow_mesh() -> Mesh {
    let outline = blob_shadow_outline();
    let mut positions = Vec::with_capacity(outline.len() + 1);
    positions.push([0.0, 0.0, 0.0]);
    positions.extend(outline.iter().map(|p| [p.x, p.y, 0.0]));

    let mut indices = Vec::with_capacity(outline.len() * 3);
    for i in 0..outline.len() {
        let current = (i + 1) as u32;
        let next = if i + 1 == outline.len() {
            1
        } else {
            (i + 2) as u32
        };
        indices.extend_from_slice(&[0, current, next]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_normals()
}
