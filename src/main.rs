use bevy::prelude::*;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
// use rand::Rng; // Unused for now

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Hell Workers".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }).set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,bevy_app=debug".to_string(),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(PostStartup, (spawn_map, spawn_colonists, setup_ui).chain())
        .add_systems(Update, (
            camera_movement, 
            camera_zoom, 
            log_periodically,
            handle_mouse_input,
            blueprint_placement,
            zone_placement,
            item_spawner_system,
            ui_interaction_system,
            menu_visibility_system,
            info_panel_system,
            update_selection_indicator,
            resource_count_display_system,
            game_time_system,
            time_control_keyboard_system,
            time_control_ui_system,
            (job_assignment_system, hauling_system, pathfinding_system, colonist_movement, construction_work_system, building_completion_system, animation_system).chain(),
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
    buildings: HashMap<(i32, i32), Entity>, // タイル座標ごとの建物/設計図
    stockpiles: HashMap<(i32, i32), Entity>, // タイル座標ごとの備蓄場所
}

impl WorldMap {
    fn new() -> Self {
        Self { 
            tiles: HashMap::new(),
            buildings: HashMap::new(),
            stockpiles: HashMap::new(),
        }
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
struct PathNode {
    pos: (i32, i32),
    g_cost: i32,
    f_cost: i32,
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost) // 最小ヒープにするため逆順
    }
}

impl PartialOrd for PathNode {
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
    open_set.push(PathNode {
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
                open_set.push(PathNode {
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
    wall: Handle<Image>,
    wood: Handle<Image>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildingType {
    Wall,
    Floor,
}

#[derive(Component)]
struct Building(BuildingType);

#[derive(Component)]
struct Blueprint {
    kind: BuildingType,
    progress: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceType {
    Wood,
}

#[derive(Component)]
struct ResourceItem(ResourceType);

#[derive(Component)]
struct ClaimedBy(Entity);

#[derive(Component)]
struct InStockpile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZoneType {
    Stockpile,
}

#[derive(Component)]
struct Stockpile;

#[derive(Component, Default)]
struct Inventory(Option<Entity>);

#[derive(Resource, Default)]
struct SelectedEntity(Option<Entity>);

#[derive(Resource, Default)]
enum MenuState {
    #[default]
    Hidden,
    Architect,
    Zones,
}

#[derive(Resource, Default)]
struct BuildMode(Option<BuildingType>);

#[derive(Resource, Default)]
struct ZoneMode(Option<ZoneType>);

#[derive(Component)]
struct MenuButton(MenuAction);

#[derive(Clone, Copy)]
enum MenuAction {
    ToggleArchitect,
    ToggleZones,
    SelectBuild(BuildingType),
    SelectZone(ZoneType),
}

#[derive(Component)]
struct SelectionIndicator;

#[derive(Resource, Default)]
struct ResourceLabels(HashMap<(i32, i32), Entity>);

#[derive(Component)]
struct ResourceCountLabel;

#[derive(Component)]
struct Colonist;

#[derive(Component)]
struct CurrentJob(Option<Entity>);

#[derive(Component)]
struct Destination(Vec2);

#[derive(Component, Default)]
struct Path {
    waypoints: Vec<Vec2>,
    current_index: usize,
}

// Time & Progression
#[derive(Resource, Default)]
struct GameTime {
    seconds: f32,
    day: u32,
    hour: u32,
    minute: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TimeSpeed {
    Paused,
    Normal, // 1x
    Fast,   // 2x
    Super,  // 4x
}

#[derive(Component)]
struct SpeedButton(TimeSpeed);

#[derive(Component)]
struct ClockText;

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
        wall: asset_server.load("textures/stone.jpg"), // Placeholder
        wood: asset_server.load("textures/dirt.jpg"), // Placeholder
    };
    commands.insert_resource(game_assets);
    commands.init_resource::<SelectedEntity>();
    commands.init_resource::<MenuState>();
    commands.init_resource::<BuildMode>();
    commands.init_resource::<ZoneMode>();
    commands.init_resource::<ResourceLabels>();
    commands.init_resource::<GameTime>();
}

fn setup_ui(mut commands: Commands) {
    // Bottom bar
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(50.0),
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
    )).with_children(|parent| {
        // Architect button
        parent.spawn((
            Button,
            Node {
                width: Val::Px(100.0),
                height: Val::Px(40.0),
                margin: UiRect::right(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            MenuButton(MenuAction::ToggleArchitect),
        )).with_children(|button| {
            button.spawn((
                Text::new("Architect"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

        // Zones button
        parent.spawn((
            Button,
            Node {
                width: Val::Px(100.0),
                height: Val::Px(40.0),
                margin: UiRect::right(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            MenuButton(MenuAction::ToggleZones),
        )).with_children(|button| {
            button.spawn((
                Text::new("Zones"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });

    // Architect Sub-menu (Wall button)
    commands.spawn((
        Node {
            display: Display::None, // Initially hidden
            width: Val::Px(120.0),
            height: Val::Auto,
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(50.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
    )).insert(ArchitectSubMenu).with_children(|parent| {
        parent.spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
            MenuButton(MenuAction::SelectBuild(BuildingType::Wall)),
        )).with_children(|button| {
            button.spawn((
                Text::new("Wall"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });

    // Zones Sub-menu (Stockpile button)
    commands.spawn((
        Node {
            display: Display::None,
            width: Val::Px(120.0),
            height: Val::Auto,
            position_type: PositionType::Absolute,
            left: Val::Px(110.0), // Shifted to right of Architect button
            bottom: Val::Px(50.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
    )).insert(ZonesSubMenu).with_children(|parent| {
        parent.spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
            MenuButton(MenuAction::SelectZone(ZoneType::Stockpile)),
        )).with_children(|button| {
            button.spawn((
                Text::new("Stockpile"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });

    // Info Panel (Selected colonist info)
    commands.spawn((
        Node {
            display: Display::None,
            width: Val::Px(200.0),
            height: Val::Auto,
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        InfoPanel,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Colonist Info"),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
        ));
        parent.spawn((
            Text::new("Job: None"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )).insert(InfoPanelJobText);
    });

    // Time Control & Clock UI (Top Right)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            ..default()
        },
    )).with_children(|parent| {
        // Clock
        parent.spawn((
            Text::new("Day 1, 00:00"),
            TextFont {
                font_size: 24.0,
                ..default()
            },
            TextColor(Color::WHITE),
            ClockText,
        ));

        // Speed Buttons
        parent.spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::top(Val::Px(5.0)),
            ..default()
        }).with_children(|speed_row| {
            let speeds = [
                (TimeSpeed::Paused, "||"),
                (TimeSpeed::Normal, ">"),
                (TimeSpeed::Fast, ">>"),
                (TimeSpeed::Super, ">>>"),
            ];

            for (speed, label) in speeds {
                speed_row.spawn((
                    Button,
                    Node {
                        width: Val::Px(40.0),
                        height: Val::Px(30.0),
                        margin: UiRect::left(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                    SpeedButton(speed),
                )).with_children(|btn| {
                    btn.spawn((
                        Text::new(label),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
    });
}

#[derive(Component)]
struct InfoPanel;

#[derive(Component)]
struct InfoPanelJobText;

#[derive(Component)]
struct ArchitectSubMenu;

#[derive(Component)]
struct ZonesSubMenu;

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

fn handle_mouse_input(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_colonists: Query<(Entity, &GlobalTransform), With<Colonist>>,
    q_ui: Query<&Interaction, With<Button>>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut q_dest: Query<&mut Destination>,
    _commands: Commands,
) {
    // UIを触っている時は処理しない
    for interaction in q_ui.iter() {
        if *interaction != Interaction::None {
            return;
        }
    }

    let (camera, camera_transform) = q_camera.single();
    let window = q_window.single();

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            
            // Left click to select
            if buttons.just_pressed(MouseButton::Left) {
                let mut found = false;
                for (entity, transform) in q_colonists.iter() {
                    let col_pos = transform.translation().truncate();
                    if col_pos.distance(world_pos) < TILE_SIZE / 2.0 {
                        selected_entity.0 = Some(entity);
                        found = true;
                        break;
                    }
                }
                if !found {
                    selected_entity.0 = None;
                }
            }

            // Right click to order move
            if buttons.just_pressed(MouseButton::Right) {
                if let Some(selected) = selected_entity.0 {
                    if let Ok(mut dest) = q_dest.get_mut(selected) {
                        dest.0 = world_pos;
                        info!("ORDER: Move to {:?}", world_pos);
                    }
                }
            }
        }
    }
}

// Update selection indicator position
fn blueprint_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    mut world_map: ResMut<WorldMap>,
    build_mode: Res<BuildMode>,
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    // UIを触っている時は処理しない
    for interaction in q_ui.iter() {
        if *interaction != Interaction::None {
            return;
        }
    }

    if let Some(building_type) = build_mode.0 {
        if buttons.just_pressed(MouseButton::Left) {
            let (camera, camera_transform) = q_camera.single();
            let window = q_window.single();

            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    let grid = WorldMap::world_to_grid(world_pos);
                    
                    if !world_map.buildings.contains_key(&grid) {
                        let pos = WorldMap::grid_to_world(grid.0, grid.1);
                        
                        let texture = match building_type {
                            BuildingType::Wall => game_assets.wall.clone(),
                            BuildingType::Floor => game_assets.dirt.clone(), // Placeholder
                        };

                        let entity = commands.spawn((
                            Blueprint {
                                kind: building_type,
                                progress: 0.0,
                            },
                            Sprite {
                                image: texture,
                                color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, 0.1),
                        )).id();
                        
                        world_map.buildings.insert(grid, entity);
                        info!("BLUEPRINT: Placed {:?} at {:?}", building_type, grid);
                    }
                }
            }
        }
    }
}

fn ui_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<MenuState>,
    mut build_mode: ResMut<BuildMode>,
    mut zone_mode: ResMut<ZoneMode>,
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.5, 0.5, 0.5));
                match menu_button.0 {
                    MenuAction::ToggleArchitect => {
                        *menu_state = match *menu_state {
                            MenuState::Architect => MenuState::Hidden,
                            _ => MenuState::Architect,
                        };
                        build_mode.0 = None;
                        zone_mode.0 = None;
                    }
                    MenuAction::ToggleZones => {
                        *menu_state = match *menu_state {
                            MenuState::Zones => MenuState::Hidden,
                            _ => MenuState::Zones,
                        };
                        build_mode.0 = None;
                        zone_mode.0 = None;
                    }
                    MenuAction::SelectBuild(kind) => {
                        build_mode.0 = Some(kind);
                        zone_mode.0 = None;
                        info!("BUILD_MODE: Selected {:?}", kind);
                    }
                    MenuAction::SelectZone(kind) => {
                        zone_mode.0 = Some(kind);
                        build_mode.0 = None;
                        info!("ZONE_MODE: Selected {:?}", kind);
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.4, 0.4, 0.4));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }
    }
}

fn menu_visibility_system(
    menu_state: Res<MenuState>,
    mut q_architect: Query<&mut Node, (With<ArchitectSubMenu>, Without<ZonesSubMenu>)>,
    mut q_zones: Query<&mut Node, (With<ZonesSubMenu>, Without<ArchitectSubMenu>)>,
) {
    if let Ok(mut node) = q_architect.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Architect) { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = q_zones.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) { Display::Flex } else { Display::None };
    }
}

fn info_panel_system(
    selected: Res<SelectedEntity>,
    mut q_panel: Query<&mut Node, With<InfoPanel>>,
    mut q_text: Query<&mut Text, With<InfoPanelJobText>>,
    q_colonists: Query<&CurrentJob, With<Colonist>>,
    q_blueprints: Query<&Blueprint>,
) {
    let mut panel_node = q_panel.single_mut();
    if let Some(entity) = selected.0 {
        if let Ok(job) = q_colonists.get(entity) {
            panel_node.display = Display::Flex;
            let mut text = q_text.single_mut();
            if let Some(job_entity) = job.0 {
                if let Ok(bp) = q_blueprints.get(job_entity) {
                    text.0 = format!("Job: Building {:?} ({:.0}%)", bp.kind, bp.progress * 100.0);
                } else {
                    text.0 = "Job: Moving".to_string();
                }
            } else {
                text.0 = "Job: Idle".to_string();
            }
        } else {
            panel_node.display = Display::None;
        }
    } else {
        panel_node.display = Display::None;
    }
}

fn job_assignment_system(
    mut q_colonists: Query<(Entity, &mut CurrentJob, &mut Destination), With<Colonist>>,
    q_blueprints: Query<Entity, With<Blueprint>>,
    q_items_unclaimed: Query<Entity, (With<ResourceItem>, Without<ClaimedBy>, Without<InStockpile>)>,
    q_items_all: Query<Entity, With<ResourceItem>>,
    q_transforms: Query<&Transform>,
    mut commands: Commands,
) {
    for (col_entity, mut job, mut dest) in q_colonists.iter_mut() {
        if job.0.is_none() {
            // 1. 建築優先
            if let Some(bp_entity) = q_blueprints.iter().next() {
                job.0 = Some(bp_entity);
                if let Ok(bp_transform) = q_transforms.get(bp_entity) {
                    let target = bp_transform.translation.truncate();
                    if dest.0 != target {
                        dest.0 = target;
                    }
                }
            } 
            // 2. 運搬
            else if let Some(item_entity) = q_items_unclaimed.iter().next() {
                job.0 = Some(item_entity);
                commands.entity(item_entity).insert(ClaimedBy(col_entity));
                if let Ok(item_transform) = q_transforms.get(item_entity) {
                    let target = item_transform.translation.truncate();
                    if dest.0 != target {
                        dest.0 = target;
                    }
                }
            }
        } else {
            // ジョブの有効性チェック
            let job_entity = job.0.unwrap();
            let job_valid = q_blueprints.get(job_entity).is_ok() || q_items_all.get(job_entity).is_ok();
            if !job_valid {
                job.0 = None;
            }
        }
    }
}

fn construction_work_system(
    time: Res<Time>,
    q_colonists: Query<(&Transform, &CurrentJob), With<Colonist>>,
    mut q_blueprints: Query<(&Transform, &mut Blueprint)>,
) {
    for (col_transform, job) in q_colonists.iter() {
        if let Some(job_entity) = job.0 {
            if let Ok((bp_transform, mut bp)) = q_blueprints.get_mut(job_entity) {
                let dist = col_transform.translation.truncate().distance(bp_transform.translation.truncate());
                if dist < TILE_SIZE * 0.5 {
                    bp.progress += time.delta_secs() * 0.2; // 5 seconds to build
                }
            }
        }
    }
}

fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
) {
    for (entity, bp, transform) in q_blueprints.iter_mut() {
        if bp.progress >= 1.0 {
            info!("BUILDING: Completed at {:?}", transform.translation);
            
            // Replace blueprint with building
            commands.entity(entity).despawn();
            
            commands.spawn((
                Building(bp.kind),
                Sprite {
                    image: game_assets.wall.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                *transform,
            ));
        }
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
        CurrentJob(None),
        Inventory(None),
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
    mut query: Query<(&Transform, &Destination, &mut Path), Changed<Destination>>,
) {
    for (transform, destination, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        let start_grid = WorldMap::world_to_grid(current_pos);
        let goal_grid = WorldMap::world_to_grid(destination.0);
        
        info!("PATHFINDING: from {:?} to {:?}", start_grid, goal_grid);

        if let Some(grid_path) = find_path(&world_map, start_grid, goal_grid) {
            path.waypoints = grid_path.iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            info!("PATH_FOUND: {} waypoints", path.waypoints.len());
        } else {
            info!("PATH_NOT_FOUND");
            path.waypoints.clear();
        }
    }
}

fn colonist_movement(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Path, &mut AnimationState), With<Colonist>>,
) {
    for (mut transform, mut path, mut anim) in query.iter_mut() {
        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();
            
            if distance > 1.0 {
                let speed = 100.0;
                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                transform.translation += velocity.extend(0.0);
                
                anim.is_moving = true;
                // 反転のチラつき防止にデッドゾーンを設ける
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                path.current_index += 1;
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

fn update_selection_indicator(
    selected: Res<SelectedEntity>,
    mut q_indicator: Query<(Entity, &mut Transform), With<SelectionIndicator>>,
    q_transforms: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    if let Some(entity) = selected.0 {
        if let Ok(target_transform) = q_transforms.get(entity) {
            if let Ok((_, mut indicator_transform)) = q_indicator.get_single_mut() {
                indicator_transform.translation = target_transform.translation().truncate().extend(0.5);
            } else {
                commands.spawn((
                    SelectionIndicator,
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.4),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.1)),
                        ..default()
                    },
                    Transform::from_translation(target_transform.translation().truncate().extend(0.5)),
                ));
            }
        }
    } else {
        for (indicator_entity, _) in q_indicator.iter() {
            commands.entity(indicator_entity).despawn();
        }
    }
}
fn zone_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    zone_mode: Res<ZoneMode>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    if let Some(zone_type) = zone_mode.0 {
        for interaction in q_ui.iter() {
            if *interaction != Interaction::None {
                return;
            }
        }

        if buttons.pressed(MouseButton::Left) {
            let (camera, camera_transform) = q_camera.single();
            let window = q_window.single();

            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    let grid = WorldMap::world_to_grid(world_pos);
                    
                    if !world_map.stockpiles.contains_key(&grid) {
                        let pos = WorldMap::grid_to_world(grid.0, grid.1);

                        match zone_type {
                            ZoneType::Stockpile => {
                                let entity = commands.spawn((
                                    Stockpile,
                                    Sprite {
                                        color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                                        custom_size: Some(Vec2::splat(TILE_SIZE)),
                                        ..default()
                                    },
                                    Transform::from_xyz(pos.x, pos.y, 0.01),
                                )).id();
                                world_map.stockpiles.insert(grid, entity);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn item_spawner_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer > 5.0 {
        let pos = Vec2::new(100.0, 100.0);
        commands.spawn((
            ResourceItem(ResourceType::Wood),
            Sprite {
                image: game_assets.wood.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.6), // Slightly higher Z than selection indicator
        ));
        *timer = 0.0;
        info!("SPAWNER: Wood spawned at {:?}", pos);
    }
}

fn hauling_system(
    mut q_colonists: Query<(&Transform, &mut CurrentJob, &mut Inventory, &mut Destination), With<Colonist>>,
    q_items: Query<(Entity, &Transform), With<ResourceItem>>,
    q_stockpiles: Query<&Transform, With<Stockpile>>,
    mut commands: Commands,
) {
    for (transform, mut job, mut inventory, mut dest) in q_colonists.iter_mut() {
        if let Some(target_entity) = job.0 {
            if let Ok((_item_entity, item_transform)) = q_items.get(target_entity) {
                let col_pos = transform.translation.truncate();
                
                if inventory.0.is_none() {
                    let item_pos = item_transform.translation.truncate();
                    let dist = col_pos.distance(item_pos);
                    if dist < TILE_SIZE * 0.7 {
                        inventory.0 = Some(target_entity);
                        commands.entity(target_entity).insert(Visibility::Hidden);
                        info!("HAUL: Picked up item {:?}", target_entity);
                    } else if dest.0 != item_pos {
                        dest.0 = item_pos;
                    }
                } else {
                    if let Some(stock_transform) = q_stockpiles.iter().next() {
                        let target_stock = stock_transform.translation.truncate();
                        if col_pos.distance(target_stock) < TILE_SIZE * 0.7 {
                            let item_entity = inventory.0.take().unwrap();
                            commands.entity(item_entity).insert(Visibility::Visible);
                            commands.entity(item_entity).insert(Transform::from_xyz(target_stock.x, target_stock.y, 0.6));
                            commands.entity(item_entity).insert(InStockpile);
                            commands.entity(item_entity).remove::<ClaimedBy>();
                            job.0 = None;
                            info!("HAUL: Dropped item {:?} at stockpile", item_entity);
                        } else if dest.0 != target_stock {
                            dest.0 = target_stock;
                        }
                    } else {
                        // Stockpile disappeared?
                    }
                }
            }
        }
    }
}

fn resource_count_display_system(
    mut commands: Commands,
    q_items: Query<(&Transform, &Visibility), With<ResourceItem>>,
    mut labels: ResMut<ResourceLabels>,
    mut q_text: Query<&mut Text2d, With<ResourceCountLabel>>,
) {
    let mut grid_counts: HashMap<(i32, i32), usize> = HashMap::new();

    // グリッドごとに表示されているアイテムを集計
    for (transform, visibility) in q_items.iter() {
        if matches!(visibility, Visibility::Visible | Visibility::Inherited) {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            *grid_counts.entry(grid).or_insert(0) += 1;
        }
    }

    // 表示が必要なラベルを更新または作成
    for (grid, count) in grid_counts.iter() {
        if let Some(&entity) = labels.0.get(grid) {
            if let Ok(mut text) = q_text.get_mut(entity) {
                text.0 = count.to_string();
            }
        } else {
            let pos = WorldMap::grid_to_world(grid.0, grid.1);
            let entity = commands.spawn((
                ResourceCountLabel,
                Text2d::new(count.to_string()),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(JustifyText::Center),
                // タイルの右上に少しずらす
                Transform::from_xyz(pos.x + TILE_SIZE * 0.3, pos.y + TILE_SIZE * 0.3, 1.0),
            )).id();
            labels.0.insert(*grid, entity);
        }
    }

    // 不要になったラベルを削除
    let mut to_remove = Vec::new();
    for (&grid, &entity) in labels.0.iter() {
        if !grid_counts.contains_key(&grid) {
            if let Some(mut e) = commands.get_entity(entity) {
                e.despawn();
            }
            to_remove.push(grid);
        }
    }
    for grid in to_remove {
        labels.0.remove(&grid);
    }
}

fn game_time_system(
    time: Res<Time>,
    mut game_time: ResMut<GameTime>,
    mut q_clock: Query<&mut Text, With<ClockText>>,
) {
    // 1ゲーム秒 = 実際の1秒 (1xの場合)
    // 1時間 = 60秒
    // 1日 = 24時間 (1440秒)
    game_time.seconds += time.delta_secs();
    
    let total_mins = (game_time.seconds / 60.0) as u32;
    game_time.minute = total_mins % 60;
    
    let total_hours = total_mins / 60;
    game_time.hour = total_hours % 24;
    
    game_time.day = (total_hours / 24) + 1;

    if let Ok(mut text) = q_clock.get_single_mut() {
        text.0 = format!("Day {}, {:02}:{:02}", game_time.day, game_time.hour, game_time.minute);
    }
}

fn time_control_keyboard_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    
    if keyboard.just_pressed(KeyCode::Digit1) {
        time.unpause();
        time.set_relative_speed(1.0);
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        time.unpause();
        time.set_relative_speed(2.0);
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        time.unpause();
        time.set_relative_speed(4.0);
    }
}

fn time_control_ui_system(
    mut interaction_query: Query<
        (&Interaction, &SpeedButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut time: ResMut<Time<Virtual>>,
) {
    for (interaction, speed_button, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                match speed_button.0 {
                    TimeSpeed::Paused => time.pause(),
                    TimeSpeed::Normal => {
                        time.unpause();
                        time.set_relative_speed(1.0);
                    }
                    TimeSpeed::Fast => {
                        time.unpause();
                        time.set_relative_speed(2.0);
                    }
                    TimeSpeed::Super => {
                        time.unpause();
                        time.set_relative_speed(4.0);
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.4, 0.4, 0.4));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }
    }
}
