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

#[derive(Resource, Default)]
pub struct InfoPanelNodes {
    pub root: Option<Entity>,
    pub stats_group: Option<Entity>,
    pub unpin_button: Option<Entity>,
    pub header: Option<Entity>,
    pub gender_icon: Option<Entity>,
    pub motivation: Option<Entity>,
    pub stress: Option<Entity>,
    pub fatigue: Option<Entity>,
    pub task: Option<Entity>,
    pub inventory: Option<Entity>,
    pub common: Option<Entity>,
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
    InspectEntity(Entity),
    ClearInspectPin,
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
    InfoPanelUnpinButton,
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
    AreaEditPreview,
    TooltipAnchor,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TooltipTemplate {
    Soul,
    Building,
    Resource,
    UiButton,
    #[default]
    Generic,
}

#[derive(Component)]
pub struct HoverTooltip {
    pub template_type: TooltipTemplate,
    pub delay_timer: Timer,
    pub fade_alpha: f32,
}

impl Default for HoverTooltip {
    fn default() -> Self {
        Self {
            template_type: TooltipTemplate::Generic,
            delay_timer: Timer::from_seconds(0.3, TimerMode::Once),
            fade_alpha: 0.0,
        }
    }
}

#[derive(Component)]
pub struct TooltipHeader;

#[derive(Component)]
pub struct TooltipBody;

#[derive(Component)]
pub struct TooltipProgressBar(pub f32);

#[derive(Component)]
pub struct UiTooltip {
    pub text: &'static str,
    pub shortcut: Option<&'static str>,
}

impl UiTooltip {
    pub const fn new(text: &'static str) -> Self {
        Self {
            text,
            shortcut: None,
        }
    }

    pub const fn with_shortcut(text: &'static str, shortcut: &'static str) -> Self {
        Self {
            text,
            shortcut: Some(shortcut),
        }
    }
}

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
pub struct EntityListBody;

#[derive(Component)]
pub struct EntityListScrollHint;

#[derive(Component)]
pub struct EntityListMinimizeButton;

#[derive(Component)]
pub struct EntityListMinimizeButtonLabel;

/// ヘッダーのリストコンテナ
#[derive(Component)]
pub struct FamiliarListContainer;

#[derive(Component)]
pub struct SoulListItem(pub Entity);

/// 使い魔リストアイテム（選択用）
#[derive(Component)]
pub struct FamiliarListItem(pub Entity);

#[derive(Component, Clone, Copy)]
pub struct FamiliarMaxSoulAdjustButton {
    pub familiar: Entity,
    pub delta: isize,
}

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
