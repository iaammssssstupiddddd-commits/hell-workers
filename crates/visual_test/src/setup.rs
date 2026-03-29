use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite_render::MeshMaterial2d;
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    LAYER_2D, LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW,
    LAYER_OVERLAY, VIEW_HEIGHT, Z_OFFSET, Z_RTT_COMPOSITE,
    topdown_rtt_vertical_compensation, topdown_sun_direction_world,
};
use hw_visual::{CharacterMaterial, SoulMaskMaterial, SoulShadowMaterial};

use crate::soul::{SoulSpawnArgs, spawn_test_soul};
use crate::types::*;

// ─── シーン初期化 ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    mut soul_shadow_materials: ResMut<Assets<SoulShadowMaterial>>,
    mut soul_mask_materials: ResMut<Assets<SoulMaskMaterial>>,
    mut composite_materials: ResMut<Assets<LocalRttCompositeMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<TestState>,
) {
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });

    let (w, h) = q_window
        .single()
        .map(|win| (win.physical_width().max(1), win.physical_height().max(1)))
        .unwrap_or((1280, 720));

    // --- RtT テクスチャ ---
    let rtt_handle = images.add(Image::new_target_texture(
        w,
        h,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    let mask_handle = images.add(Image::new_target_texture(
        w,
        h,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));

    let cam3d_transform = Transform::from_xyz(0.0, VIEW_HEIGHT, Z_OFFSET)
        .looking_at(Vec3::ZERO, Vec3::NEG_Z);

    // --- Camera3d (RtT — ソウル本体/顔) ---
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -2,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        AmbientLight { brightness: 500.0, ..default() },
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
        Camera { order: 0, ..default() },
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
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
        RenderLayers::layer(LAYER_OVERLAY),
    ));

    // --- RtT 合成メッシュ ---
    let win_size = q_window
        .single()
        .ok()
        .map(|win| win.size())
        .unwrap_or(Vec2::new(1280.0, 720.0));
    let comp_height = win_size.y * topdown_rtt_vertical_compensation();
    let mesh = meshes.add(Rectangle::default().mesh());
    let composite_mat = composite_materials.add(LocalRttCompositeMaterial {
        params: RttCompositeParams {
            pixel_size: Vec2::new(1.0 / win_size.x.max(1.0), 1.0 / win_size.y.max(1.0)),
            mask_radius_px: 2.25,
            mask_feather: 0.28,
        },
        scene_texture: rtt_handle,
        soul_mask_texture: mask_handle,
    });
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(composite_mat),
        Transform::from_xyz(0.0, 0.0, Z_RTT_COMPOSITE)
            .with_scale(Vec3::new(win_size.x, comp_height, 1.0)),
        RenderLayers::layer(LAYER_OVERLAY),
        LocalRttComposite,
    ));

    // --- アセット ---
    let soul_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/characters/soul.glb"));
    let gltf_handle = asset_server.load("models/characters/soul.glb");
    let face_atlas = asset_server.load("textures/character/soul_face_atlas.png");
    let white_pixel = images.add(Image::new(
        bevy::render::render_resource::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ));
    let font: Handle<Font> = asset_server.load("fonts/NotoSansJP-VF.ttf");
    let soul_shadow_material = soul_shadow_materials.add(SoulShadowMaterial::default());
    let soul_mask_material = soul_mask_materials.add(SoulMaskMaterial::solid_white());

    // --- 指向性ライト (本番相当) ---
    let sun_dir = topdown_sun_direction_world();
    commands.spawn((
        DirectionalLight { shadows_enabled: true, illuminance: 12_000.0, ..default() },
        Transform::from_translation(sun_dir * 360.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 120.0,
            maximum_distance: 500.0,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]),
    ));

    commands.insert_resource(TestAssets {
        soul_scene: soul_scene.clone(),
        face_atlas: face_atlas.clone(),
        white_pixel: white_pixel.clone(),
        gltf_handle,
        soul_shadow_material: soul_shadow_material.clone(),
        soul_mask_material: soul_mask_material.clone(),
    });

    // --- 初期 3 Soul ---
    for i in 0..3 {
        spawn_test_soul(
            &mut commands,
            &mut character_materials,
            SoulSpawnArgs {
                soul_scene: &soul_scene,
                face_atlas: &face_atlas,
                white_pixel: &white_pixel,
                soul_shadow_material: &soul_shadow_material,
                soul_mask_material: &soul_mask_material,
                x: (i as f32 - 1.0) * SOUL_SPACING,
                z: 0.0,
                index: state.next_index,
                initial_expr: FaceExpression::Normal,
                selected: i == 0,
            },
        );
        state.next_index += 1;
        state.soul_count += 1;
    }

    spawn_menu_ui(&mut commands, font);
}

// ─── ボタンメニュー ───────────────────────────────────────────────────────────

