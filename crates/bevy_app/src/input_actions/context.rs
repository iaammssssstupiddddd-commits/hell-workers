/// Frame-local state used by the pure input resolver.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputContextSnapshot {
    pub text_input_blocks_keybinds: bool,
    pub has_in_progress_gesture: bool,
}
