use bevy::prelude::{App, Plugin};

pub type RegisterUiPlugin = fn(&mut App);

pub struct UiInfoPanelPlugin {
    register: RegisterUiPlugin,
}

impl UiInfoPanelPlugin {
    pub const fn new(register: RegisterUiPlugin) -> Self {
        Self { register }
    }
}

impl Plugin for UiInfoPanelPlugin {
    fn build(&self, app: &mut App) {
        (self.register)(app);
    }
}
