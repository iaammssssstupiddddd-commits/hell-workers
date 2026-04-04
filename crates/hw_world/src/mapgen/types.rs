use hw_core::world::GridPos;

use crate::anchor::AnchorLayout;
use crate::world_masks::WorldMasks;
use crate::terrain::TerrainType;

/// WFC 地形生成の最終出力。hw_world → bevy_app 間のコントラクト。
///
/// すべてのフィールドは Bevy 依存なし
/// （`#[derive(Resource)]` は bevy_app 側で newtype/wrapper する）。
#[derive(Debug, Clone)]
pub struct GeneratedWorldLayout {
    // ── 地形 ────────────────────────────────────
    /// MAP_WIDTH × MAP_HEIGHT, row-major (`y * MAP_WIDTH + x`)
    pub terrain_tiles: Vec<TerrainType>,

    // ── 固定アンカー ──────────────────────────────
    pub anchors: AnchorLayout,

    // ── 診断用中間結果 ────────────────────────────
    pub masks: WorldMasks,

    // ── 資源配置候補（validator 到達確認済み） ──────
    pub resource_spawn_candidates: ResourceSpawnCandidates,

    // ── 木 ──────────────────────────────────────
    /// procedural 配置された初期木座標
    pub initial_tree_positions: Vec<GridPos>,
    /// 木の再生エリア定義（regrowth システムが参照する）
    pub forest_regrowth_zones: Vec<WfcForestZone>,

    // ── 岩 ──────────────────────────────────────
    /// procedural 配置された初期岩座標
    pub initial_rock_positions: Vec<GridPos>,

    // ── メタ ─────────────────────────────────────
    pub master_seed: u64,
    /// 何回目の試行（0-indexed）で収束したか
    pub generation_attempt: u32,
    /// MAX_WFC_RETRIES 後に deterministic fallback へ入ったか
    pub used_fallback: bool,
}

impl GeneratedWorldLayout {
    /// MS-WFC-2 実装前のスタブ。現行の固定地形を terrain_tiles に入れ、
    /// anchors と masks だけ正しく設定して返す。
    ///
    /// **注意**: このスタブは wfc-ms0 の lightweight 到達 invariant を満たすとは限らない
    /// （川・障害の配置が旧ロジックのまま）。到達保証の検証は MS-WFC-2 以降で行う。
    /// `resource_spawn_candidates` / `initial_tree_positions` / `initial_rock_positions`
    /// は MS-WFC-2 / MS-WFC-3 で埋める。
    pub fn stub(master_seed: u64) -> Self {
        use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

        use super::generate_base_terrain_tiles;
        use crate::layout::SAND_WIDTH;

        let anchors = AnchorLayout::fixed();
        let masks = WorldMasks::from_anchor(&anchors);

        GeneratedWorldLayout {
            terrain_tiles: generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, SAND_WIDTH),
            anchors,
            masks,
            resource_spawn_candidates: ResourceSpawnCandidates::default(),
            initial_tree_positions: Vec::new(),
            forest_regrowth_zones: Vec::new(),
            initial_rock_positions: Vec::new(),
            master_seed,
            generation_attempt: 0,
            used_fallback: false,
        }
    }
}

/// validator 到達確認済みの資源位置
#[derive(Debug, Clone, Default)]
pub struct ResourceSpawnCandidates {
    /// Yard から到達可能な River タイル
    pub water_tiles: Vec<GridPos>,
    /// Yard から到達可能な Sand タイル
    pub sand_tiles: Vec<GridPos>,
    /// 岩オブジェクトの候補座標（procedural 配置前）
    pub rock_candidates: Vec<GridPos>,
}

/// WFC 生成用の森林ゾーン定義（center + radius、手続き的に使う）。
///
/// 形状は wfc-ms0 §3.0 の初期提案に合わせ、チェビシェフ距離ベースの正方形 zone で固定する。
///
/// # 既存型との関係
/// `hw_world::regrowth::ForestZone` は `{ min, max, initial_count, tree_positions }` の
/// ボックス形状で固定座標を持つ旧型。MS-WFC-3 でこちらに統一し、名称も `ForestZone` に戻す。
#[derive(Debug, Clone)]
pub struct WfcForestZone {
    pub center: GridPos,
    pub radius: u32,
    // 将来: density_weight, age_category など
}

impl WfcForestZone {
    /// チェビシェフ距離（正方形 zone）で包含判定する。geometry は MS-WFC-1 で固定。
    pub fn contains(&self, pos: GridPos) -> bool {
        let dx = (pos.0 - self.center.0).abs();
        let dy = (pos.1 - self.center.1).abs();
        dx <= self.radius as i32 && dy <= self.radius as i32
    }
}
