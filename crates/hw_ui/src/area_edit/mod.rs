pub mod interaction;
mod state;

pub use interaction::{apply_area_edit_drag, cursor_icon_for_operation, detect_area_edit_operation};
pub use state::{
    AreaEditClipboard, AreaEditDrag, AreaEditHandleKind, AreaEditHistory, AreaEditHistoryEntry,
    AreaEditOperation, AreaEditPresets, AreaEditSession,
};
