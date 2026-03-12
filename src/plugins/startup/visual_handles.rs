//! GameAssets から hw_visual のハンドルリソースを初期化するシステム

use crate::assets::GameAssets;
use bevy::prelude::*;
use hw_visual::{
    BuildingAnimHandles, GatheringVisualHandles, HaulItemHandles, MaterialIconHandles,
    PlantTreeHandles, SoulTaskHandles, SpeechHandles, WallVisualHandles, WorkIconHandles,
};

pub fn init_visual_handles(mut commands: Commands, game_assets: Res<GameAssets>) {
    commands.insert_resource(WallVisualHandles {
        stone_isolated: game_assets.wall_isolated.clone(),
        stone_horizontal_left: game_assets.wall_horizontal_left.clone(),
        stone_horizontal_right: game_assets.wall_horizontal_right.clone(),
        stone_horizontal_both: game_assets.wall_horizontal_both.clone(),
        stone_vertical_top: game_assets.wall_vertical_top.clone(),
        stone_vertical_bottom: game_assets.wall_vertical_bottom.clone(),
        stone_vertical_both: game_assets.wall_vertical_both.clone(),
        stone_corner_tl: game_assets.wall_corner_top_left.clone(),
        stone_corner_tr: game_assets.wall_corner_top_right.clone(),
        stone_corner_bl: game_assets.wall_corner_bottom_left.clone(),
        stone_corner_br: game_assets.wall_corner_bottom_right.clone(),
        stone_t_up: game_assets.wall_t_up.clone(),
        stone_t_down: game_assets.wall_t_down.clone(),
        stone_t_left: game_assets.wall_t_left.clone(),
        stone_t_right: game_assets.wall_t_right.clone(),
        stone_cross: game_assets.wall_cross.clone(),
        door_closed: game_assets.door_closed.clone(),
        door_open: game_assets.door_open.clone(),
        mud_isolated: game_assets.mud_wall_isolated.clone(),
        mud_horizontal: game_assets.mud_wall_horizontal.clone(),
        mud_vertical: game_assets.mud_wall_vertical.clone(),
        mud_corner_tl: game_assets.mud_wall_corner_top_left.clone(),
        mud_corner_tr: game_assets.mud_wall_corner_top_right.clone(),
        mud_corner_bl: game_assets.mud_wall_corner_bottom_left.clone(),
        mud_corner_br: game_assets.mud_wall_corner_bottom_right.clone(),
        mud_t_up: game_assets.mud_wall_t_up.clone(),
        mud_t_down: game_assets.mud_wall_t_down.clone(),
        mud_t_left: game_assets.mud_wall_t_left.clone(),
        mud_t_right: game_assets.mud_wall_t_right.clone(),
        mud_cross: game_assets.mud_wall_cross.clone(),
        mud_end_top: game_assets.mud_wall_end_top.clone(),
        mud_end_bottom: game_assets.mud_wall_end_bottom.clone(),
        mud_end_left: game_assets.mud_wall_end_left.clone(),
        mud_end_right: game_assets.mud_wall_end_right.clone(),
        mud_floor: game_assets.mud_floor.clone(),
    });

    commands.insert_resource(BuildingAnimHandles {
        mud_mixer_idle: game_assets.mud_mixer.clone(),
        mud_mixer_anim_1: game_assets.mud_mixer_anim_1.clone(),
        mud_mixer_anim_2: game_assets.mud_mixer_anim_2.clone(),
        mud_mixer_anim_3: game_assets.mud_mixer_anim_3.clone(),
        mud_mixer_anim_4: game_assets.mud_mixer_anim_4.clone(),
        tank_empty: game_assets.tank_empty.clone(),
        tank_partial: game_assets.tank_partial.clone(),
        tank_full: game_assets.tank_full.clone(),
    });

    commands.insert_resource(WorkIconHandles {
        hammer: game_assets.icon_hammer.clone(),
        pick: game_assets.icon_pick.clone(),
        axe: game_assets.icon_axe.clone(),
        haul: game_assets.icon_haul.clone(),
        wheelbarrow_small: game_assets.icon_wheelbarrow_small.clone(),
    });

    commands.insert_resource(MaterialIconHandles {
        wood_small: game_assets.icon_wood_small.clone(),
        rock_small: game_assets.icon_rock_small.clone(),
        sand_small: game_assets.icon_sand_small.clone(),
        bone_small: game_assets.icon_bone_small.clone(),
        stasis_mud_small: game_assets.icon_stasis_mud_small.clone(),
        water_small: game_assets.icon_water_small.clone(),
        font_ui: game_assets.font_ui.clone(),
    });

    commands.insert_resource(HaulItemHandles {
        wheelbarrow_empty: game_assets.wheelbarrow_empty.clone(),
        wheelbarrow_loaded: game_assets.wheelbarrow_loaded.clone(),
        wheelbarrow_parking: game_assets.wheelbarrow_parking.clone(),
        bucket_empty: game_assets.bucket_empty.clone(),
        bucket_water: game_assets.bucket_water.clone(),
        sand_pile: game_assets.sand_pile.clone(),
        stasis_mud: game_assets.stasis_mud.clone(),
    });

    commands.insert_resource(SpeechHandles {
        bubble_9slice: game_assets.bubble_9slice.clone(),
        glow_circle: game_assets.glow_circle.clone(),
        font_familiar: game_assets.font_familiar.clone(),
        font_soul_name: game_assets.font_soul_name.clone(),
        font_soul_emoji: game_assets.font_soul_emoji.clone(),
    });

    commands.insert_resource(PlantTreeHandles {
        magic_circle: game_assets.plant_tree_magic_circle.clone(),
        life_spark: game_assets.plant_tree_life_spark.clone(),
    });

    commands.insert_resource(GatheringVisualHandles {
        aura_circle: game_assets.aura_circle.clone(),
        card_table: game_assets.gathering_card_table.clone(),
        campfire: game_assets.gathering_campfire.clone(),
        barrel: game_assets.gathering_barrel.clone(),
    });

    commands.insert_resource(SoulTaskHandles {
        wood: game_assets.wood.clone(),
        tree_animes: game_assets.tree_animes.clone(),
        rock: game_assets.rock.clone(),
        icon_bone_small: game_assets.icon_bone_small.clone(),
        icon_sand_small: game_assets.icon_sand_small.clone(),
        icon_stasis_mud_small: game_assets.icon_stasis_mud_small.clone(),
        bucket_water: game_assets.bucket_water.clone(),
        bucket_empty: game_assets.bucket_empty.clone(),
    });
}
