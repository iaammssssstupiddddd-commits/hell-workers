//! Shared visual ordering for full-viewport capture overlays.

use bevy::prelude::GlobalZIndex;

pub const OPERATION_DIALOG_LAYER: GlobalZIndex = GlobalZIndex(20_010);
pub const PAUSE_LAYER: GlobalZIndex = GlobalZIndex(20_020);
pub const SETTINGS_LAYER: GlobalZIndex = GlobalZIndex(20_030);
pub const HELP_LAYER: GlobalZIndex = GlobalZIndex(20_040);
pub const LOAD_CONFIRM_LAYER: GlobalZIndex = GlobalZIndex(20_050);

const _: () = {
    assert!(LOAD_CONFIRM_LAYER.0 > HELP_LAYER.0);
    assert!(HELP_LAYER.0 > SETTINGS_LAYER.0);
    assert!(SETTINGS_LAYER.0 > PAUSE_LAYER.0);
    assert!(PAUSE_LAYER.0 > OPERATION_DIALOG_LAYER.0);
};
