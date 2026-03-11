use bevy::prelude::*;

/// 壁・床スプライト（wall_connection.rs, wall_construction.rs, floor_construction.rs）
#[derive(Resource)]
pub struct WallVisualHandles {
    // 石壁 16 variants
    pub stone_isolated: Handle<Image>,
    pub stone_horizontal_left: Handle<Image>,
    pub stone_horizontal_right: Handle<Image>,
    pub stone_horizontal_both: Handle<Image>,
    pub stone_vertical_top: Handle<Image>,
    pub stone_vertical_bottom: Handle<Image>,
    pub stone_vertical_both: Handle<Image>,
    pub stone_corner_tl: Handle<Image>,
    pub stone_corner_tr: Handle<Image>,
    pub stone_corner_bl: Handle<Image>,
    pub stone_corner_br: Handle<Image>,
    pub stone_t_up: Handle<Image>,
    pub stone_t_down: Handle<Image>,
    pub stone_t_left: Handle<Image>,
    pub stone_t_right: Handle<Image>,
    pub stone_cross: Handle<Image>,
    // ドア
    pub door_closed: Handle<Image>,
    pub door_open: Handle<Image>,
    // 泥壁 16 variants
    pub mud_isolated: Handle<Image>,
    pub mud_horizontal: Handle<Image>,
    pub mud_vertical: Handle<Image>,
    pub mud_corner_tl: Handle<Image>,
    pub mud_corner_tr: Handle<Image>,
    pub mud_corner_bl: Handle<Image>,
    pub mud_corner_br: Handle<Image>,
    pub mud_t_up: Handle<Image>,
    pub mud_t_down: Handle<Image>,
    pub mud_t_left: Handle<Image>,
    pub mud_t_right: Handle<Image>,
    pub mud_cross: Handle<Image>,
    pub mud_end_top: Handle<Image>,
    pub mud_end_bottom: Handle<Image>,
    pub mud_end_left: Handle<Image>,
    pub mud_end_right: Handle<Image>,
    // 泥床
    pub mud_floor: Handle<Image>,
}

/// ビルディングアニメーション（mud_mixer.rs, tank.rs）
#[derive(Resource)]
pub struct BuildingAnimHandles {
    pub mud_mixer_idle: Handle<Image>,
    pub mud_mixer_anim_1: Handle<Image>,
    pub mud_mixer_anim_2: Handle<Image>,
    pub mud_mixer_anim_3: Handle<Image>,
    pub mud_mixer_anim_4: Handle<Image>,
    pub tank_empty: Handle<Image>,
    pub tank_partial: Handle<Image>,
    pub tank_full: Handle<Image>,
}

/// 作業アイコン（gather/worker_indicator.rs, blueprint/worker_indicator.rs）
#[derive(Resource)]
pub struct WorkIconHandles {
    pub hammer: Handle<Image>,
    pub pick: Handle<Image>,
    pub axe: Handle<Image>,
    pub haul: Handle<Image>,
    pub wheelbarrow_small: Handle<Image>,
}

/// 資材アイコン（blueprint/material_display.rs, haul/carrying_item.rs, floor/wall_construction.rs）
#[derive(Resource)]
pub struct MaterialIconHandles {
    pub wood_small: Handle<Image>,
    pub rock_small: Handle<Image>,
    pub sand_small: Handle<Image>,
    pub bone_small: Handle<Image>,
    pub stasis_mud_small: Handle<Image>,
    pub water_small: Handle<Image>,
    pub font_ui: Handle<Font>,
}

/// 運搬・アイテムスプライト（haul/carrying_item.rs）
#[derive(Resource)]
pub struct HaulItemHandles {
    pub wheelbarrow_empty: Handle<Image>,
    pub wheelbarrow_loaded: Handle<Image>,
    pub wheelbarrow_parking: Handle<Image>,
    pub bucket_empty: Handle<Image>,
    pub bucket_water: Handle<Image>,
    pub sand_pile: Handle<Image>,
    pub stasis_mud: Handle<Image>,
}

/// 会話バブル・フォント（speech/spawn.rs, speech/emitter.rs, soul.rs）
#[derive(Resource)]
pub struct SpeechHandles {
    pub bubble_9slice: Handle<Image>,
    pub glow_circle: Handle<Image>,
    pub font_familiar: Handle<Font>,
    pub font_soul_name: Handle<Font>,
    pub font_soul_emoji: Handle<Font>,
}

/// 植樹ビジュアル（plant_trees/systems.rs）
#[derive(Resource)]
pub struct PlantTreeHandles {
    pub magic_circle: Handle<Image>,
    pub life_spark: Handle<Image>,
}

/// 集会スポット visual（soul/gathering_spawn.rs）
#[derive(Resource)]
pub struct GatheringVisualHandles {
    pub aura_circle: Handle<Image>,
    pub card_table: Handle<Image>,
    pub campfire: Handle<Image>,
    pub barrel: Handle<Image>,
}