const BTN_H: f32 = 24.0;
const BTN_GAP: f32 = 2.0;
const SFONT: f32 = 11.0;
const SEC_COL: Color = Color::Srgba(bevy::color::Srgba::new(0.55, 0.45, 0.70, 1.0));
const VAL_COL: Color = Color::Srgba(bevy::color::Srgba::new(1.00, 0.80, 0.40, 1.0));
const DIM_COL: Color = Color::Srgba(bevy::color::Srgba::new(0.65, 0.65, 0.70, 1.0));
const PANEL_BG: Color = Color::Srgba(bevy::color::Srgba::new(0.04, 0.04, 0.04, 0.82));

/// ボタン本体スポーン。w は Val::Percent(49.0) か Val::Percent(100.0) を使う。
fn spawn_btn(p: &mut ChildSpawnerCommands, a: VisualTestAction, label: &str, w: Val, font: &Handle<Font>) {
    p.spawn((
        Button,
        Node {
            width: w,
            height: Val::Px(BTN_H),
            margin: UiRect { right: Val::Px(BTN_GAP), bottom: Val::Px(BTN_GAP), ..default() },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BTN_DEF),
        a,
    ))
    .with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont { font: font.clone(), font_size: SFONT, ..default() },
            TextColor(Color::srgb(0.9, 0.9, 0.92)),
        ));
    });
}

/// 小ボタン（+/−/O など）。
fn small_btn(p: &mut ChildSpawnerCommands, a: VisualTestAction, label: &str, font: &Handle<Font>) {
    p.spawn((
        Button,
        Node {
            width: Val::Px(22.0),
            height: Val::Px(22.0),
            margin: UiRect::horizontal(Val::Px(2.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BTN_DEF),
        a,
    ))
    .with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.9, 0.9, 0.92)),
        ));
    });
}

/// セクションラベル（flex-wrap 親内で width:100% により改行）。
fn sec_label(p: &mut ChildSpawnerCommands, text: &str, font: &Handle<Font>) {
    p.spawn((
        Text::new(text),
        TextFont { font: font.clone(), font_size: 10.0, ..default() },
        TextColor(SEC_COL),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect { top: Val::Px(8.0), bottom: Val::Px(3.0), ..default() },
            ..default()
        },
    ));
}

/// 動的値テキスト（update_dynamic_texts で更新）。
fn val_text(p: &mut ChildSpawnerCommands, initial: &str, kind: DynamicTextKind, font: &Handle<Font>) {
    p.spawn((
        Text::new(initial),
        TextFont { font: font.clone(), font_size: SFONT, ..default() },
        TextColor(VAL_COL),
        Node { min_width: Val::Px(36.0), justify_content: JustifyContent::Center, ..default() },
        kind,
    ));
}

/// ラベル + [−] 値 [+] の横並び行。幅 100% で flex-wrap 親内に収まる。
fn param_row(
    p: &mut ChildSpawnerCommands, label: &str, initial: &str, kind: DynamicTextKind,
    font: &Handle<Font>, down: VisualTestAction, up: VisualTestAction,
) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        width: Val::Percent(100.0),
        margin: UiRect::bottom(Val::Px(BTN_GAP)),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Text::new(label),
            TextFont { font: font.clone(), font_size: SFONT, ..default() },
            TextColor(DIM_COL),
            Node { min_width: Val::Px(50.0), ..default() },
        ));
        small_btn(row, down, "−", font);
        val_text(row, initial, kind, font);
        small_btn(row, up, "+", font);
    });
}

fn spawn_header(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Text::new("Visual Test"),
        TextFont { font: font.clone(), font_size: 14.0, weight: FontWeight::BOLD, ..default() },
        TextColor(Color::WHITE),
        Node { margin: UiRect::bottom(Val::Px(6.0)), width: Val::Percent(100.0), ..default() },
    ));
    // モード切替ボタン (2 列)
    p.spawn(Node { flex_direction: FlexDirection::Row, width: Val::Percent(100.0), ..default() })
        .with_children(|row| {
            spawn_btn(row, VisualTestAction::SetMode(AppMode::Soul), "SOUL", Val::Percent(49.0), font);
            spawn_btn(row, VisualTestAction::SetMode(AppMode::Build), "BUILD", Val::Percent(49.0), font);
        });
}

