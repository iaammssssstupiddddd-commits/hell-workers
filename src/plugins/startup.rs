//! スタートアップ関連のプラグイン

use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::{
    ActiveCommand, FamiliarCommand, FamiliarOperation, FamiliarSpawnEvent,
};
use crate::game_state::{BuildContext, CompanionPlacementState, TaskContext, ZoneContext};
use crate::interface::camera::{MainCamera, PanCamera};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, setup_ui};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, Rock, TaskSlots, Tree, WorkType};
use crate::systems::logistics::{ResourceLabels, initial_resource_spawner};
use crate::systems::soul_ai::decide::work::AutoHaulCounter;
use crate::systems::spatial::{
    BlueprintSpatialGrid, FamiliarSpatialGrid, GatheringSpotSpatialGrid, ResourceSpatialGrid,
    SpatialGrid, SpatialGridOps, StockpileSpatialGrid,
};
use crate::systems::time::GameTime;
use crate::world::map::{WorldMap, spawn_map};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::NoIndirectDrawing;
use std::env;
use std::time::Instant;

fn has_cli_flag(flag: &str) -> bool {
    env::args().any(|arg| arg == flag)
}

pub struct StartupPlugin;

#[derive(Resource, Default)]
struct PerfScenarioApplied(bool);

impl Plugin for StartupPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .init_resource::<WorldMap>()
            .init_resource::<SelectedEntity>()
            .init_resource::<HoveredEntity>()
            .init_resource::<MenuState>()
            .init_resource::<BuildContext>()
            .init_resource::<ZoneContext>()
            .init_resource::<CompanionPlacementState>()
            .init_resource::<ResourceLabels>()
            .init_resource::<GameTime>()
            .init_resource::<TaskContext>()
            .init_resource::<SpatialGrid>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<GatheringSpotSpatialGrid>()
            .init_resource::<BlueprintSpatialGrid>()
            .init_resource::<StockpileSpatialGrid>()
            .init_resource::<AutoHaulCounter>()
            .init_resource::<PerfScenarioApplied>()
            // Startup systems
            .add_systems(Startup, (setup, initialize_gizmo_config))
            .add_systems(
                PostStartup,
                (
                    log_post_startup_begin,
                    spawn_map_timed,
                    initial_resource_spawner_timed, // 先に岩・木を配置して障害物情報を登録
                    spawn_entities,                 // その後、通行可能な場所にソウルを配置
                    spawn_familiar_wrapper,
                    setup_perf_scenario_if_enabled,
                    setup_ui,
                    populate_resource_spatial_grid,
                )
                    .chain(),
            );
        app.add_systems(Update, setup_perf_scenario_runtime_if_enabled);
    }
}

fn log_post_startup_begin() {
    info!("STARTUP_TIMING: PostStartup begin");
}

