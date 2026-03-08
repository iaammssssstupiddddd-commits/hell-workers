use crate::components::{UiNodeRegistry, UiSlot};
use bevy::prelude::*;

#[derive(Debug)]
pub struct ModeTextPayload {
    pub text: String,
}

#[derive(Debug)]
pub struct TaskSummaryPayload {
    pub total: u32,
    pub high: u32,
}

#[derive(Debug)]
pub struct AreaEditPreviewPayload {
    pub display: bool,
    pub text: String,
    pub left: f32,
    pub top: f32,
}

pub fn update_mode_text_system(
    payload: Option<ModeTextPayload>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    let Some(payload) = payload else {
        return;
    };

    let Some(entity) = ui_nodes.get_slot(UiSlot::ModeText) else {
        return;
    };

    if let Ok(mut text) = q_text.get_mut(entity) {
        text.0 = payload.text;
    }
}

pub fn task_summary_ui_system(
    payload: Option<TaskSummaryPayload>,
    task_high_color: Color,
    normal_color: Color,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<(&mut Text, &mut TextColor)>,
) {
    let Some(payload) = payload else {
        return;
    };

    let Some(entity) = ui_nodes.get_slot(UiSlot::TaskSummaryText) else {
        return;
    };
    if let Ok((mut text, mut color)) = q_text.get_mut(entity) {
        text.0 = format!("Tasks: {} ({} High)", payload.total, payload.high);
        if payload.high > 0 {
            color.0 = task_high_color;
        } else {
            color.0 = normal_color;
        }
    }
}

pub fn update_area_edit_preview_ui_system(
    payload: Option<AreaEditPreviewPayload>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_node: Query<&mut Node>,
    mut q_text: Query<&mut Text>,
) {
    let Some(payload) = payload else {
        return;
    };

    let Some(preview_entity) = ui_nodes.get_slot(UiSlot::AreaEditPreview) else {
        return;
    };
    let Ok(mut node) = q_node.get_mut(preview_entity) else {
        return;
    };

    if !payload.display {
        node.display = Display::None;
        return;
    }

    let Ok(mut text) = q_text.get_mut(preview_entity) else {
        return;
    };

    node.display = Display::Flex;
    node.left = Val::Px(payload.left);
    node.top = Val::Px(payload.top);
    text.0 = payload.text;
}