fn spawn_camera_section(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    sec_label(p, "─ カメラ ─", font);

    // 矢視ボタン（DynamicTextKind::ViewDir でラベル更新）
    p.spawn((
        Button,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(BTN_H),
            margin: UiRect::bottom(Val::Px(BTN_GAP)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BTN_DEF),
        VisualTestAction::NextView,
    ))
    .with_children(|b| {
        b.spawn((
            Text::new("TopDown  [V]"),
            TextFont { font: font.clone(), font_size: SFONT, ..default() },
            TextColor(Color::srgb(0.9, 0.9, 0.92)),
            DynamicTextKind::ViewDir,
        ));
    });

    // HEIGHT 行
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        margin: UiRect::bottom(Val::Px(BTN_GAP)),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Text::new("H:"),
            TextFont { font: font.clone(), font_size: SFONT, ..default() },
            TextColor(DIM_COL),
            Node { min_width: Val::Px(22.0), ..default() },
        ));
        small_btn(row, VisualTestAction::HeightDown, "−", font);
        val_text(row, "150", DynamicTextKind::Height, font);
        small_btn(row, VisualTestAction::HeightUp, "+", font);
        small_btn(row, VisualTestAction::ResetElevation, "O", font);
    });

    // OFFSET 行
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        margin: UiRect::bottom(Val::Px(BTN_GAP)),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Text::new("Off:"),
            TextFont { font: font.clone(), font_size: SFONT, ..default() },
            TextColor(DIM_COL),
            Node { min_width: Val::Px(22.0), ..default() },
        ));
        small_btn(row, VisualTestAction::OffsetDown, "−", font);
        val_text(row, "90", DynamicTextKind::Offset, font);
        small_btn(row, VisualTestAction::OffsetUp, "+", font);
    });
}

/// Soul モード用セクション（SoulSectionNode でモード切替時に show/hide）。
fn spawn_soul_section(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, width: Val::Percent(100.0), ..default() },
        SoulSectionNode,
    ))
    .with_children(|s| {
        sec_label(s, "─ Soul ─", font);
        spawn_btn(s, VisualTestAction::AddSoul, "+Soul [=]", Val::Percent(49.0), font);
        spawn_btn(s, VisualTestAction::RemoveSoul, "-Soul [-]", Val::Percent(49.0), font);
        spawn_btn(s, VisualTestAction::SelectNextSoul, "Select [Tab]", Val::Percent(49.0), font);
        spawn_btn(s, VisualTestAction::ResetSoulPos, "Reset [R]", Val::Percent(49.0), font);

        sec_label(s, "─ 表情 ─", font);
        for expr in FaceExpression::ALL {
            spawn_btn(s, VisualTestAction::SetFace(expr), expr.label(), Val::Percent(49.0), font);
        }
        spawn_btn(s, VisualTestAction::SetFaceAll, "全表情 [G]", Val::Percent(100.0), font);

        sec_label(s, "─ アニメーション ─", font);
        for (i, &name) in ANIM_CLIP_NAMES.iter().enumerate() {
            spawn_btn(s, VisualTestAction::SetAnimation(i), name, Val::Percent(49.0), font);
        }

        sec_label(s, "─ モーション ─", font);
        for mode in MotionMode::ALL {
            spawn_btn(s, VisualTestAction::SetMotion(mode), mode.label(), Val::Percent(49.0), font);
        }

        sec_label(s, "─ シェーダー ─", font);
        param_row(s, "Ghost:", "1.00", DynamicTextKind::Ghost, font, VisualTestAction::GhostDown, VisualTestAction::GhostUp);
        param_row(s, "Rim:  ", "0.28", DynamicTextKind::Rim, font, VisualTestAction::RimDown, VisualTestAction::RimUp);
        param_row(s, "Post: ", "4.0", DynamicTextKind::Posterize, font, VisualTestAction::PosterizeDown, VisualTestAction::PosterizeUp);
        spawn_btn(s, VisualTestAction::ResetShader, "Reset Shader [P]", Val::Percent(100.0), font);
    });
}

/// Build モード用セクション（BuildSectionNode でモード切替時に show/hide）。
fn spawn_build_section(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Percent(100.0),
            display: Display::None, // デフォルトは Soul モード
            ..default()
        },
        BuildSectionNode,
    ))
    .with_children(|s| {
        sec_label(s, "─ 建築種別 ─", font);
        for kind in TestBuildingKind::ALL {
            spawn_btn(s, VisualTestAction::SetBuildingKind(kind), kind.label(), Val::Percent(49.0), font);
        }

        sec_label(s, "─ 配置位置 ─", font);
        s.spawn((
            Text::new("(50, 50)"),
            TextFont { font: font.clone(), font_size: SFONT, ..default() },
            TextColor(VAL_COL),
            Node { width: Val::Percent(100.0), margin: UiRect::bottom(Val::Px(4.0)), ..default() },
            DynamicTextKind::CursorPos,
        ));
        spawn_btn(s, VisualTestAction::PlaceOrRemove, "配置/削除 [Enter]", Val::Percent(100.0), font);
        spawn_btn(s, VisualTestAction::RemoveAllBuildings, "全削除 [Del]", Val::Percent(100.0), font);
    });
}

fn spawn_menu_ui(commands: &mut Commands, font: Handle<Font>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(MENU_WIDTH),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            ScrollPosition::default(),
            MenuPanel,
        ))
        .with_children(|p| {
            spawn_header(p, &font);
            spawn_camera_section(p, &font);
            spawn_soul_section(p, &font);
            spawn_build_section(p, &font);
        });

    commands.spawn((
        Text::new("[H] メニュー表示"),
        TextFont { font, font_size: 13.0, ..default() },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        Visibility::Hidden,
        MenuHint,
    ));
}
