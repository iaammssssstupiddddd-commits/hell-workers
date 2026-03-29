use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    LAYER_2D, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, Z_BUILDING_FLOOR, Z_BUILDING_STRUCT,
    Z_MAP, Z_MAP_DIRT, Z_MAP_GRASS, Z_MAP_SAND, building_3d_render_layers,
};
use hw_world::{SAND_WIDTH, TerrainType, generate_base_terrain_tiles, grid_to_world, world_to_grid};

use crate::types::*;

// ─── マーカー / コンポーネント ────────────────────────────────────────────────

/// visual_test でスポーンされた建築物 2D エンティティを識別するマーカー。
#[derive(Component)]
pub struct TestBuilding {
    pub kind: TestBuildingKind,
    pub grid: (i32, i32),
}

/// 建築物 3D ビジュアルエンティティを識別するマーカー（2D と座標で対応）。
#[derive(Component)]
pub struct TestBuilding3dVisual {
    pub owner_grid: (i32, i32),
}

/// 建築カーソル（配置プレビュー）マーカー。
#[derive(Component)]
pub struct TestBuildingCursor;

// ─── リソース ─────────────────────────────────────────────────────────────────

/// 建築物 2D テクスチャのハンドル。
#[derive(Resource)]
pub struct TestBuildingAssets {
    pub wall: Handle<Image>,
    pub door: Handle<Image>,
    pub floor: Handle<Image>,
    pub tank: Handle<Image>,
    pub mud_mixer: Handle<Image>,
    pub rest_area: Handle<Image>,
    pub bridge: Handle<Image>,
    pub sand_pile: Handle<Image>,
    pub bone_pile: Handle<Image>,
    pub wheelbarrow_parking: Handle<Image>,
}

impl TestBuildingAssets {
    pub fn image_for(&self, kind: TestBuildingKind) -> Handle<Image> {
        match kind {
            TestBuildingKind::Wall => self.wall.clone(),
            TestBuildingKind::Door => self.door.clone(),
            TestBuildingKind::Floor => self.floor.clone(),
            TestBuildingKind::Tank => self.tank.clone(),
            TestBuildingKind::MudMixer => self.mud_mixer.clone(),
            TestBuildingKind::RestArea => self.rest_area.clone(),
            TestBuildingKind::Bridge => self.bridge.clone(),
            TestBuildingKind::SandPile => self.sand_pile.clone(),
            TestBuildingKind::BonePile => self.bone_pile.clone(),
            TestBuildingKind::WheelbarrowParking => self.wheelbarrow_parking.clone(),
            TestBuildingKind::SoulSpa => self.rest_area.clone(),
        }
    }

    pub fn size_for(kind: TestBuildingKind) -> Vec2 {
        match kind {
            TestBuildingKind::Bridge => Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
            TestBuildingKind::Tank
            | TestBuildingKind::MudMixer
            | TestBuildingKind::RestArea
            | TestBuildingKind::WheelbarrowParking
            | TestBuildingKind::SoulSpa => Vec2::splat(TILE_SIZE * 2.0),
            _ => Vec2::splat(TILE_SIZE),
        }
    }
}

/// 建築物 3D ビジュアルのメッシュ・マテリアルハンドル。
#[derive(Resource)]
pub struct TestBuilding3dHandles {
    pub wall_mesh: Handle<Mesh>,
    pub wall_material: Handle<StandardMaterial>,
    pub floor_mesh: Handle<Mesh>,
    pub floor_material: Handle<StandardMaterial>,
    pub door_mesh: Handle<Mesh>,
    pub door_material: Handle<StandardMaterial>,
    pub equipment_1x1_mesh: Handle<Mesh>,
    pub equipment_2x2_mesh: Handle<Mesh>,
    pub equipment_material: Handle<StandardMaterial>,
    pub render_layers: RenderLayers,
}

impl TestBuilding3dHandles {
    /// 建築種別から (メッシュ, マテリアル, 高さ) を返す。Bridge は None（2D のみ）。
    pub fn mesh_material_height(
        &self,
        kind: TestBuildingKind,
    ) -> Option<(Handle<Mesh>, Handle<StandardMaterial>, f32)> {
        match kind {
            TestBuildingKind::Wall => {
                Some((self.wall_mesh.clone(), self.wall_material.clone(), TILE_SIZE))
            }
            TestBuildingKind::Door => {
                Some((self.door_mesh.clone(), self.door_material.clone(), TILE_SIZE * 0.5))
            }
            TestBuildingKind::Floor => {
                Some((self.floor_mesh.clone(), self.floor_material.clone(), 0.0))
            }
            TestBuildingKind::SandPile | TestBuildingKind::BonePile => Some((
                self.equipment_1x1_mesh.clone(),
                self.equipment_material.clone(),
                TILE_SIZE * 0.6,
            )),
            TestBuildingKind::WheelbarrowParking => Some((
                self.equipment_1x1_mesh.clone(),
                self.equipment_material.clone(),
                TILE_SIZE * 0.6,
            )),
            TestBuildingKind::Tank
            | TestBuildingKind::MudMixer
            | TestBuildingKind::RestArea
            | TestBuildingKind::SoulSpa => Some((
                self.equipment_2x2_mesh.clone(),
                self.equipment_material.clone(),
                TILE_SIZE * 0.8,
            )),
            TestBuildingKind::Bridge => None,
        }
    }
}

