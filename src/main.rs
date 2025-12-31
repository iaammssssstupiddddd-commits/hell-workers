use bevy::prelude::*;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use rand::Rng;

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
        .add_systems(PostStartup, (spawn_map, spawn_colonists).chain())
        .add_systems(Update, (
            camera_movement, 
            camera_zoom, 
            log_periodically,
            (pathfinding_system, colonist_movement, animation_system).chain(),
        ))
        .run();
}

const TILE_SIZE: f32 = 32.0;
const MAP_WIDTH: i32 = 40;
const MAP_HEIGHT: i32 = 40;

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Tile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerrainType {
    Grass,
    Dirt,
    Stone, // 通行不可
}

impl TerrainType {
    fn is_walkable(&self) -> bool {
        match self {
            TerrainType::Grass | TerrainType::Dirt => true,
            TerrainType::Stone => false,
        }
    }
}

// ワールドマップリソース（A*用）
#[derive(Resource)]
struct WorldMap {
    tiles: HashMap<(i32, i32), TerrainType>,
}

impl WorldMap {
    fn new() -> Self {
        Self { tiles: HashMap::new() }
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
            return false;
        }
        self.tiles.get(&(x, y)).map_or(false, |t| t.is_walkable())
    }

    fn world_to_grid(pos: Vec2) -> (i32, i32) {
        let x = ((pos.x / TILE_SIZE) + (MAP_WIDTH as f32 / 2.0)).floor() as i32;
        let y = ((pos.y / TILE_SIZE) + (MAP_HEIGHT as f32 / 2.0)).floor() as i32;
        (x, y)
    }

    fn grid_to_world(x: i32, y: i32) -> Vec2 {
        Vec2::new(
            (x as f32 - MAP_WIDTH as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0,
            (y as f32 - MAP_HEIGHT as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0,
        )
    }
}

// A*のためのノード
#[derive(Clone, Eq, PartialEq)]
struct Node {
    pos: (i32, i32),
    g_cost: i32,
    f_cost: i32,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost) // 最小ヒープにするため逆順
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// A*パスファインディング
fn find_path(world_map: &WorldMap, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if !world_map.is_walkable(goal.0, goal.1) {
        return None;
    }

    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    let heuristic = |a: (i32, i32), b: (i32, i32)| -> i32 {
        ((a.0 - b.0).abs() + (a.1 - b.1).abs()) * 10
    };

    g_score.insert(start, 0);
    open_set.push(Node {
        pos: start,
        g_cost: 0,
        f_cost: heuristic(start, goal),
    });

    let directions = [
        (0, 1), (0, -1), (1, 0), (-1, 0),
        (1, 1), (1, -1), (-1, 1), (-1, -1),
    ];

    while let Some(current) = open_set.pop() {
        if current.pos == goal {
            // パスを再構築
            let mut path = vec![goal];
            let mut current_pos = goal;
            while let Some(&prev) = came_from.get(&current_pos) {
                path.push(prev);
                current_pos = prev;
            }
            path.reverse();
            return Some(path);
        }

        for (dx, dy) in &directions {
            let neighbor = (current.pos.0 + dx, current.pos.1 + dy);
            
            if !world_map.is_walkable(neighbor.0, neighbor.1) {
                continue;
            }

            // 斜め移動のコスト（14）と直線移動のコスト（10）
            let move_cost = if *dx != 0 && *dy != 0 { 14 } else { 10 };
            let tentative_g = g_score.get(&current.pos).unwrap_or(&i32::MAX) + move_cost;

            if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open_set.push(Node {
                    pos: neighbor,
                    g_cost: tentative_g,
                    f_cost: tentative_g + heuristic(neighbor, goal),
                });
            }
        }
    }

    None
}

#[derive(Resource)]
struct GameAssets {
    grass: Handle<Image>,
    dirt: Handle<Image>,
    stone: Handle<Image>,
    colonist: Handle<Image>,
}

#[derive(Component)]
struct Colonist;

#[derive(Component)]
struct Destination(Vec2);

#[derive(Component, Default)]
struct Path {
    waypoints: Vec<Vec2>,
    current_index: usize,
}

// アニメーション状態
#[derive(Component)]
struct AnimationState {
    is_moving: bool,
    facing_right: bool,
    bob_timer: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            is_moving: false,
            facing_right: true,
            bob_timer: 0.0,
        }
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((Camera2d, MainCamera));
    commands.insert_resource(WorldMap::new());
    
    let game_assets = GameAssets {
        grass: asset_server.load("textures/grass.jpg"),
        dirt: asset_server.load("textures/dirt.jpg"),
        stone: asset_server.load("textures/stone.jpg"),
        colonist: asset_server.load("textures/colonist.jpg"),
    };
    commands.insert_resource(game_assets);
}

fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let (terrain, texture) = if (x + y) % 7 == 0 {
                (TerrainType::Stone, game_assets.stone.clone())
            } else if (x * y) % 5 == 0 {
                (TerrainType::Dirt, game_assets.dirt.clone())
            } else {
                (TerrainType::Grass, game_assets.grass.clone())
            };

            world_map.tiles.insert((x, y), terrain);

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
        let speed = 500.0 * projection.scale;
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
    
    projection.scale = projection.scale.clamp(0.1, 5.0);
}

