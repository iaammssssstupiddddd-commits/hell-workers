use bevy::prelude::{App, Plugin};

pub type RegisterUiPlugin = fn(&mut App);

pub struct UiEntityListPlugin {
    register: RegisterUiPlugin,
}

impl UiEntityListPlugin {
    pub const fn new(register: RegisterUiPlugin) -> Self {
        Self { register }
    }
}

impl Plugin for UiEntityListPlugin {
    fn build(&self, app: &mut App) {
        (self.register)(app);
    }
}
