use bevy::prelude::*;

/// Soul task 実行中に spawn するスプライトハンドル。
#[derive(Resource)]
pub struct SoulTaskHandles {
    pub wood: Handle<Image>,
    pub tree_animes: Vec<Handle<Image>>,
    pub rock: Handle<Image>,
    pub icon_bone_small: Handle<Image>,
    pub icon_sand_small: Handle<Image>,
    pub icon_stasis_mud_small: Handle<Image>,
    pub bucket_water: Handle<Image>,
    pub bucket_empty: Handle<Image>,
}

/// Sprite を徐々に透明化して消す visual marker。
#[derive(Component)]
pub struct FadeOut {
    pub speed: f32,
}

/// 手押し車の追従表示状態。
#[derive(Component, Default)]
pub struct WheelbarrowMovement {
    pub prev_pos: Option<Vec2>,
    pub current_angle: f32,
}
