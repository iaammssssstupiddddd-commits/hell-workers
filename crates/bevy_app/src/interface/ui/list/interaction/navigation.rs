use crate::app_contexts::TaskContext;
use crate::input_actions::{InputAction, ResolvedInputFrame};
use crate::systems::command::TaskMode;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{
    EntityListScrollArea, EntityListScrollHint, FamiliarListItem, SoulListItem,
    UnassignedSectionArrowIcon,
};

#[derive(SystemParam)]
pub struct EntityListTabFocusCtx<'w, 's> {
    pub resolved_frame: Res<'w, ResolvedInputFrame>,
    pub task_context: Res<'w, TaskContext>,
    pub selected_entity: ResMut<'w, crate::interface::selection::SelectedEntity>,
    pub view_model: Res<'w, super::super::EntityListViewModel>,
    pub mode: Res<'w, hw_ui::components::LeftPanelMode>,
    pub minimized: Res<'w, hw_ui::list::EntityListMinimizeState>,
    pub shell: Option<Res<'w, hw_ui::shell::UiShellState>>,
    pub rows: Query<
        'w,
        's,
        (
            Option<&'static SoulListItem>,
            Option<&'static FamiliarListItem>,
            &'static ComputedNode,
            &'static UiGlobalTransform,
        ),
    >,
    pub scroll: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static mut ScrollPosition,
        ),
        With<EntityListScrollArea>,
    >,
}

pub fn entity_list_tab_focus_system(mut ctx: EntityListTabFocusCtx) {
    if *ctx.mode != hw_ui::components::LeftPanelMode::EntityList
        || ctx.minimized.minimized
        || ctx
            .shell
            .as_ref()
            .is_some_and(|shell| !shell.management_open())
    {
        return;
    }
    let reverse = if ctx.resolved_frame.contains(InputAction::ListPrevious) {
        true
    } else if ctx.resolved_frame.contains(InputAction::ListNext) {
        false
    } else {
        return;
    };

    let in_area_task_mode = matches!(ctx.task_context.0, TaskMode::AreaSelection(_));
    let mut candidates = Vec::new();
    for familiar in &ctx.view_model.current.familiars {
        candidates.push(familiar.entity);
        if !in_area_task_mode && !familiar.is_folded {
            candidates.extend(familiar.souls.iter().map(|soul| soul.entity));
        }
    }
    if !in_area_task_mode && !ctx.view_model.current.unassigned_folded {
        candidates.extend(
            ctx.view_model
                .current
                .unassigned
                .iter()
                .map(|soul| soul.entity),
        );
    }
    if candidates.is_empty() {
        return;
    }

    let current_index = ctx
        .selected_entity
        .0
        .and_then(|selected| candidates.iter().position(|&entity| entity == selected));
    let next_index = if reverse {
        current_index
            .map(|idx| (idx + candidates.len() - 1) % candidates.len())
            .unwrap_or(candidates.len().saturating_sub(1))
    } else {
        current_index
            .map(|idx| (idx + 1) % candidates.len())
            .unwrap_or(0)
    };

    ctx.selected_entity.0 = Some(candidates[next_index]);
    for (soul, familiar, row, transform) in &ctx.rows {
        if soul.map(|item| item.0).or(familiar.map(|item| item.0)) != Some(candidates[next_index]) {
            continue;
        }
        let top = transform.translation.y - row.size().y * 0.5;
        let bottom = transform.translation.y + row.size().y * 0.5;
        for (viewport, origin, mut position) in &mut ctx.scroll {
            let viewport_top = origin.translation.y - viewport.size().y * 0.5;
            let viewport_bottom = origin.translation.y + viewport.size().y * 0.5;
            let delta = if top < viewport_top {
                top - viewport_top
            } else if bottom > viewport_bottom {
                bottom - viewport_bottom
            } else {
                0.0
            };
            let inverse = viewport.inverse_scale_factor();
            let max = ((viewport.content_size().y - viewport.size().y) * inverse).max(0.0);
            position.0.y = (position.0.y + delta * inverse).clamp(0.0, max);
        }
        break;
    }
}

pub fn entity_list_scroll_hint_visibility_system(
    q_scroll: Query<&ComputedNode, With<EntityListScrollArea>>,
    mut q_hint_nodes: Query<&mut Node, With<EntityListScrollHint>>,
) {
    let has_overflow = q_scroll
        .iter()
        .next()
        .is_some_and(|computed| computed.content_size().y > computed.size().y + 1.0);

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
    view_model: Res<super::super::EntityListViewModel>,
    mut q_arrow: Query<&mut ImageNode, With<UnassignedSectionArrowIcon>>,
) {
    if view_model.is_changed() {
        let is_folded = view_model.current.unassigned_folded;
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
    use hw_ui::camera::MainCamera;

    #[test]
    fn resolved_list_direction_reaches_the_existing_focus_consumer() {
        let mut app = minimal_app();
        app.init_resource::<TaskContext>()
            .init_resource::<super::super::super::EntityListViewModel>()
            .init_resource::<hw_ui::components::LeftPanelMode>()
            .init_resource::<hw_ui::list::EntityListMinimizeState>()
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
            .resource_mut::<super::super::super::EntityListViewModel>()
            .current
            .familiars = [first, second]
            .into_iter()
            .map(|entity| hw_ui::list::FamiliarRowViewModel {
                entity,
                label: String::new(),
                is_folded: false,
                show_empty: false,
                souls: Vec::new(),
            })
            .collect();
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
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0,
            Some(second)
        );
    }
}
