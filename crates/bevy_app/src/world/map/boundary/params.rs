//! 境界メッシュ生成パラメータ定数

use hw_core::constants::TILE_SIZE;

/// ノイズの空間周波数（弧長ワールド単位に対する周波数）。
/// 約 3 タイル分（96 ワールド単位）で 1 周期。
pub(crate) const NOISE_FREQ: f32 = 1.0 / (TILE_SIZE * 3.0);

/// ノイズの最大変位量（ワールド単位）。
/// 隣セル中心 TILE_SIZE/2 = 16.0 未満に抑え、論理境界と視覚の乖離を防ぐ。
pub(crate) const NOISE_AMPLITUDE: f32 = TILE_SIZE * 0.375; // 12.0

/// Catmull-Rom スプライン 1 セグメントあたりのサンプル数。
pub(crate) const CATMULL_ROM_STEPS: u32 = 8;

/// 面取り（Chamfer）ベベル距離（ワールド単位）。
/// 川岸 1 タイル段差（32wu）の 35% を面取りし、Catmull-Rom のオーバーシュートを抑制する。
pub(crate) const CHAMFER_DISTANCE: f32 = TILE_SIZE * 0.35; // ≈ 11.2wu

/// 面取りを適用するコーナー角のコサイン閾値。
/// cos(60°) = 0.5: それより鋭い角（0°〜60°未満）のコーナーのみ面取りする。
/// 川岸の 90° ステップ（cos = 0）はこの閾値に確実に掛かる。
pub(crate) const CHAMFER_COS_THRESHOLD: f32 = 0.5;

/// terrain_region_map テクスチャの解像度（1 辺のピクセル数）。
/// MAP_WIDTH=100 に対して 10.24 px/tile（1024 にすると 5.12 の倍精細でジャギーが減る）。
pub(crate) const TERRAIN_REGION_RES: usize = 1024;
pub(crate) const BOUNDARY_PROXIMITY_RES: usize = 256;
pub(crate) const BOUNDARY_PROXIMITY_DILATION_PX: i32 = 5;

/// terrain_region_map のセンチネル値。River(255) と区別するため 254 を使う。
/// BFS flood fill のバリア壁として機能し、最終的に短 dilation で隣接値に置き換えられる。
pub(crate) const TERRAIN_REGION_SENTINEL: u8 = 254;

/// terrain_region_map の未割当値。BFS で塗りつぶされる前の初期値。
/// 11 値エンコーディング (0,1,2,85,86,87,170,171,172,255) および SENTINEL(254) と衝突しない。
pub(crate) const TERRAIN_REGION_UNASSIGNED: u8 = 253;
