use bevy::prelude::*;

use crate::components::OperationDialog;

pub fn open_operation_dialog(q_dialog: &mut Query<&mut Node, With<OperationDialog>>) {
    set_operation_dialog_display(q_dialog, Display::Flex);
}

pub fn close_operation_dialog(q_dialog: &mut Query<&mut Node, With<OperationDialog>>) {
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