fn log_periodically(
    time: Res<Time>,
    mut timer: Local<f32>,
    query_cam: Query<&Transform, With<MainCamera>>,
    query_col: Query<(&Transform, Option<&Path>), With<Colonist>>,
    game_assets: Res<GameAssets>,
    asset_server: Res<AssetServer>,
) {
    *timer += time.delta_secs();
    if *timer > 2.0 {
        let cam_transform = query_cam.single();
        info!("CAMERA_POS: x: {:.1}, y: {:.1}", cam_transform.translation.x, cam_transform.translation.y);
        
        for (col_transform, path) in query_col.iter() {
            let path_len = path.map_or(0, |p| p.waypoints.len());
            info!("COLONIST_POS: x: {:.1}, y: {:.1}, path_len: {}", 
                col_transform.translation.x, col_transform.translation.y, path_len);
        }

        let grass_load = asset_server.get_load_state(&game_assets.grass);
        let colonist_load = asset_server.get_load_state(&game_assets.colonist);
        info!("ASSET_LOAD_STATE: Grass:{:?}, Colonist:{:?}", grass_load, colonist_load);
        
        *timer = 0.0;
    }
}

fn spawn_colonists(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    // 入植者の初期位置を設定
    let spawn_pos = Vec2::new(0.0, 0.0);
    let spawn_grid = WorldMap::world_to_grid(spawn_pos);
    
    // 歩行可能な目的地を探す
    let mut dest_grid = (spawn_grid.0 + 5, spawn_grid.1 + 5);
    for dx in 0..10 {
        for dy in 0..10 {
            let test = (spawn_grid.0 + dx, spawn_grid.1 + dy);
            if world_map.is_walkable(test.0, test.1) {
                dest_grid = test;
                break;
            }
        }
    }
    let dest_pos = WorldMap::grid_to_world(dest_grid.0, dest_grid.1);
    
    info!("SPAWN_DEBUG: spawn_grid={:?}, spawn_walkable={}, dest_grid={:?}, dest_walkable={}",
        spawn_grid, world_map.is_walkable(spawn_grid.0, spawn_grid.1),
        dest_grid, world_map.is_walkable(dest_grid.0, dest_grid.1));

    commands.spawn((
        Colonist,
        Sprite {
            image: game_assets.colonist.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 1.0),
        Destination(dest_pos),
        Path::default(),
        AnimationState::default(),
    ));
    
    info!("BEVY_STARTUP: Colonist spawned at {:?}, destination {:?}", spawn_pos, dest_pos);
}

// パスファインディングシステム
fn pathfinding_system(
    world_map: Res<WorldMap>,
    mut query: Query<(&Transform, &mut Destination, &mut Path), With<Colonist>>,
) {
    // 確実に歩行可能な巡回ポイント
    let patrol_points = [
        Vec2::new(-304.0, -304.0),  // grid(10,10)
        Vec2::new(336.0, -304.0),   // grid(30,10)
        Vec2::new(336.0, 336.0),    // grid(30,30)
        Vec2::new(-304.0, 336.0),   // grid(10,30)
        Vec2::new(16.0, 16.0),      // grid(20,20)
        Vec2::new(-144.0, 176.0),   // grid(15,25)
        Vec2::new(176.0, -144.0),   // grid(25,15)
        Vec2::new(-240.0, -240.0),  // grid(12,12)
    ];

    for (transform, mut destination, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        
        // パスが完了している場合、新しい目的地を選択
        if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
            // 現在位置から最も遠い巡回ポイントを選択
            let mut best_point = patrol_points[0];
            let mut max_dist = 0.0f32;
            
            for point in &patrol_points {
                let dist = current_pos.distance(*point);
                if dist > max_dist {
                    max_dist = dist;
                    best_point = *point;
                }
            }
            
            destination.0 = best_point;
            
            // すぐにパスを計算
            let start_grid = WorldMap::world_to_grid(current_pos);
            let goal_grid = WorldMap::world_to_grid(best_point);
            
            info!("PATHFINDING: from {:?} to {:?}, distance: {:.1}", 
                start_grid, goal_grid, max_dist);

            if let Some(grid_path) = find_path(&world_map, start_grid, goal_grid) {
                path.waypoints = grid_path.iter()
                    .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                    .collect();
                path.current_index = 0;
                info!("PATH_FOUND: {} waypoints from {:?} to {:?}", 
                    path.waypoints.len(), start_grid, goal_grid);
            } else {
                info!("PATH_NOT_FOUND: from {:?} to {:?}", start_grid, goal_grid);
                // パスが見つからない場合、次のポイントを試す
                path.waypoints.clear();
            }
        }
    }
}

fn colonist_movement(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Path, &mut AnimationState), With<Colonist>>,
) {
    for (mut transform, mut path, mut anim) in query.iter_mut() {
        // パスがある場合のみ移動
        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let distance = current_pos.distance(target);
            
            if distance > 2.0 {
                let direction = (target - current_pos).normalize();
                let velocity = direction * 100.0 * time.delta_secs();
                transform.translation += velocity.extend(0.0);
                
                // アニメーション状態を更新
                anim.is_moving = true;
                anim.facing_right = direction.x >= 0.0;
            } else {
                path.current_index += 1;
                anim.is_moving = false;
            }
        } else {
            anim.is_moving = false;
        }
    }
}

// アニメーションシステム
fn animation_system(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Sprite, &mut AnimationState), With<Colonist>>,
) {
    for (mut transform, mut sprite, mut anim) in query.iter_mut() {
        // スプライトの反転（移動方向に応じて）
        sprite.flip_x = !anim.facing_right;
        
        // 移動中のボビングアニメーション
        if anim.is_moving {
            anim.bob_timer += time.delta_secs() * 10.0;
            let bob = (anim.bob_timer.sin() * 0.05) + 1.0;
            transform.scale = Vec3::new(
                if anim.facing_right { 1.0 } else { 1.0 },
                bob,
                1.0
            );
        } else {
            // 静止時はゆっくり呼吸するようなアニメーション
            anim.bob_timer += time.delta_secs() * 2.0;
            let breath = (anim.bob_timer.sin() * 0.02) + 1.0;
            transform.scale = Vec3::splat(breath);
        }
    }
}
