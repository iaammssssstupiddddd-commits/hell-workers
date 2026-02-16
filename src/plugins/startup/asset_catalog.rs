//! アセットカタログの生成
//!
//! Phase 5: アセットロード定義を startup から分離。アセット追加時の差分衝突を軽減。

use crate::assets::GameAssets;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// AssetServer と Images から GameAssets を構築する
pub fn create_game_assets(asset_server: &AssetServer, images: &mut Assets<Image>) -> GameAssets {
    let aura_circle = create_circular_gradient_texture(images);
    let aura_ring = create_circular_outline_texture(images);

    let font_ui = asset_server.load("fonts/NotoSansJP-VF.ttf");
    let font_familiar = asset_server.load("fonts/ShantellSans-VF.ttf");
    let font_soul_name = asset_server.load("fonts/SourceSerif4-VF.ttf");
    let font_soul_emoji = asset_server.load("fonts/NotoEmoji-VF.ttf");

    GameAssets {
        grass: asset_server.load("textures/grass.png"),
        dirt: asset_server.load("textures/dirt.png"),
        stone: asset_server.load("textures/stone.jpg"),
        river: asset_server.load("textures/river.png"),
        sand: asset_server.load("textures/sand_terrain.png"),
        familiar: asset_server.load("textures/character/familiar/imp anime 1.png"),
        familiar_anim_2: asset_server.load("textures/character/familiar/imp anime 2.png"),
        familiar_anim_3: asset_server.load("textures/character/familiar/imp anime 3.png"),
        familiar_anim_4: asset_server.load("textures/character/familiar/imp anime 4.png"),
        soul: asset_server.load("textures/character/soul.png"),
        soul_exhausted: asset_server.load("textures/character/soul_exhausted.png"),
        soul_lough: asset_server.load("textures/character/soul_lough.png"),
        soul_sleep: asset_server.load("textures/character/soul_sleep.png"),
        soul_wine: asset_server.load("textures/character/soul_wine.png"),
        soul_trump: asset_server.load("textures/character/soul_trump.png"),
        soul_stress: asset_server.load("textures/character/soul_stress.png"),
        soul_stress_breakdown: asset_server.load("textures/character/soul_stressBreakdown.png"),
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
        wood: asset_server.load("textures/dirt.png"),
        tree: asset_server.load("textures/environment/tree/tree_1.png"),
        trees: vec![
            asset_server.load("textures/environment/tree/tree_1.png"),
            asset_server.load("textures/environment/tree/tree_2.png"),
            asset_server.load("textures/environment/tree/tree_3.png"),
        ],
        tree_animes: vec![
            asset_server.load("textures/environment/tree/tree_1_anime.png"),
            asset_server.load("textures/environment/tree/tree_2_anime.png"),
            asset_server.load("textures/environment/tree/tree_3_anime.png"),
        ],
        rock: asset_server.load("textures/rock.png"),
        aura_circle,
        aura_ring,
        tank_empty: asset_server.load("textures/buildings/tank/empty_tank.png"),
        tank_partial: asset_server.load("textures/buildings/tank/half_tank.png"),
        tank_full: asset_server.load("textures/buildings/tank/full_tank.png"),
        bucket_empty: asset_server.load("textures/items/bucket/bucket_empty.png"),
        bucket_water: asset_server.load("textures/items/bucket/bucket_full.png"),
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
        glow_circle: asset_server.load("textures/ui/glow_circle.png"),
        bubble_9slice: asset_server.load("textures/ui/bubble_9slice.png"),
        icon_hammer: asset_server.load("textures/ui/hammer.png"),
        icon_wood_small: asset_server.load("textures/ui/wood_small.png"),
        icon_rock_small: asset_server.load("textures/ui/rock_small.png"),
        icon_water_small: asset_server.load("textures/items/bucket/bucket_water.png"),
        icon_sand_small: asset_server.load("textures/resources/sandpile/sandpile.png"),
        icon_bone_small: asset_server.load("textures/ui/bone_small.png"),
        icon_stasis_mud_small: asset_server.load("textures/resources/stasis_mud/stasis_mud.png"),
        gathering_card_table: asset_server.load("textures/ui/card_table.png"),
        gathering_campfire: asset_server.load("textures/ui/campfire.png"),
        gathering_barrel: asset_server.load("textures/ui/barrel.png"),
        sand_pile: asset_server.load("textures/resources/sandpile/sandpile.png"),
        bone_pile: asset_server.load("textures/resources/bone_pile/bone_pile.png"),
        stasis_mud: asset_server.load("textures/resources/stasis_mud/stasis_mud.png"),
        mud_mixer: asset_server.load("textures/buildings/mud_mixer/mud mixer.png"),
        rest_area: asset_server.load("textures/buildings/rest_area/rest_area.png"),
        mud_mixer_anim_1: asset_server.load("textures/buildings/mud_mixer/mud mixer anime 1.png"),
        mud_mixer_anim_2: asset_server.load("textures/buildings/mud_mixer/mud mixer anime 2.png"),
        mud_mixer_anim_3: asset_server.load("textures/buildings/mud_mixer/mud mixer anime 3.png"),
        mud_mixer_anim_4: asset_server.load("textures/buildings/mud_mixer/mud mixer anime 4.png"),
        wheelbarrow_empty: asset_server.load("textures/items/wheel_barrow/wheel_barrow.png"),
        wheelbarrow_loaded: asset_server.load("textures/items/wheel_barrow/wheel_barrow_full.png"),
        wheelbarrow_parking: asset_server
            .load("textures/items/wheel_barrow/wheel_barrow_parking.png"),
        icon_wheelbarrow_small: asset_server
            .load("textures/items/wheel_barrow/wheel_barrow_icon.png"),
        grass_edge: asset_server.load("textures/terrain/grass_edge.png"),
        grass_corner: asset_server.load("textures/terrain/grass_corner.png"),
        dirt_edge: asset_server.load("textures/terrain/dirt_edge.png"),
        dirt_corner: asset_server.load("textures/terrain/dirt_corner.png"),
        sand_edge: asset_server.load("textures/terrain/sand_edge.png"),
        sand_corner: asset_server.load("textures/terrain/sand_corner.png"),
        grass_inner_corner: asset_server.load("textures/terrain/grass_inner_corner.png"),
        dirt_inner_corner: asset_server.load("textures/terrain/dirt_inner_corner.png"),
        sand_inner_corner: asset_server.load("textures/terrain/sand_inner_corner.png"),
        font_ui,
        font_familiar,
        font_soul_name,
        font_soul_emoji,
    }
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
