//! Z軸レイヤー管理 (RenderLayers) および描画レイヤー定数

use super::world::TILE_SIZE;
use bevy::prelude::Color;

/// Camera2d が使用する RenderLayer インデックス（デフォルトレイヤー）
pub const LAYER_2D: usize = 0;
/// Camera3d（RtT オフスクリーン）が使用する RenderLayer インデックス
pub const LAYER_3D: usize = 1;
/// RtT composite sprite 専用のオーバーレイレイヤー（矢視モード中も常時表示）
pub const LAYER_OVERLAY: usize = 2;

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
/// 建築物: 床・地面面（Z_ITEM より下）
pub const Z_BUILDING_FLOOR: f32 = 0.05;
/// 建築物: 壁・構造体（Z_ITEM より上、Z_AURA より下）
pub const Z_BUILDING_STRUCT: f32 = 0.12;
/// 建築物: 装飾レイヤー（Z_BUILDING_STRUCT の上）
pub const Z_BUILDING_DECO: f32 = 0.15;
/// 建築物: 照明・エフェクトレイヤー（Z_AURA より下）
pub const Z_BUILDING_LIGHT: f32 = 0.18;
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
/// RtT composite sprite の Z レイヤー（Overlay Camera で合成表示）
pub const Z_RTT_COMPOSITE: f32 = 20.0;

/// Camera3d（TopDown）の固定高度
pub const VIEW_HEIGHT: f32 = 150.0;
/// Camera3d（TopDown）の Z オフセット
pub const Z_OFFSET: f32 = 90.0;
/// Soul GLB PoC の初期スケール（Blender 1.0 単位をタイル基準へ揃える）
pub const SOUL_GLB_SCALE: f32 = TILE_SIZE * 0.8;

/// 斜め TopDown オーソ投影で圧縮される地面の Y 方向を、RtT 合成時に打ち消す係数。
pub fn topdown_rtt_vertical_compensation() -> f32 {
    (VIEW_HEIGHT.hypot(Z_OFFSET)) / VIEW_HEIGHT
}

/// Room 境界線の色（壁の上に乗せるボーダーライン）
pub const ROOM_BORDER_COLOR: Color = Color::srgba(0.2, 0.7, 1.0, 0.8);
/// Room 境界線の太さ（ピクセル）
pub const ROOM_BORDER_THICKNESS: f32 = 3.0;
