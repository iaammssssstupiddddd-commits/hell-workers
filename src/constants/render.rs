//! Z軸レイヤー管理 (RenderLayers)

use bevy::prelude::Color;

/// 背景マップのレイヤー
pub const Z_MAP: f32 = 0.0;
/// 地形境界オーバーレイ: Sand（Riverの上）
pub const Z_MAP_SAND: f32 = 0.01;
/// 地形境界オーバーレイ: Dirt（Sandの上）
pub const Z_MAP_DIRT: f32 = 0.02;
/// 地形境界オーバーレイ: Grass（最高優先度）
pub const Z_MAP_GRASS: f32 = 0.03;
/// Roomオーバーレイ（床より上、拾得アイテムより下）
pub const Z_ROOM_OVERLAY: f32 = 0.08;
/// 地面にあるアイテム（資材など）のベースレイヤー
pub const Z_ITEM: f32 = 0.1;
/// オーラや範囲表示のレイヤー（地面とキャラクターの間）
pub const Z_AURA: f32 = 0.2;
/// 障害物アイテム（木、岩など）のレイヤー
pub const Z_ITEM_OBSTACLE: f32 = 0.5;
/// Dream植林の魔法陣エフェクト（木の下）
pub const Z_DREAM_TREE_MAGIC_CIRCLE: f32 = 0.45;
/// Dream植林の生命力スパーク（木の上）
pub const Z_DREAM_TREE_LIFE_SPARK: f32 = 0.55;
/// Dream植林の生成位置プレビュー（木の実体より少し上）
pub const Z_DREAM_TREE_PREVIEW: f32 = 0.57;
/// 拾えるアイテム（伐採後の木材など）のレイヤー
pub const Z_ITEM_PICKUP: f32 = 0.6;
/// キャラクター（魂、使い魔）のレイヤー
pub const Z_CHARACTER: f32 = 1.0;
/// 選択インジケータやオーラのレイヤー
pub const Z_SELECTION: f32 = 2.0;
/// 作業ライン等のビジュアル効果のレイヤー
pub const Z_VISUAL_EFFECT: f32 = 3.0;
/// プログレスバー（枠）のレイヤー
pub const Z_BAR_BG: f32 = 4.0;
/// プログレスバー（中身）のレイヤー
pub const Z_BAR_FILL: f32 = 4.1;
/// 空飛ぶ文字（FloatingText）のレイヤー
pub const Z_FLOATING_TEXT: f32 = 10.0;
/// 吹き出しのZレイヤー
pub const Z_SPEECH_BUBBLE: f32 = 11.0;
/// 吹き出し背景のZレイヤー
pub const Z_SPEECH_BUBBLE_BG: f32 = 10.9;

/// Room 境界線の色（壁の上に乗せるボーダーライン）
pub const ROOM_BORDER_COLOR: Color = Color::srgba(0.2, 0.7, 1.0, 0.8);
/// Room 境界線の太さ（ピクセル）
pub const ROOM_BORDER_THICKNESS: f32 = 3.0;
