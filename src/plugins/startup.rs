//! スタートアップ関連のプラグイン

use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::game_state::{BuildContext, TaskContext, ZoneContext};
use crate::interface::camera::{MainCamera, PanCamera};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, setup_ui};
use crate::systems::logistics::{ResourceLabels, initial_resource_spawner};
use crate::systems::soul_ai::work::AutoHaulCounter;
use crate::systems::spatial::{
    FamiliarSpatialGrid, ResourceSpatialGrid, SpatialGrid, SpatialGridOps,
};
use crate::systems::task_queue::{GlobalTaskQueue, TaskQueue};
use crate::systems::time::GameTime;
use crate::world::map::{WorldMap, spawn_map};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::NoIndirectDrawing;

pub struct StartupPlugin;

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
            .init_resource::<ResourceLabels>()
            .init_resource::<GameTime>()
            .init_resource::<TaskContext>()
            .init_resource::<SpatialGrid>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<AutoHaulCounter>()
            .init_resource::<TaskQueue>()
            .init_resource::<GlobalTaskQueue>()
            // Startup systems
            .add_systems(Startup, (setup, initialize_gizmo_config))
            .add_systems(
                PostStartup,
                (
                    spawn_map,
                    spawn_entities,
                    spawn_familiar_wrapper,
                    setup_ui,
                    initial_resource_spawner,
                    populate_resource_spatial_grid,
                )
                    .chain(),
            );
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
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
    let font_narrator = asset_server.load("fonts/NotoSerifJP-VF.ttf");
    let font_familiar = asset_server.load("fonts/ShantellSans-VF.ttf");
    let font_soul_name = asset_server.load("fonts/SourceSerif4-VF.ttf");
    let font_soul_emoji = asset_server.load("fonts/NotoEmoji-VF.ttf");

    let game_assets = GameAssets {
        grass: asset_server.load("textures/grass.jpg"),
        dirt: asset_server.load("textures/dirt.jpg"),
        stone: asset_server.load("textures/stone.jpg"),
        colonist: asset_server.load("textures/colonist.png"),
        familiar: asset_server.load("textures/familiar_spritesheet.png"),
        wall: asset_server.load("textures/stone.jpg"),
        wood: asset_server.load("textures/dirt.jpg"),
        aura_circle,
        aura_ring,
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
        // Building Visual Icons
        icon_hammer: asset_server.load("textures/ui/hammer.png"),
        icon_wood_small: asset_server.load("textures/ui/wood_small.png"),
        icon_stone_small: asset_server.load("textures/ui/stone_small.png"),
        // Fonts
        font_ui,
        font_narrator,
        font_familiar,
        font_soul_name,
        font_soul_emoji,
    };
    commands.insert_resource(game_assets);
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
}

fn spawn_entities(spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    spawn_damned_souls(spawn_events);
}

fn spawn_familiar_wrapper(spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    crate::entities::familiar::spawn_familiar(spawn_events);
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
