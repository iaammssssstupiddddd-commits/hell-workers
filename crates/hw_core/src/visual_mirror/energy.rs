use bevy::prelude::*;

/// Outdoor Lamp の電源状態を hw_visual に伝える VisualMirror。
/// `on_power_consumer_visual_added` Observer が初期値を設定し、
/// `on_unpowered_added` / `on_unpowered_removed` Observer が更新する。
#[derive(Component, Default)]
pub struct PoweredVisualState {
    pub is_powered: bool,
}
