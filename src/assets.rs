use bevy::prelude::*;

#[derive(Resource)]
pub struct GameAssets {
    pub grass: Handle<Image>,
    pub dirt: Handle<Image>,
    pub stone: Handle<Image>,
    pub river: Handle<Image>,
    pub sand: Handle<Image>,
    pub familiar: Handle<Image>,
    pub familiar_anim_2: Handle<Image>,
    pub familiar_anim_3: Handle<Image>,
    pub familiar_anim_4: Handle<Image>,
    // Soul
    pub soul: Handle<Image>,
    pub soul_exhausted: Handle<Image>,
    pub soul_lough: Handle<Image>,
    pub soul_sleep: Handle<Image>,
    pub soul_wine: Handle<Image>,
    pub soul_trump: Handle<Image>,
    pub soul_stress: Handle<Image>,
    pub soul_stress_breakdown: Handle<Image>,
    // Mud Wall Assets (Permanent)
    pub mud_wall_isolated: Handle<Image>,
    pub mud_wall_horizontal: Handle<Image>,
    pub mud_wall_vertical: Handle<Image>,
    pub mud_wall_corner_top_left: Handle<Image>,
    pub mud_wall_corner_top_right: Handle<Image>,
    pub mud_wall_corner_bottom_left: Handle<Image>,
    pub mud_wall_corner_bottom_right: Handle<Image>,
    pub mud_wall_t_up: Handle<Image>,
    pub mud_wall_t_down: Handle<Image>,
    pub mud_wall_t_left: Handle<Image>,
    pub mud_wall_t_right: Handle<Image>,
    pub mud_wall_cross: Handle<Image>,
    pub mud_wall_end_top: Handle<Image>,
    pub mud_wall_end_bottom: Handle<Image>,
    pub mud_wall_end_left: Handle<Image>,
    pub mud_wall_end_right: Handle<Image>,

    // Wall connections
    pub wall_isolated: Handle<Image>,
    pub wall_horizontal_left: Handle<Image>,
    pub wall_horizontal_right: Handle<Image>,
    pub wall_horizontal_both: Handle<Image>,
    pub wall_vertical_top: Handle<Image>,
    pub wall_vertical_bottom: Handle<Image>,
    pub wall_vertical_both: Handle<Image>,
    pub wall_corner_top_left: Handle<Image>,
    pub wall_corner_top_right: Handle<Image>,
    pub wall_corner_bottom_left: Handle<Image>,
    pub wall_corner_bottom_right: Handle<Image>,
    pub wall_t_up: Handle<Image>,
    pub wall_t_down: Handle<Image>,
    pub wall_t_left: Handle<Image>,
    pub wall_t_right: Handle<Image>,
    pub wall_cross: Handle<Image>,
    pub mud_floor: Handle<Image>,

    // Base Resources
    pub wood: Handle<Image>,
    pub tree: Handle<Image>,             // Legacy support (tree_1)
    pub trees: Vec<Handle<Image>>,       // Multiple tree variants
    pub tree_animes: Vec<Handle<Image>>, // Chopped tree variants
    pub rock: Handle<Image>,             // 岩のスプライト
    pub aura_circle: Handle<Image>,      // 円形オーラテクスチャ
    pub aura_ring: Handle<Image>,        // 強調用リングテクスチャ
    // Water related
    pub tank_empty: Handle<Image>,
    pub tank_partial: Handle<Image>,
    pub tank_full: Handle<Image>,
    pub bucket_empty: Handle<Image>,
    pub bucket_water: Handle<Image>,
    // UI Icons
    pub icon_male: Handle<Image>,
    pub icon_female: Handle<Image>,
    pub icon_fatigue: Handle<Image>,
    pub icon_stress: Handle<Image>,
    pub icon_idle: Handle<Image>,
    pub icon_pick: Handle<Image>,
    pub icon_axe: Handle<Image>,
    pub icon_haul: Handle<Image>,
    pub icon_water_small: Handle<Image>,
    pub icon_arrow_down: Handle<Image>,
    pub icon_arrow_right: Handle<Image>,
    pub glow_circle: Handle<Image>,   // グロー効果用
    pub bubble_9slice: Handle<Image>, // 9-slice吹き出し画像
    // Building Visual Icons
    pub icon_hammer: Handle<Image>,
    pub icon_wood_small: Handle<Image>,
    pub icon_rock_small: Handle<Image>, // 旧icon_stone_small
    pub icon_sand_small: Handle<Image>,
    pub icon_bone_small: Handle<Image>,
    pub icon_stasis_mud_small: Handle<Image>,
    // Gathering Objects
    pub gathering_card_table: Handle<Image>,
    pub gathering_campfire: Handle<Image>,
    pub gathering_barrel: Handle<Image>,
    // New Resource & Station
    pub sand_pile: Handle<Image>,
    pub bone_pile: Handle<Image>,
    pub stasis_mud: Handle<Image>,
    pub mud_mixer: Handle<Image>,
    pub rest_area: Handle<Image>,
    pub mud_mixer_anim_1: Handle<Image>,
    pub mud_mixer_anim_2: Handle<Image>,
    pub mud_mixer_anim_3: Handle<Image>,
    pub mud_mixer_anim_4: Handle<Image>,
    // Wheelbarrow
    pub wheelbarrow_empty: Handle<Image>,
    pub wheelbarrow_loaded: Handle<Image>,
    pub wheelbarrow_parking: Handle<Image>,
    pub icon_wheelbarrow_small: Handle<Image>,
    // Terrain border overlays
    pub grass_edge: Handle<Image>,
    pub grass_corner: Handle<Image>,
    pub dirt_edge: Handle<Image>,
    pub dirt_corner: Handle<Image>,
    pub sand_edge: Handle<Image>,
    pub sand_corner: Handle<Image>,
    pub grass_inner_corner: Handle<Image>,
    pub dirt_inner_corner: Handle<Image>,
    pub sand_inner_corner: Handle<Image>,
    // Fonts
    pub font_ui: Handle<Font>,         // UI全般
    pub font_familiar: Handle<Font>,   // Familiar吹き出し
    pub font_soul_name: Handle<Font>,  // Soul名
    pub font_soul_emoji: Handle<Font>, // Soulセリフ（絵文字）
}
