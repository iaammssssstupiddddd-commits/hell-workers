use bevy::prelude::{App, Plugin};

pub type RegisterUiPlugin = fn(&mut App);

pub struct UiCorePlugin {
    register: RegisterUiPlugin,
}

impl UiCorePlugin {
    pub const fn new(register: RegisterUiPlugin) -> Self {
        Self { register }
    }
}

impl Plugin for UiCorePlugin {
    fn build(&self, app: &mut App) {
        (self.register)(app);
    }
}
