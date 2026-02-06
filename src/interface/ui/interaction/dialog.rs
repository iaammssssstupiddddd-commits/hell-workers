use bevy::prelude::*;

use crate::interface::ui::components::OperationDialog;

pub(super) fn open_operation_dialog(q_dialog: &mut Query<&mut Node, With<OperationDialog>>) {
    set_operation_dialog_display(q_dialog, Display::Flex);
}

pub(super) fn close_operation_dialog(q_dialog: &mut Query<&mut Node, With<OperationDialog>>) {
    set_operation_dialog_display(q_dialog, Display::None);
}

fn set_operation_dialog_display(
    q_dialog: &mut Query<&mut Node, With<OperationDialog>>,
    display: Display,
) {
    if let Ok(mut dialog_node) = q_dialog.single_mut() {
        dialog_node.display = display;
    }
}