fn spawn_map_timed(commands: Commands, game_assets: Res<GameAssets>, world_map: ResMut<WorldMap>) {
    let start = Instant::now();
    spawn_map(commands, game_assets, world_map);
    info!(
        "STARTUP_TIMING: spawn_map finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn initial_resource_spawner_timed(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: ResMut<WorldMap>,
) {
    let start = Instant::now();
    initial_resource_spawner(commands, game_assets, world_map);
    info!(
        "STARTUP_TIMING: initial_resource_spawner finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let start = Instant::now();
    commands.spawn((
        Camera2d,
        MainCamera,
        PanCamera::default(),
        NoIndirectDrawing,
    ));

    let aura_circle = create_circular_gradient_texture(&mut *images);
    let aura_ring = create_circular_outline_texture(&mut *images);

    // Load Fonts
    let font_ui = asset_server.load("fonts/NotoSansJP-VF.ttf");
    let font_familiar = asset_server.load("fonts/ShantellSans-VF.ttf");
    let font_soul_name = asset_server.load("fonts/SourceSerif4-VF.ttf");
    let font_soul_emoji = asset_server.load("fonts/NotoEmoji-VF.ttf");

    let game_assets = GameAssets {
        grass: asset_server.load("textures/grass.jpg"),
        dirt: asset_server.load("textures/dirt.jpg"),
        stone: asset_server.load("textures/stone.jpg"),
        river: asset_server.load("textures/river.png"),
        sand: asset_server.load("textures/resources/sandpile/sandpile.png"),
        familiar: asset_server.load("textures/familiar_spritesheet.png"),
        // Soul Animations
        soul_move: asset_server.load("textures/character/soul_move_spritesheet.png"),
        soul_layout: {
            // 1024x1024 を 3x3 に整数分割し、端数ピクセルも最後の列/行に含める。
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(1024, 1024));
            for row in 0..3 {
                for col in 0..3 {
                    let left = col * 1024 / 3;
                    let top = row * 1024 / 3;
                    let right = (col + 1) * 1024 / 3;
                    let bottom = (row + 1) * 1024 / 3;
                    layout.add_texture(URect::new(left, top, right, bottom));
                }
            }
            layouts.add(layout)
        },
        // wall: asset_server.load("textures/stone.jpg"),
        // Wall connections
        wall_isolated: asset_server.load("textures/buildings/wooden_wall/wall_isolated.png"),
        wall_horizontal_left: asset_server
            .load("textures/buildings/wooden_wall/wall_horizontal_left_side_connected.png"),
        wall_horizontal_right: asset_server
            .load("textures/buildings/wooden_wall/wall_horizontal_right_side_connected.png"),
        wall_horizontal_both: asset_server
            .load("textures/buildings/wooden_wall/wall_horizontal_connected_both_side.png"),
        wall_vertical_top: asset_server
            .load("textures/buildings/wooden_wall/wall_vertical_top_side_connected.png"),
        wall_vertical_bottom: asset_server
            .load("textures/buildings/wooden_wall/wall_vertical_bottom_side_connected.png"),
        wall_vertical_both: asset_server
            .load("textures/buildings/wooden_wall/wall_vertical_both_side_connected.png"),
        wall_corner_top_left: asset_server
            .load("textures/buildings/wooden_wall/wall_corner_left_top.png"),
        wall_corner_top_right: asset_server
            .load("textures/buildings/wooden_wall/wall_corner_right_top.png"),
        wall_corner_bottom_left: asset_server
            .load("textures/buildings/wooden_wall/wall_corner_left_down.png"),
        wall_corner_bottom_right: asset_server
            .load("textures/buildings/wooden_wall/wall_corner_right_down.png"),
        wall_t_up: asset_server.load("textures/buildings/wooden_wall/wall_t_up.png"),
        wall_t_down: asset_server.load("textures/buildings/wooden_wall/wall_t_down.png"),
        wall_t_left: asset_server.load("textures/buildings/wooden_wall/wall_t_left.png"),
        wall_t_right: asset_server.load("textures/buildings/wooden_wall/wall_t_right.png"),
        wall_cross: asset_server.load("textures/buildings/wooden_wall/wall_cross.png"),

        wood: asset_server.load("textures/dirt.jpg"),
        tree: asset_server.load("textures/environment/tree/tree_1.png"),
        rock: asset_server.load("textures/rock.png"),
        aura_circle,
        aura_ring,
        // Water related
        tank_empty: asset_server.load("textures/buildings/tank/empty_tank.png"),
        tank_partial: asset_server.load("textures/buildings/tank/half_tank.png"),
        tank_full: asset_server.load("textures/buildings/tank/full_tank.png"),
        bucket_empty: asset_server.load("textures/items/bucket/bucket_empty.png"),
        bucket_water: asset_server.load("textures/items/bucket/bucket_water.png"),
        icon_male: asset_server.load("textures/ui/male.png"),
        icon_female: asset_server.load("textures/ui/female.png"),
        icon_fatigue: asset_server.load("textures/ui/fatigue.png"),
        icon_stress: asset_server.load("textures/ui/stress.png"),
        icon_idle: asset_server.load("textures/ui/idle.png"),
        icon_pick: asset_server.load("textures/ui/pick.png"),
        icon_axe: asset_server.load("textures/ui/axe.png"),
        icon_haul: asset_server.load("textures/ui/haul.png"),
        icon_arrow_down: asset_server.load("textures/ui/arrow_down.png"),
        icon_arrow_right: asset_server.load("textures/ui/arrow_right.png"),
        familiar_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(1024, 1024));
            // フレーム1: 左上
            layout.add_texture(URect::new(0, 0, 512, 512));
            // フレーム2: 右上
            layout.add_texture(URect::new(512, 0, 1024, 512));
            // フレーム3: 下段中央
            layout.add_texture(URect::new(256, 512, 768, 1024));
            layouts.add(layout)
        },
        glow_circle: asset_server.load("textures/ui/glow_circle.png"),
        bubble_9slice: asset_server.load("textures/ui/bubble_9slice.png"),
        // Building Visual Icons
        icon_hammer: asset_server.load("textures/ui/hammer.png"),
        icon_wood_small: asset_server.load("textures/ui/wood_small.png"),
        icon_rock_small: asset_server.load("textures/ui/rock_small.png"),
        icon_water_small: asset_server.load("textures/items/bucket/bucket_water.png"),
        icon_sand_small: asset_server.load("textures/resources/sandpile/sandpile.png"),
        icon_stasis_mud_small: asset_server.load("textures/ui/wood_small.png"),
        // Gathering Objects
        gathering_card_table: asset_server.load("textures/ui/card_table.png"),
        gathering_campfire: asset_server.load("textures/ui/campfire.png"),
        gathering_barrel: asset_server.load("textures/ui/barrel.png"),
        // New Resource & Station
        sand_pile: asset_server.load("textures/resources/sandpile/sandpile.png"),
        stasis_mud: asset_server.load("textures/stone.jpg"),
        mud_mixer: asset_server.load("textures/buildings/mud_mixer/mud mixer.png"),
        // Wheelbarrow
        wheelbarrow_empty: asset_server.load("textures/items/wheel_barrow/wheel_barrow.png"),
        wheelbarrow_loaded: asset_server.load("textures/items/wheel_barrow/wheel_barrow_full.png"),
        wheelbarrow_parking: asset_server
            .load("textures/items/wheel_barrow/wheel_barrow_parking.png"),
        icon_wheelbarrow_small: asset_server
            .load("textures/items/wheel_barrow/wheel_barrow_icon.png"),
        // Fonts
        font_ui,
        font_familiar,
        font_soul_name,
        font_soul_emoji,
    };
    commands.insert_resource(game_assets);
    info!(
        "STARTUP_TIMING: setup (assets + resources) finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn initialize_gizmo_config(mut config_store: ResMut<GizmoConfigStore>) {
    for (_, config, _) in config_store.iter_mut() {
        config.enabled = true;
        config.line.width = 1.0;
    }
}

