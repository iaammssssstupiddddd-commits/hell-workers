//! Soul Spa エンティティ型: SoulSpaSite / SoulSpaTile / SoulSpaPhase

use bevy::prelude::*;

use crate::components::PowerGenerator;
use crate::constants::{SOUL_SPA_BONE_COST_PER_TILE, SOUL_SPA_MAX_ACTIVE_SLOTS};

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
            active_slots: SOUL_SPA_MAX_ACTIVE_SLOTS,
        }
    }
}

impl SoulSpaSite {
    pub fn clamped_active_slots(requested: u32) -> u32 {
        requested.min(SOUL_SPA_MAX_ACTIVE_SLOTS)
    }

    /// 永続値・UI入力を施設の物理タイル数へ正規化する。
    pub fn normalize_active_slots(&mut self) {
        self.active_slots = Self::clamped_active_slots(self.active_slots);
    }

    /// 稼働枠を正規化して適用し、実際に保存された値を返す。
    pub fn set_active_slots(&mut self, requested: u32) -> u32 {
        self.active_slots = Self::clamped_active_slots(requested);
        self.active_slots
    }

    /// Soul を追加で割り当て可能かチェック。
    pub fn has_available_slot(&self, occupied: u32) -> bool {
        self.phase == SoulSpaPhase::Operational && occupied < self.active_slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_slots_are_clamped_at_the_domain_boundary() {
        let mut site = SoulSpaSite::default();
        assert_eq!(site.set_active_slots(2), 2);
        assert_eq!(site.set_active_slots(u32::MAX), SOUL_SPA_MAX_ACTIVE_SLOTS);

        site.active_slots = 99;
        site.normalize_active_slots();
        assert_eq!(site.active_slots, SOUL_SPA_MAX_ACTIVE_SLOTS);
    }

    #[test]
    fn lowering_slots_only_closes_the_new_assignment_gate() {
        let mut site = SoulSpaSite {
            phase: SoulSpaPhase::Operational,
            ..default()
        };
        site.set_active_slots(2);

        assert!(!site.has_available_slot(4));
        assert!(!site.has_available_slot(2));
        assert!(site.has_available_slot(1));
    }
}

/// Soul Spa を構成するタイル 1 枚（2×2 で合計 4 枚）。
/// Designation(GeneratePower) + TaskSlots{max:1} は Operational 遷移時に付与される。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct SoulSpaTile {
    /// 所属する SoulSpaSite エンティティ
    #[entities]
    pub parent_site: Entity,
    /// グリッド座標
    pub grid_pos: (i32, i32),
}

/// Soul Spa稼働枠UI操作1件に対する終端結果。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulSpaSlotsChangeOutcome {
    pub target: Entity,
    pub status: SoulSpaSlotsChangeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulSpaSlotsChangeStatus {
    Applied {
        requested: u32,
        applied: u32,
        clamped: bool,
    },
    StaleTarget,
    UnsupportedTarget,
    PhaseUnavailable,
}
