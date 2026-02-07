//! UIコンポーネント定義
//!
//! UIの列挙型とコンポーネント構造体を定義します。

use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================
// UI列挙型
// ============================================================

#[derive(Resource, Default)]
pub struct UiInputState {
    pub pointer_over_ui: bool,
}

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

#[derive(Resource, Default, Debug, Clone, Copy)]
pub enum MenuState {
    #[default]
    Hidden,
    Architect,
    Zones,
    Orders,
}

#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    ToggleArchitect,
    ToggleZones,
    ToggleOrders,
    SelectBuild(BuildingType),
    SelectZone(ZoneType),
    SelectTaskMode(crate::systems::command::TaskMode),
    SelectAreaTask,
    OpenOperationDialog,
    AdjustFatigueThreshold(f32),
    AdjustMaxControlledSoul(isize),
    CloseDialog,
}

// ============================================================
// UiSlot - 統一UIスロットコンポーネント
// ============================================================

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UiSlot {
    InfoPanelRoot,
    InfoPanelStatsGroup,
    // Info Panel
    Header,
    GenderIcon,
    StatMotivation,
    StatStress,
    StatFatigue,
    TaskText,
    InventoryText,
    CommonText,
    // Dialog
    DialogFamiliarName,
    DialogThresholdText,
    DialogMaxSoulText,
    // Bottom bar
    ModeText,
    // Other
    TaskSummaryText,
    HoverTooltipText,
    FpsText,
}

// ============================================================
// UIコンポーネント
// ============================================================

#[derive(Component)]
pub struct MenuButton(pub MenuAction);

#[derive(Component)]
pub struct ArchitectSubMenu;

#[derive(Component)]
pub struct ZonesSubMenu;

#[derive(Component)]
pub struct OrdersSubMenu;

#[derive(Component)]
pub struct InfoPanel;

#[derive(Component)]
pub struct ContextMenu;

#[derive(Component)]
pub struct HoverTooltip;

#[derive(Component)]
pub struct UiTooltip(pub &'static str);

#[derive(Component, Default)]
pub struct UiInputBlocker;

#[derive(Component)]
pub struct UiRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UiMountSlot {
    LeftPanel,
    RightPanel,
    Bottom,
    Overlay,
    TopRight,
    TopLeft,
}

#[derive(Component)]
pub struct OperationDialog;

// ============================================================
// エンティティリスト UI コンポーネント
// ============================================================

#[derive(Component)]
pub struct EntityListPanel;

#[derive(Component)]
pub struct EntityListScrollHint;

/// ヘッダーのリストコンテナ
#[derive(Component)]
pub struct FamiliarListContainer;

#[derive(Component)]
pub struct SoulListItem(pub Entity);

/// 使い魔リストアイテム（選択用）
#[derive(Component)]
pub struct FamiliarListItem(pub Entity);

#[derive(Component)]
pub struct UnassignedSoulSection;

#[derive(Component)]
pub struct UnassignedSoulContent;

#[derive(Component)]
pub struct UiScrollArea {
    pub speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityListSectionType {
    Familiar(Entity),
    Unassigned,
}

/// セクションの折りたたみイベント等に使う識別
#[derive(Component)]
pub struct SectionToggle(pub EntityListSectionType);

/// 未所属ソウルセクションの矢印アイコン（動的更新用）
#[derive(Component)]
pub struct UnassignedSectionArrowIcon;

/// セクションが折りたたまれていることを示すコンポーネント
#[derive(Component, Default, Debug, Reflect)]
#[reflect(Component)]
pub struct SectionFolded;

/// 未所属セクションが折りたたまれていることを示すコンポーネント
/// 未所属セクションのエンティティに付与される
#[derive(Component, Default, Debug, Reflect)]
#[reflect(Component)]
pub struct UnassignedFolded;