// ─── 初期化 ───────────────────────────────────────────────────────────────────

/// テクスチャ・メッシュ・マテリアルハンドルを読み込みリソースとして登録する。
pub fn setup_building_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TestBuildingAssets {
        wall: asset_server.load("textures/buildings/mud_wall/mud_wall_isolated.png"),
        door: asset_server.load("textures/buildings/door/door_closed.png"),
        floor: asset_server.load("textures/terrain/mud_floor.png"),
        tank: asset_server.load("textures/buildings/tank/empty_tank.png"),
        mud_mixer: asset_server.load("textures/buildings/mud_mixer/mud mixer.png"),
        rest_area: asset_server.load("textures/buildings/rest_area/barrack.png"),
        bridge: asset_server.load("textures/buildings/bridge/bridge.png"),
        sand_pile: asset_server.load("textures/resources/sandpile/sandpile.png"),
        bone_pile: asset_server.load("textures/resources/bone_pile/bone_pile.png"),
        wheelbarrow_parking: asset_server
            .load("textures/items/wheel_barrow/wheel_barrow_parking.png"),
    });

    commands.insert_resource(TestBuilding3dHandles {
        wall_mesh: meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE)),
        wall_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.60, 0.50, 0.38),
            perceptual_roughness: 0.9,
            ..default()
        }),
        floor_mesh: meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)),
        floor_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.35, 0.25),
            perceptual_roughness: 1.0,
            ..default()
        }),
        door_mesh: meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE * 0.5, TILE_SIZE)),
        door_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.40, 0.30, 0.18),
            perceptual_roughness: 0.7,
            ..default()
        }),
        equipment_1x1_mesh: meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE * 0.6, TILE_SIZE)),
        equipment_2x2_mesh: meshes.add(Cuboid::new(
            TILE_SIZE * 2.0,
            TILE_SIZE * 0.8,
            TILE_SIZE * 2.0,
        )),
        equipment_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.40, 0.42, 0.52),
            perceptual_roughness: 0.6,
            ..default()
        }),
        render_layers: building_3d_render_layers(),
    });

    // カーソルスプライト（配置プレビュー）を常駐エンティティとして生成。
    // 初期位置は TestState::default の building_cursor (50,50) と一致させる。
    let initial_pos = grid_to_world(50, 50);
    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 1.0, 0.2, 0.40),
            custom_size: Some(Vec2::splat(TILE_SIZE)),
            ..default()
        },
        Transform::from_xyz(initial_pos.x, initial_pos.y, 0.25),
        RenderLayers::layer(LAYER_2D),
        TestBuildingCursor,
        Name::new("TestBuildingCursor"),
    ));
}

// ─── スポーン / デスポーン ─────────────────────────────────────────────────────

/// 指定グリッド位置に建築物の 2D スプライトと 3D ビジュアルをスポーンする。
pub fn spawn_test_building(
    commands: &mut Commands,
    kind: TestBuildingKind,
    grid: (i32, i32),
    assets: &TestBuildingAssets,
    handles_3d: &TestBuilding3dHandles,
) {
    let pos2d = grid_to_world(grid.0, grid.1);
    let z = match kind {
        TestBuildingKind::Floor
        | TestBuildingKind::SandPile
        | TestBuildingKind::BonePile => Z_BUILDING_FLOOR,
        _ => Z_BUILDING_STRUCT,
    };

    commands.spawn((
        Sprite {
            image: assets.image_for(kind),
            custom_size: Some(TestBuildingAssets::size_for(kind)),
            ..default()
        },
        Transform::from_xyz(pos2d.x, pos2d.y, z),
        RenderLayers::layer(LAYER_2D),
        TestBuilding { kind, grid },
        Name::new(format!("TestBuilding {:?} ({},{})", kind, grid.0, grid.1)),
    ));

    if let Some((mesh, material, height)) = handles_3d.mesh_material_height(kind) {
        let center_y = height / 2.0;
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(pos2d.x, center_y, -pos2d.y),
            handles_3d.render_layers.clone(),
            TestBuilding3dVisual { owner_grid: grid },
            Name::new(format!("TestBuilding3dVisual ({},{})", grid.0, grid.1)),
        ));
    }
}

