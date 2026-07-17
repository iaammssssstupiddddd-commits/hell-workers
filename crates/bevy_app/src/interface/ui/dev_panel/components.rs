use super::*;

/// LOD インジケーターテキストのマーカー
#[derive(Component)]
pub struct LodIndicatorText;

/// RtT / Soul mask / Light 状態表示テキストのマーカー
#[derive(Component)]
pub struct RenderPerfStatusText;

/// Soul mask トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleSoulMaskButton;

/// RtT directional light トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttLightButton;

/// 追加 RtT directional light トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttExtraLightButton;

/// RtT terrain トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttTerrainButton;

/// RtT scene object トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttSceneObjectsButton;

/// 3D表示切り替えボタンのマーカー
#[derive(Component)]
pub struct ToggleRender3dButton;

/// 即時ビルドトグルボタンのマーカー
#[derive(Component)]
pub struct InstantBuildButton;
