use super::*;

#[derive(Resource)]
pub struct TestState {
    pub menu_visible: bool,
    pub building_kind: TestBuildingKind,
    pub building_cursor: (i32, i32),
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            menu_visible: true,
            building_kind: TestBuildingKind::Wall,
            building_cursor: (50, 50),
        }
    }
}
