use bevy::prelude::*;

#[derive(Resource)]
pub struct GameAssets {
    pub grass: Handle<Image>,
    pub dirt: Handle<Image>,
    pub stone: Handle<Image>,
    pub colonist: Handle<Image>,
    pub wall: Handle<Image>,
    pub wood: Handle<Image>,
    pub aura_circle: Handle<Image>, // 円形オーラテクスチャ
    pub aura_ring: Handle<Image>,   // 強調用リングテクスチャ
    // UI Icons
    pub icon_male: Handle<Image>,
    pub icon_female: Handle<Image>,
    pub icon_fatigue: Handle<Image>,
    pub icon_stress: Handle<Image>,
    pub icon_idle: Handle<Image>,
    pub icon_pick: Handle<Image>,
    pub icon_haul: Handle<Image>,
    pub icon_arrow_down: Handle<Image>,
    pub icon_arrow_right: Handle<Image>,
}
