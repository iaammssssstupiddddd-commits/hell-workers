use super::*;

/// 最小化時に非表示にする DevPanel 本文のマーカー
#[derive(Component)]
pub struct DevPanelBody;

/// DevPanel 最小化・復元ボタンのマーカー
#[derive(Component)]
pub struct DevPanelMinimizeButton;

/// DevPanel 最小化・復元ボタンのラベルマーカー
#[derive(Component)]
pub struct DevPanelMinimizeButtonLabel;

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
