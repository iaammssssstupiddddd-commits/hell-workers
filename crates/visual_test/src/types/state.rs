use super::*;

#[derive(Clone, Copy, Debug)]
pub enum FaceMode {
    Single(FaceExpression),
    AllDifferent,
}

#[derive(Resource)]
pub struct TestState {
    pub mode: AppMode,
    pub face_mode: FaceMode,
    pub motion: MotionMode,
    pub soul_layout: SoulLayout,
    pub soul_count: usize,
    pub next_index: usize,
    pub anim_clip_idx: usize,
    pub view_height: f32,
    pub z_offset: f32,
    pub ghost_alpha: f32,
    pub rim_strength: f32,
    pub posterize_steps: f32,
    pub menu_visible: bool,
    pub building_kind: TestBuildingKind,
    pub building_cursor: (i32, i32),
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            mode: AppMode::Soul,
            face_mode: FaceMode::Single(FaceExpression::Normal),
            motion: MotionMode::Idle,
            soul_layout: SoulLayout::Default,
            soul_count: 0,
            next_index: 0,
            anim_clip_idx: 0,
            view_height: VIEW_HEIGHT,
            z_offset: Z_OFFSET,
            ghost_alpha: DEFAULT_GHOST_ALPHA,
            rim_strength: DEFAULT_RIM_STRENGTH,
            posterize_steps: DEFAULT_POSTERIZE_STEPS,
            menu_visible: true,
            building_kind: TestBuildingKind::Wall,
            building_cursor: (50, 50),
        }
    }
}
