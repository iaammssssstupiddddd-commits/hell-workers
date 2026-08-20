use super::*;

#[derive(Component)]
pub struct MenuPanel;
#[derive(Component)]
pub struct MenuHint;
#[derive(Component)]
pub struct WorldMapTile;

// ─── パネルボタン ─────────────────────────────────────────────────────────────

/// Building controls container.
#[derive(Component)]
pub struct BuildSectionNode;

/// パネル内の動的テキスト。update_dynamic_texts で値を一括更新。
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DynamicTextKind {
    CursorPos,
}

/// パネルボタンアクション。Changed<Interaction> ハンドラで処理する。
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisualTestAction {
    SetBuildingKind(TestBuildingKind),
    PlaceOrRemove,
    RemoveAllBuildings,
}

// ─── ボタンカラー定数 ─────────────────────────────────────────────────────────
pub const BTN_DEF: Color = Color::Srgba(Srgba::new(0.25, 0.25, 0.30, 1.0));
pub const BTN_HOVER: Color = Color::Srgba(Srgba::new(0.35, 0.15, 0.28, 1.0));
pub const BTN_PRESS: Color = Color::Srgba(Srgba::new(0.60, 0.30, 0.08, 1.0));
pub const BTN_ACT: Color = Color::Srgba(Srgba::new(0.80, 0.40, 0.10, 1.0));
pub const BTN_ACT_H: Color = Color::Srgba(Srgba::new(0.90, 0.50, 0.20, 1.0));
