use bevy::prelude::*;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy".to_string()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }).set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,bevy_app=debug".to_string(),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(PostStartup, spawn_map) // アセットがロードされるタイミングを考慮
        .add_systems(Update, (camera_movement, camera_zoom, log_periodically))
        .run();
}

const TILE_SIZE: f32 = 32.0;
const MAP_WIDTH: i32 = 40;
const MAP_HEIGHT: i32 = 40;

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Tile;

#[derive(Debug, Clone, Copy, PartialEq)]
enum TerrainType {
    Grass,
    Dirt,
    Stone,
}

impl TerrainType {
    #[allow(dead_code)]
    fn color(&self) -> Color {
        match self {
            TerrainType::Grass => Color::srgb(0.2, 0.5, 0.2),
            TerrainType::Dirt => Color::srgb(0.4, 0.3, 0.2),
            TerrainType::Stone => Color::srgb(0.5, 0.5, 0.5),
        }
    }
}

#[derive(Resource)]
struct TileAssets {
    grass: Handle<Image>,
    dirt: Handle<Image>,
    stone: Handle<Image>,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((Camera2d, MainCamera));
    
    // アセットをロードしてリソースに登録
    let tile_assets = TileAssets {
        grass: asset_server.load("textures/grass.jpg"),
        dirt: asset_server.load("textures/dirt.jpg"),
        stone: asset_server.load("textures/stone.jpg"),
    };
    commands.insert_resource(tile_assets);
}

fn spawn_map(
    mut commands: Commands,
    tile_assets: Res<TileAssets>,
) {
    // タイルマップを生成
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let (_terrain, texture) = if (x + y) % 7 == 0 {
                (TerrainType::Stone, tile_assets.stone.clone())
            } else if (x * y) % 5 == 0 {
                (TerrainType::Dirt, tile_assets.dirt.clone())
            } else {
                (TerrainType::Grass, tile_assets.grass.clone())
            };

            commands.spawn((
                Tile,
                Sprite {
                    image: texture,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(
                    (x as f32 - MAP_WIDTH as f32 / 2.0) * TILE_SIZE,
                    (y as f32 - MAP_HEIGHT as f32 / 2.0) * TILE_SIZE,
                    0.0,
                ),
            ));
        }
    }

    info!("BEVY_STARTUP: Map spawned ({}x{} tiles)", MAP_WIDTH, MAP_HEIGHT);
}

fn camera_movement(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &OrthographicProjection), With<MainCamera>>,
) {
    let (mut transform, projection) = query.single_mut();
    let mut direction = Vec3::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) { direction.y += 1.0; }
    if keyboard_input.pressed(KeyCode::KeyS) { direction.y -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyA) { direction.x -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyD) { direction.x += 1.0; }

    if direction != Vec3::ZERO {
        let speed = 500.0 * projection.scale; // ズームレベルに応じて移動速度を調整
        transform.translation += direction.normalize() * speed * time.delta_secs();
    }
}

fn camera_zoom(
    mut mouse_wheel_events: EventReader<bevy::input::mouse::MouseWheel>,
    mut query: Query<&mut OrthographicProjection, With<MainCamera>>,
) {
    let mut projection = query.single_mut();
    
    for event in mouse_wheel_events.read() {
        let zoom_factor = 1.1;
        if event.y > 0.0 {
            projection.scale /= zoom_factor;
        } else if event.y < 0.0 {
            projection.scale *= zoom_factor;
        }
    }
    
    // 極端なズームを防止
    projection.scale = projection.scale.clamp(0.1, 5.0);
}

fn log_periodically(
    time: Res<Time>,
    mut timer: Local<f32>,
    query: Query<&Transform, With<MainCamera>>,
    tile_assets: Res<TileAssets>,
    asset_server: Res<AssetServer>,
) {
    *timer += time.delta_secs();
    if *timer > 2.0 {
        let transform = query.single();
        info!("CAMERA_POS: x: {:.1}, y: {:.1}", transform.translation.x, transform.translation.y);
        
        // アセットのロード状況をチェック
        let grass_load = asset_server.get_load_state(&tile_assets.grass);
        let dirt_load = asset_server.get_load_state(&tile_assets.dirt);
        let stone_load = asset_server.get_load_state(&tile_assets.stone);
        info!("ASSET_LOAD_STATE: Grass:{:?}, Dirt:{:?}, Stone:{:?}", grass_load, dirt_load, stone_load);
        
        *timer = 0.0;
    }
}
