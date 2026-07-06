use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

use crate::components::LoadConfirmDialog;

// フィルタはジェネリック: 呼び出し側（bevy_app の IntentUiQueries）は &mut Node の
// クエリ同士を Without で disjoint にしているため、`With<OperationDialog>` 固定だと
// 型が一致しない。対象ダイアログの選別は呼び出し側のクエリフィルタに委ねる。

pub fn open_operation_dialog<F: QueryFilter>(q_dialog: &mut Query<&mut Node, F>) {
    set_dialog_display(q_dialog, Display::Flex);
}

pub fn close_operation_dialog<F: QueryFilter>(q_dialog: &mut Query<&mut Node, F>) {
    set_dialog_display(q_dialog, Display::None);
}

pub fn open_load_confirm_dialog<F: QueryFilter>(q_dialog: &mut Query<&mut Node, F>) {
    set_dialog_display(q_dialog, Display::Flex);
}

pub fn close_load_confirm_dialog<F: QueryFilter>(q_dialog: &mut Query<&mut Node, F>) {
    set_dialog_display(q_dialog, Display::None);
}

pub fn is_load_confirm_dialog_open(q_dialog: &Query<&Node, With<LoadConfirmDialog>>) -> bool {
    q_dialog
        .single()
        .is_ok_and(|node| node.display != Display::None)
}

fn set_dialog_display<F: QueryFilter>(q_dialog: &mut Query<&mut Node, F>, display: Display) {
    if let Ok(mut dialog_node) = q_dialog.single_mut() {
        dialog_node.display = display;
    }
}
