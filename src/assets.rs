use bevy::prelude::*;

#[derive(Resource)]
pub struct GameAssets {
    pub grass: Handle<Image>,
    pub dirt: Handle<Image>,
    pub stone: Handle<Image>,
    pub river: Handle<Image>,
    pub sand: Handle<Image>,
    pub colonist: Handle<Image>,
    pub familiar: Handle<Image>,
    pub wall: Handle<Image>,
    pub wood: Handle<Image>,
    pub tree: Handle<Image>,        // 木のスプライト
    pub rock: Handle<Image>,        // 岩のスプライト
    pub aura_circle: Handle<Image>, // 円形オーラテクスチャ
    pub aura_ring: Handle<Image>,   // 強調用リングテクスチャ
    // Water related
    pub tank_empty: Handle<Image>,
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
    pub familiar_layout: Handle<TextureAtlasLayout>,
    pub glow_circle: Handle<Image>,   // グロー効果用
    pub bubble_9slice: Handle<Image>, // 9-slice吹き出し画像
    // Building Visual Icons
    pub icon_hammer: Handle<Image>,
    pub icon_wood_small: Handle<Image>,
    pub icon_rock_small: Handle<Image>, // 旧icon_stone_small
    // Fonts
    pub font_ui: Handle<Font>,         // UI全般
    pub font_familiar: Handle<Font>,   // Familiar吹き出し
    pub font_soul_name: Handle<Font>,  // Soul名
    pub font_soul_emoji: Handle<Font>, // Soulセリフ（絵文字）
}
