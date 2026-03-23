use bevy::prelude::*;
use std::collections::HashMap;

/// 共有 UI スロットから Entity を引けるレジストリ。
///
/// UI ツリーの構築は `hw_ui` が担うが、他クレートから read-only に参照されるため
/// 契約型として `hw_core` に配置する。
#[derive(Resource, Default)]
pub struct UiNodeRegistry {
    pub slots: HashMap<UiSlot, Entity>,
}

impl UiNodeRegistry {
    pub fn set_slot(&mut self, slot: UiSlot, entity: Entity) {
        self.slots.insert(slot, entity);
    }

    pub fn get_slot(&self, slot: UiSlot) -> Option<Entity> {
        self.slots.get(&slot).copied()
    }
}

/// UI ツリー上の代表ノードを引くための共有スロット識別子。
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UiSlot {
    InfoPanelRoot,
    InfoPanelStatsGroup,
    InfoPanelUnpinButton,
    Header,
    GenderIcon,
    StatMotivation,
    StatStress,
    StatFatigue,
    StatDream,
    TaskText,
    InventoryText,
    CommonText,
    DialogFamiliarName,
    DialogThresholdText,
    DialogMaxSoulText,
    ModeText,
    TaskSummaryText,
    AreaEditPreview,
    TooltipAnchor,
    FpsText,
    DreamPoolText,
    DreamPoolIcon,
}

/// 主要 UI レイヤー/親ノードの識別子。
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UiMountSlot {
    LeftPanel,
    RightPanel,
    Bottom,
    Overlay,
    TopRight,
    TopLeft,
    /// 夢の泡パーティクル専用レイヤー。パネルより後ろに描画する。
    DreamBubbleLayer,
}

/// UI ルートノードのマーカー。
#[derive(Component)]
pub struct UiRoot;
