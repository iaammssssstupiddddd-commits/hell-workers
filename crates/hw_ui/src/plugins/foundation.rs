use crate::components::{
    PlacementFailureTooltip, SectionFolded, UiInputState, UiNodeRegistry, UnassignedFolded,
};
use crate::theme::UiTheme;
use bevy::prelude::*;

pub struct UiFoundationPlugin;

impl Plugin for UiFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SectionFolded>();
        app.register_type::<UnassignedFolded>();
        app.init_resource::<UiInputState>();
        app.init_resource::<PlacementFailureTooltip>();
        app.init_resource::<UiNodeRegistry>();
        app.init_resource::<UiTheme>();
    }
}
