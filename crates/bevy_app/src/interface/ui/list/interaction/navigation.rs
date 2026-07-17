use crate::app_contexts::TaskContext;
use crate::input_actions::{InputAction, ResolvedInputFrame};
use crate::systems::command::TaskMode;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::camera::MainCamera;
use hw_ui::components::{
    EntityListScrollHint, FamiliarListItem, SoulListItem, UnassignedFolded,
    UnassignedSectionArrowIcon, UnassignedSoulContent, UnassignedSoulSection,
};

#[derive(SystemParam)]
pub struct EntityListTabFocusCtx<'w, 's> {
    pub resolved_frame: Res<'w, ResolvedInputFrame>,
    pub task_context: Res<'w, TaskContext>,
    pub selected_entity: ResMut<'w, crate::interface::selection::SelectedEntity>,
    pub soul_items: Query<'w, 's, &'static SoulListItem>,
    pub familiar_items: Query<'w, 's, &'static FamiliarListItem>,
    pub camera: Query<'w, 's, &'static mut Transform, With<MainCamera>>,
    pub transforms: Query<'w, 's, &'static GlobalTransform>,
}

pub fn entity_list_tab_focus_system(mut ctx: EntityListTabFocusCtx) {
    let reverse = if ctx.resolved_frame.contains(InputAction::ListPrevious) {
        true
    } else if ctx.resolved_frame.contains(InputAction::ListNext) {
        false
    } else {
        return;
    };

    let in_area_task_mode = matches!(ctx.task_context.0, TaskMode::AreaSelection(_));
    let mut candidates: Vec<Entity> = if in_area_task_mode {
        ctx.familiar_items.iter().map(|item| item.0).collect()
    } else {
        ctx.familiar_items
            .iter()
            .map(|item| item.0)
            .chain(ctx.soul_items.iter().map(|item| item.0))
            .collect()
    };
    candidates.sort_by_key(|entity| entity.index());
    candidates.dedup();
    if candidates.is_empty() {
        return;
    }

    let current_index = ctx
        .selected_entity
        .0
        .and_then(|selected| candidates.iter().position(|&entity| entity == selected));
    let next_index = if reverse {
        current_index
            .map(|idx| idx.saturating_sub(1))
            .unwrap_or(candidates.len().saturating_sub(1))
    } else {
        current_index
            .map(|idx| (idx + 1) % candidates.len())
            .unwrap_or(0)
    };

    hw_ui::list::select_entity_and_focus_camera(
        candidates[next_index],
        "tab-focus",
        &mut ctx.selected_entity,
        &mut ctx.camera,
        &ctx.transforms,
    );
}

pub fn entity_list_scroll_hint_visibility_system(
    q_unassigned_section: Query<Has<UnassignedFolded>, With<UnassignedSoulSection>>,
    q_unassigned_content: Query<&ComputedNode, With<UnassignedSoulContent>>,
    mut q_hint_nodes: Query<&mut Node, With<EntityListScrollHint>>,
) {
    let unassigned_folded = q_unassigned_section.iter().next().unwrap_or(false);
    let has_overflow = if unassigned_folded {
        false
    } else {
        q_unassigned_content
            .iter()
            .next()
            .is_some_and(|computed| computed.content_size().y > computed.size().y + 1.0)
    };

    let desired = if has_overflow {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in q_hint_nodes.iter_mut() {
        if node.display != desired {
            node.display = desired;
        }
    }
}

pub fn update_unassigned_arrow_icon_system(
    game_assets: Res<crate::assets::GameAssets>,
    unassigned_folded_query: Query<
        Has<UnassignedFolded>,
        (With<UnassignedSoulSection>, Changed<UnassignedFolded>),
    >,
    mut q_arrow: Query<&mut ImageNode, With<UnassignedSectionArrowIcon>>,
) {
    if let Some(is_folded) = unassigned_folded_query.iter().next() {
        for mut icon in q_arrow.iter_mut() {
            icon.image = if is_folded {
                game_assets.icon_arrow_right.clone()
            } else {
                game_assets.icon_arrow_down.clone()
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::InputModifiers;
    use crate::test_support::minimal_app;

    #[test]
    fn resolved_list_direction_reaches_the_existing_focus_consumer() {
        let mut app = minimal_app();
        app.init_resource::<TaskContext>()
            .init_resource::<crate::interface::selection::SelectedEntity>()
            .init_resource::<ResolvedInputFrame>()
            .add_systems(Update, entity_list_tab_focus_system);
        app.world_mut().spawn((Transform::default(), MainCamera));
        let first = app
            .world_mut()
            .spawn(GlobalTransform::from_translation(Vec3::ZERO))
            .id();
        let second = app
            .world_mut()
            .spawn(GlobalTransform::from_translation(Vec3::X))
            .id();
        app.world_mut().spawn(SoulListItem(first));
        app.world_mut().spawn(SoulListItem(second));
        app.world_mut()
            .resource_mut::<crate::interface::selection::SelectedEntity>()
            .0 = Some(second);
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::ListPrevious],
                None,
                true,
            );

        app.update();

        assert_eq!(
            app.world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0,
            Some(first)
        );
    }
}