/// 指定グリッド位置の建築物（2D + 3D）をデスポーンする。
pub fn despawn_test_building_at(
    commands: &mut Commands,
    grid: (i32, i32),
    q_buildings: &Query<(Entity, &TestBuilding)>,
    q_3d: &Query<(Entity, &TestBuilding3dVisual)>,
) {
    for (entity, b) in q_buildings.iter() {
        if b.grid == grid {
            commands.entity(entity).despawn();
        }
    }
    for (entity, v) in q_3d.iter() {
        if v.owner_grid == grid {
            commands.entity(entity).despawn();
        }
    }
}

// ─── カーソル / ゴースト更新 ──────────────────────────────────────────────────

type CursorQuery<'w, 's> =
    Query<'w, 's, (&'static mut Transform, &'static mut Sprite), With<TestBuildingCursor>>;

type GhostCamQuery<'w, 's> =
    Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<TestMainCamera>>;

/// Build モード時にマウス追従のゴーストプレビューを表示し、左クリックで配置・削除する。
/// Soul モード時はカーソルを非表示にする。
#[allow(clippy::too_many_arguments)]
pub fn update_building_cursor(
    mut state: ResMut<TestState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: GhostCamQuery,
    building_assets: Option<Res<TestBuildingAssets>>,
    building_3d_handles: Option<Res<TestBuilding3dHandles>>,
    q_buildings: Query<(Entity, &TestBuilding)>,
    q_building_3d: Query<(Entity, &TestBuilding3dVisual)>,
    mut q_cursor: CursorQuery,
    mut commands: Commands,
) {
    let Ok((mut tf, mut sprite)) = q_cursor.single_mut() else { return };

    // Soul モード: ゴーストを画面外に退避
    if state.mode != AppMode::Build {
        tf.translation.z = -999.0;
        return;
    }

    // マウスがメニューパネル上にあるか判定
    let over_panel = state.menu_visible
        && q_window
            .single()
            .ok()
            .and_then(|w| w.cursor_position().map(|p| p.x > w.width() - MENU_WIDTH))
            .unwrap_or(false);

    // マウス座標 → ワールド座標 → グリッド座標
    if !over_panel
        && let Ok(window) = q_window.single()
        && let Some(cursor_screen) = window.cursor_position()
        && let Ok((camera, cam_tf)) = q_camera.single()
        && let Ok(world_pos) = camera.viewport_to_world_2d(cam_tf, cursor_screen)
    {
        let (gx, gy) = world_to_grid(world_pos);
        state.building_cursor = (
            gx.clamp(0, MAP_WIDTH - 1),
            gy.clamp(0, MAP_HEIGHT - 1),
        );
    }

    let grid = state.building_cursor;
    let occupied = q_buildings.iter().any(|(_, b)| b.grid == grid);

    // 左クリック: 配置 or 削除（パネル上でなければ）
    if !over_panel && mouse_buttons.just_pressed(MouseButton::Left) {
        if occupied {
            despawn_test_building_at(&mut commands, grid, &q_buildings, &q_building_3d);
        } else if let (Some(ba), Some(bh)) = (
            building_assets.as_deref(),
            building_3d_handles.as_deref(),
        ) {
            spawn_test_building(&mut commands, state.building_kind, grid, ba, bh);
        }
    }

    // ゴーストスプライトを更新: 建築テクスチャ + 緑(空き) / 赤(占有)
    let pos2d = grid_to_world(grid.0, grid.1);
    tf.translation.x = pos2d.x;
    tf.translation.y = pos2d.y;
    tf.translation.z = 0.25;
    sprite.custom_size = Some(TestBuildingAssets::size_for(state.building_kind));
    sprite.color = if occupied {
        Color::srgba(1.0, 0.2, 0.2, 0.5)
    } else {
        Color::srgba(0.5, 1.0, 0.5, 0.5)
    };
    if let Some(assets) = &building_assets {
        sprite.image = assets.image_for(state.building_kind);
    }
}

// ─── ワールドマップ ───────────────────────────────────────────────────────────

/// ワールドマップのタイルスプライトをスポーンする。
pub fn setup_world_map(mut commands: Commands, asset_server: Res<AssetServer>) {
    let grass = asset_server.load("textures/grass.png");
    let dirt = asset_server.load("textures/dirt.png");
    let river = asset_server.load("textures/river.png");
    let sand = asset_server.load("textures/sand_terrain.png");

    let terrain = generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, SAND_WIDTH);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            let (texture, z) = match terrain[idx] {
                TerrainType::Grass => (grass.clone(), Z_MAP_GRASS),
                TerrainType::Dirt => (dirt.clone(), Z_MAP_DIRT),
                TerrainType::River => (river.clone(), Z_MAP),
                TerrainType::Sand => (sand.clone(), Z_MAP_SAND),
            };
            let pos = grid_to_world(x, y);
            commands.spawn((
                Sprite { image: texture, custom_size: Some(Vec2::splat(TILE_SIZE)), ..default() },
                Transform::from_xyz(pos.x, pos.y, z),
                RenderLayers::layer(LAYER_2D),
                WorldMapTile,
            ));
        }
    }
}
