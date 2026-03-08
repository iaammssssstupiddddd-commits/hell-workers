use bevy::prelude::{App, Plugin};

pub type RegisterUiPlugin = fn(&mut App);

pub struct UiTooltipPlugin {
    register: RegisterUiPlugin,
}

impl UiTooltipPlugin {
    pub const fn new(register: RegisterUiPlugin) -> Self {
        Self { register }
    }
}
impl Plugin for UiTooltipPlugin {
    fn build(&self, app: &mut App) {
        (self.register)(app);
    }
}
