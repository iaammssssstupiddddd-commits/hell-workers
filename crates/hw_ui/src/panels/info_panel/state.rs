use super::model::InfoPanelViewModel;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct InfoPanelState {
    pub(super) last: Option<InfoPanelViewModel>,
    pub(super) last_pinned: bool,
}

#[derive(Resource, Default)]
pub struct InfoPanelPinState {
    pub entity: Option<Entity>,
}
