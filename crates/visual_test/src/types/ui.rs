use super::*;

#[derive(Component)]
pub struct SelectedSoul;
#[derive(Component)]
pub struct MenuPanel;
#[derive(Component)]
pub struct MenuHint;
#[derive(Component)]
pub struct WorldMapTile;

// ─── パネルボタン ─────────────────────────────────────────────────────────────

/// ソウルモード専用セクション。モード切替で Node::display を制御。
#[derive(Component)]
pub struct SoulSectionNode;

/// ビルドモード専用セクション。モード切替で Node::display を制御。
#[derive(Component)]
pub struct BuildSectionNode;

/// パネル内の動的テキスト。update_dynamic_texts で値を一括更新。
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DynamicTextKind {
    ViewDir,
    Height,
    Offset,
    ShadowLayout,
    Ghost,
    Rim,
    Posterize,
    CursorPos,
}

/// パネルボタンアクション。Changed<Interaction> ハンドラで処理する。
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisualTestAction {
    SetMode(AppMode),
    // カメラ
    NextView,
    HeightDown,
    HeightUp,
    OffsetDown,
    OffsetUp,
    ResetElevation,
    SetSoulLayout(SoulLayout),
    // Soul
    SetFace(FaceExpression),
    SetFaceAll,
    SetAnimation(usize),
    SetMotion(MotionMode),
    GhostDown,
    GhostUp,
    RimDown,
    RimUp,
    PosterizeDown,
    PosterizeUp,
    ResetShader,
    AddSoul,
    RemoveSoul,
    SelectNextSoul,
    ResetSoulPos,
    // Build
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