fn populate_resource_spatial_grid(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources: Query<
        (Entity, &Transform, Option<&Visibility>),
        With<crate::systems::logistics::ResourceItem>,
    >,
) {
    let start = Instant::now();
    let mut registered_count = 0;
    for (entity, transform, visibility) in q_resources.iter() {
        let should_register = visibility
            .map(|v| *v != bevy::prelude::Visibility::Hidden)
            .unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
            registered_count += 1;
        }
    }
    info!(
        "RESOURCE_GRID: Populated {} existing resources into grid",
        registered_count
    );
    info!(
        "STARTUP_TIMING: populate_resource_spatial_grid finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn spawn_entities(spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let start = Instant::now();
    spawn_damned_souls(spawn_events);
    info!(
        "STARTUP_TIMING: spawn_entities finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn spawn_familiar_wrapper(spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    let start = Instant::now();
    crate::entities::familiar::spawn_familiar(spawn_events);
    info!(
        "STARTUP_TIMING: spawn_familiar_wrapper finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn setup_perf_scenario_if_enabled(
    mut commands: Commands,
    mut q_familiars: Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_trees: Query<Entity, With<Tree>>,
    q_rocks: Query<Entity, With<Rock>>,
) {
    let perf_enabled =
        has_cli_flag("--perf-scenario") || env::var("HW_PERF_SCENARIO").is_ok_and(|v| v == "1");
    if !perf_enabled {
        return;
    }

    let area = TaskArea {
        min: Vec2::new(-1600.0, -1600.0),
        max: Vec2::new(1600.0, 1600.0),
    };

    let mut familiar_count = 0usize;
    for (fam_entity, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(fam_entity).insert(area.clone());
        familiar_count += 1;
    }

    let mut chop_designations = 0usize;
    for tree_entity in q_trees.iter() {
        commands.entity(tree_entity).insert((
            Designation {
                work_type: WorkType::Chop,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
        chop_designations += 1;
    }

    let mut mine_designations = 0usize;
    for rock_entity in q_rocks.iter() {
        commands.entity(rock_entity).insert((
            Designation {
                work_type: WorkType::Mine,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
        mine_designations += 1;
    }

    info!(
        "PERF_SCENARIO: enabled familiars={} chop_designations={} mine_designations={}",
        familiar_count, chop_designations, mine_designations
    );
}

fn setup_perf_scenario_runtime_if_enabled(
    mut commands: Commands,
    mut applied: ResMut<PerfScenarioApplied>,
    mut q_familiars: Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
) {
    if applied.0 {
        return;
    }
    if !has_cli_flag("--perf-scenario") && !env::var("HW_PERF_SCENARIO").is_ok_and(|v| v == "1") {
        return;
    }
    if q_familiars.is_empty() {
        return;
    }

    let area = TaskArea {
        min: Vec2::new(-1600.0, -1600.0),
        max: Vec2::new(1600.0, 1600.0),
    };
    let mut familiar_count = 0usize;
    for (fam_entity, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(fam_entity).insert(area.clone());
        familiar_count += 1;
    }

    applied.0 = true;
    info!(
        "PERF_SCENARIO_RUNTIME: configured familiars={} max_controlled_soul=20",
        familiar_count
    );
}

fn create_circular_outline_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 128u32;
    let center = size as f32 / 2.0;
    let thickness = 2.0;
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();

            let dist_from_edge = (distance - (center - thickness)).abs();
            let alpha = if dist_from_edge < thickness {
                let factor = 1.0 - (dist_from_edge / thickness);
                (factor * 255.0) as u8
            } else {
                0
            };

            data.push(255);
            data.push(255);
            data.push(255);
            data.push(alpha);
        }
    }

    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );

    images.add(image)
}

fn create_circular_gradient_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 128u32;
    let center = size as f32 / 2.0;
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt() / center;

            let alpha = if distance <= 1.0 {
                ((1.0 - distance).powf(0.5) * 255.0) as u8
            } else {
                0
            };

            data.push(255);
            data.push(255);
            data.push(255);
            data.push(alpha);
        }
    }

    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );

    images.add(image)
}
