use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::events::{SquadManagementOperation, SquadManagementRequest};
use crate::interface::ui::components::{FamiliarListItem, SoulListItem, UiNodeRegistry, UiSlot};
use crate::interface::ui::theme::UiTheme;
use crate::relationships::CommandedBy;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

const DRAG_HOLD_SECONDS: f32 = 0.2;

#[derive(Resource)]
pub struct DragState {
    pending_soul: Option<Entity>,
    hold_timer: Timer,
    active_soul: Option<Entity>,
    drop_target: Option<Entity>,
    ghost_entity: Option<Entity>,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            pending_soul: None,
            hold_timer: Timer::from_seconds(DRAG_HOLD_SECONDS, TimerMode::Once),
            active_soul: None,
            drop_target: None,
            ghost_entity: None,
        }
    }
}

impl DragState {
    pub fn is_dragging(&self) -> bool {
        self.active_soul.is_some()
    }

    pub fn drop_target(&self) -> Option<Entity> {
        self.drop_target
    }
}

#[derive(Component)]
struct DragGhost;

pub fn entity_list_drag_drop_system(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    ui_nodes: Res<UiNodeRegistry>,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    mut drag_state: ResMut<DragState>,
    q_soul_rows: Query<(&Interaction, &SoulListItem), With<Button>>,
    q_familiar_rows: Query<(&Interaction, &FamiliarListItem), With<Button>>,
    q_soul_names: Query<&SoulIdentity, With<DamnedSoul>>,
    q_commanded_by: Query<&CommandedBy, With<DamnedSoul>>,
    mut squad_request_writer: MessageWriter<SquadManagementRequest>,
) {
    if buttons.just_pressed(MouseButton::Left)
        && let Some(soul_entity) = hovered_soul_row(&q_soul_rows)
    {
        drag_state.pending_soul = Some(soul_entity);
        drag_state.hold_timer = Timer::from_seconds(DRAG_HOLD_SECONDS, TimerMode::Once);
        drag_state.hold_timer.reset();
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
            reset_drag_state(&mut commands, &mut drag_state);
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
            reset_drag_state(&mut commands, &mut drag_state);
        } else if !buttons.pressed(MouseButton::Left) {
            reset_drag_state(&mut commands, &mut drag_state);
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
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_sm,
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

fn reset_drag_state(commands: &mut Commands, drag_state: &mut DragState) {
    if let Some(ghost_entity) = drag_state.ghost_entity.take() {
        commands.entity(ghost_entity).despawn();
    }
    drag_state.pending_soul = None;
    drag_state.active_soul = None;
    drag_state.drop_target = None;
    drag_state.hold_timer = Timer::from_seconds(DRAG_HOLD_SECONDS, TimerMode::Once);
    drag_state.hold_timer.reset();
}
