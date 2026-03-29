//! Soul Spa エンティティ型: SoulSpaSite / SoulSpaTile / SoulSpaPhase

use bevy::prelude::*;

use crate::components::PowerGenerator;
use crate::constants::SOUL_SPA_BONE_COST_PER_TILE;

/// Soul Spa の建設フェーズ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum SoulSpaPhase {
    #[default]
    Constructing,
    Operational,
}

/// Soul Spa サイト（2×2 施設）のルートエンティティ。
/// `#[require(PowerGenerator)]` により自動で PowerGenerator が付与される。
/// PowerGenerator のデフォルト値は `output_per_soul = OUTPUT_PER_SOUL`。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(PowerGenerator)]
pub struct SoulSpaSite {
    pub phase: SoulSpaPhase,
    /// 建設完了に必要な Bone 総数（= SOUL_SPA_BONE_COST_PER_TILE × 4）
    pub bones_required: u32,
    /// これまでに搬入された Bone 数
    pub bones_delivered: u32,
    /// 同時稼働可能 Soul 数の上限（UI で調整可能; 最大 = タイル数 = 4）
    pub active_slots: u32,
}

impl Default for SoulSpaSite {
    fn default() -> Self {
        Self {
            phase: SoulSpaPhase::Constructing,
            bones_required: SOUL_SPA_BONE_COST_PER_TILE * 4,
            bones_delivered: 0,
            active_slots: 4,
        }
    }
}

impl SoulSpaSite {
    /// Soul を追加で割り当て可能かチェック。
    pub fn has_available_slot(&self, occupied: u32) -> bool {
        self.phase == SoulSpaPhase::Operational && occupied < self.active_slots
    }
}

/// Soul Spa を構成するタイル 1 枚（2×2 で合計 4 枚）。
/// Designation(GeneratePower) + TaskSlots{max:1} は Operational 遷移時に付与される。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct SoulSpaTile {
    /// 所属する SoulSpaSite エンティティ
    pub parent_site: Entity,
    /// グリッド座標
    pub grid_pos: (i32, i32),
}
