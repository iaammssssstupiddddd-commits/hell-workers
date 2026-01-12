//! UIコンポーネント定義
//!
//! UIの列挙型とコンポーネント構造体を定義します。

use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use bevy::prelude::*;

// ============================================================
// UI列挙型
// ============================================================

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
pub struct InfoPanelJobText;

#[derive(Component)]
pub struct InfoPanelHeader;

#[derive(Component)]
pub struct ModeText;

#[derive(Component)]
pub struct ContextMenu;

#[derive(Component)]
pub struct TaskSummaryText;

#[derive(Component)]
pub struct HoverTooltipText;

#[derive(Component)]
pub struct HoverTooltip;

#[derive(Component)]
pub struct OperationDialog;

#[derive(Component)]
pub struct OperationDialogFamiliarName;

#[derive(Component)]
pub struct OperationDialogThresholdText;

#[derive(Component)]
pub struct OperationDialogMaxSoulText;
