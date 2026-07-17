use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::{SquadManagementOperation, SquadManagementRequest};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use hw_core::relationships::CommandedBy;
use hw_ui::components::{FamiliarListItem, SoulListItem, UiInputState, UiNodeRegistry, UiSlot};
pub use hw_ui::list::DragState;
use hw_ui::theme::UiTheme;

#[derive(Component)]
struct DragGhost;

#[derive(SystemParam)]
pub struct DragDropResources<'w> {
    time: Res<'w, Time>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    ui_nodes: Res<'w, UiNodeRegistry>,
    game_assets: Res<'w, crate::assets::GameAssets>,
    theme: Res<'w, UiTheme>,
    resolved_frame: Res<'w, crate::input_actions::ResolvedInputFrame>,
    ui_input_state: Res<'w, UiInputState>,
    drag_state: ResMut<'w, DragState>,
    squad_request_writer: MessageWriter<'w, SquadManagementRequest>,
}

#[derive(SystemParam)]
pub struct DragDropQueries<'w, 's> {
    q_soul_rows: Query<'w, 's, (&'static Interaction, &'static SoulListItem), With<Button>>,
    q_familiar_rows: Query<'w, 's, (&'static Interaction, &'static FamiliarListItem), With<Button>>,
    q_soul_names: Query<'w, 's, &'static SoulIdentity, With<DamnedSoul>>,
    q_commanded_by: Query<'w, 's, &'static CommandedBy, With<DamnedSoul>>,
}

pub fn entity_list_drag_drop_system(
    mut commands: Commands,
    resources: DragDropResources,
    queries: DragDropQueries,
) {
    let DragDropResources {
        time,
        buttons,
        ui_nodes,
        game_assets,
        theme,
        resolved_frame,
        ui_input_state,
        mut drag_state,
        mut squad_request_writer,
    } = resources;
    let DragDropQueries {
        q_soul_rows,
        q_familiar_rows,
        q_soul_names,
        q_commanded_by,
    } = queries;
    if ui_input_state.world_input_captured || resolved_frame.pointer_selection_suppressed() {
        reset_entity_list_drag_state(&mut commands, &mut drag_state);
        return;
    }
    if buttons.just_pressed(MouseButton::Left)
        && let Some(soul_entity) = hovered_soul_row(&q_soul_rows)
    {
        drag_state.pending_soul = Some(soul_entity);
        drag_state.reset_hold_timer();
    }

    if drag_state.pending_soul.is_some() && !drag_state.is_dragging() {
        if buttons.pressed(MouseButton::Left) {
            drag_state.hold_timer.tick(time.delta());
            if drag_state.hold_timer.is_finished()
                && let Some(soul_entity) = drag_state.pending_soul
            {
                drag_state.active_soul = Some(soul_entity);
                drag_state.drop_target = None;
                spawn_drag_ghost(
                    &mut commands,
                    soul_entity,
                    &ui_nodes,
                    &game_assets,
                    &theme,
                    &q_soul_names,
                    &mut drag_state,
                );
            }
        } else {
            reset_entity_list_drag_state(&mut commands, &mut drag_state);
        }
    }

    if drag_state.is_dragging() {
        drag_state.drop_target = hovered_familiar_row(&q_familiar_rows);

        if buttons.just_released(MouseButton::Left) {
            if let (Some(soul_entity), Some(familiar_entity)) =
                (drag_state.active_soul, drag_state.drop_target)
            {
                let already_commanded = q_commanded_by
                    .get(soul_entity)
                    .ok()
                    .is_some_and(|commanded_by| commanded_by.0 == familiar_entity);
                if !already_commanded {
                    squad_request_writer.write(SquadManagementRequest {
                        familiar_entity,
                        operation: SquadManagementOperation::AddMember { soul_entity },
                    });
                }
            }
            reset_entity_list_drag_state(&mut commands, &mut drag_state);
        } else if !buttons.pressed(MouseButton::Left) {
            reset_entity_list_drag_state(&mut commands, &mut drag_state);
        }
    }
}

fn hovered_soul_row(
    q_soul_rows: &Query<(&Interaction, &SoulListItem), With<Button>>,
) -> Option<Entity> {
    q_soul_rows
        .iter()
        .find(|(interaction, _)| {
            matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
        })
        .map(|(_, soul_item)| soul_item.0)
}

fn hovered_familiar_row(
    q_familiar_rows: &Query<(&Interaction, &FamiliarListItem), With<Button>>,
) -> Option<Entity> {
    q_familiar_rows
        .iter()
        .find(|(interaction, _)| {
            matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
        })
        .map(|(_, familiar_item)| familiar_item.0)
}

fn spawn_drag_ghost(
    commands: &mut Commands,
    soul_entity: Entity,
    ui_nodes: &UiNodeRegistry,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    q_soul_names: &Query<&SoulIdentity, With<DamnedSoul>>,
    drag_state: &mut DragState,
) {
    let Some(anchor) = ui_nodes.get_slot(UiSlot::TooltipAnchor) else {
        return;
    };
    let label = q_soul_names
        .get(soul_entity)
        .map(|identity| format!("Drag: {}", identity.name))
        .unwrap_or_else(|_| "Drag: Soul".to_string());

    let ghost = commands
        .spawn((
            DragGhost,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(14.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.06, 0.09, 0.92)),
            BorderColor::all(theme.colors.border_accent),
            FocusPolicy::Pass,
            ZIndex(60),
            Name::new("Soul Drag Ghost"),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui.clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_sm),
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        })
        .id();

    commands.entity(anchor).add_child(ghost);
    drag_state.ghost_entity = Some(ghost);
}

pub(crate) fn reset_entity_list_drag_state(commands: &mut Commands, drag_state: &mut DragState) {
    if let Some(ghost_entity) = drag_state.ghost_entity.take() {
        commands.entity(ghost_entity).despawn();
    }
    drag_state.pending_soul = None;
    drag_state.active_soul = None;
    drag_state.drop_target = None;
    drag_state.reset_hold_timer();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;

    fn reset(mut commands: Commands, mut drag_state: ResMut<DragState>) {
        reset_entity_list_drag_state(&mut commands, &mut drag_state);
    }

    #[test]
    fn capture_reset_removes_drag_ghost_and_all_pending_drop_state() {
        let mut app = minimal_app();
        app.init_resource::<DragState>().add_systems(Update, reset);
        let soul = app.world_mut().spawn_empty().id();
        let familiar = app.world_mut().spawn_empty().id();
        let ghost = app.world_mut().spawn(DragGhost).id();
        {
            let mut state = app.world_mut().resource_mut::<DragState>();
            state.pending_soul = Some(soul);
            state.active_soul = Some(soul);
            state.drop_target = Some(familiar);
            state.ghost_entity = Some(ghost);
        }

        app.update();

        let state = app.world().resource::<DragState>();
        assert_eq!(state.pending_soul, None);
        assert_eq!(state.active_soul, None);
        assert_eq!(state.drop_target, None);
        assert_eq!(state.ghost_entity, None);
        assert!(app.world().get_entity(ghost).is_err());
    }
}
