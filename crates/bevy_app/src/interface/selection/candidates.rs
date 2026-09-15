//! A latched world point makes overlapping targets available without a click deadline.
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};
use hw_core::WorldEpoch;
use hw_core::game_state::PlayMode;
use hw_core::selection::{SelectedEntity, SelectionCandidate, SelectionTargetClass};
use hw_ui::camera::MainCamera;
use hw_ui::components::{ContextMenu, UiInputBlocker, UiInputState, UiRoot};
use hw_ui::theme::UiTheme;

use super::hit_test::SelectionResolver;
use crate::assets::GameAssets;
use crate::input_actions::ResolvedInputFrame;

#[derive(Resource, Default)]
pub(crate) struct OverlapCandidates {
    anchor: Option<Vec2>,
    entries: Vec<(Entity, SelectionTargetClass)>,
    revision: u64,
    epoch: WorldEpoch,
}

impl OverlapCandidates {
    pub(super) fn latch(&mut self, point: Vec2, candidates: &[SelectionCandidate]) {
        let entries: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.class != SelectionTargetClass::TaskArea)
            .map(|candidate| (candidate.entity, candidate.class))
            .collect();
        if self.anchor != Some(point) || self.entries != entries {
            self.anchor = Some(point);
            self.entries = entries;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn choose(&self, button: CandidateButton) -> Option<Entity> {
        button.target.filter(|target| {
            button.revision == self.revision
                && button.epoch == self.epoch
                && self.entries.iter().any(|entry| entry.0 == *target)
        })
    }
}

pub(crate) fn refresh_candidates(
    resolver: SelectionResolver,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    selected: Res<SelectedEntity>,
    epoch: Res<WorldEpoch>,
    mode: Res<State<PlayMode>>,
    ui: Res<UiInputState>,
    mut state: ResMut<OverlapCandidates>,
) {
    if state.epoch != *epoch || *mode.get() != PlayMode::Normal || ui.world_input_captured {
        if state.anchor.is_some() || state.epoch != *epoch {
            *state = OverlapCandidates {
                epoch: *epoch,
                revision: state.revision.wrapping_add(1),
                ..default()
            };
        }
        return;
    }
    let Some(point) = state.anchor else {
        return;
    };
    let Ok((camera, transform)) = cameras.single() else {
        state.anchor = None;
        state.entries.clear();
        state.revision = state.revision.wrapping_add(1);
        return;
    };
    let Ok(screen) = camera.world_to_viewport(transform, point.extend(0.0)) else {
        state.anchor = None;
        state.entries.clear();
        state.revision = state.revision.wrapping_add(1);
        return;
    };
    state.latch(
        point,
        &resolver.resolve(screen, point, camera, transform, selected.0),
    );
}

#[derive(Component)]
struct CandidatePanel;
#[derive(Component)]
struct CandidatePopup;

/// Never changed in place: a revision rebuilds the button entities, cancelling armed input.
#[derive(Component, Clone, Copy)]
struct CandidateButton {
    target: Option<Entity>,
    revision: u64,
    epoch: WorldEpoch,
}

#[derive(SystemParam)]
pub(crate) struct PickerQueries<'w, 's> {
    roots: Query<'w, 's, Entity, With<UiRoot>>,
    panels: Query<'w, 's, Entity, With<CandidatePanel>>,
    popups: Query<'w, 's, Entity, With<CandidatePopup>>,
    buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static CandidateButton,
            Ref<'static, Interaction>,
            &'static mut BackgroundColor,
        ),
    >,
    names: Query<
        'w,
        's,
        (
            Option<&'static crate::entities::damned_soul::SoulIdentity>,
            Option<&'static hw_core::familiar::Familiar>,
            Option<&'static Name>,
        ),
    >,
}

#[derive(SystemParam)]
pub(crate) struct PickerState<'w, 's> {
    candidates: Res<'w, OverlapCandidates>,
    selected: ResMut<'w, SelectedEntity>,
    ui: Res<'w, UiInputState>,
    frame: Res<'w, ResolvedInputFrame>,
    assets: Option<Res<'w, GameAssets>>,
    theme: Res<'w, UiTheme>,
    rendered: Local<'s, Option<(u64, Option<Entity>)>>,
}

pub(crate) fn candidate_picker(
    mut commands: Commands,
    mut queries: PickerQueries,
    mut state: PickerState,
) {
    let Some(assets) = state.assets.as_ref() else {
        return;
    };
    let snapshot = &state.candidates;
    let mut toggle = false;
    let mut close = false;
    for (entity, action, interaction, mut color) in &mut queries.buttons {
        if interaction.is_changed() || state.theme.is_changed() {
            hw_ui::interaction::common::update_interaction_color(
                *interaction,
                &mut color,
                &state.theme,
            );
        }
        if !state.ui.button_activated(entity)
            || state.ui.world_input_captured
            || state.frame.pointer_selection_suppressed()
        {
            continue;
        }
        if let Some(target) = snapshot.choose(*action) {
            state.selected.0 = Some(target);
            close = true;
        } else if action.target.is_none()
            && action.revision == snapshot.revision
            && action.epoch == snapshot.epoch
        {
            toggle = true;
        }
    }
    let key = (snapshot.revision, state.selected.0);
    let rebuild = *state.rendered != Some(key);
    if rebuild {
        for panel in &queries.panels {
            commands.entity(panel).despawn();
        }
        *state.rendered = Some(key);
    }
    if snapshot.entries.len() < 2 {
        return;
    }
    let panel = if rebuild || queries.panels.is_empty() {
        let Ok(root) = queries.roots.single() else {
            return;
        };
        let panel = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(45.0),
                    top: Val::Px(54.0),
                    width: Val::Px(280.0),
                    max_width: Val::Percent(40.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                CandidatePanel,
                ChildOf(root),
                GlobalZIndex(150),
            ))
            .id();
        let index = snapshot
            .entries
            .iter()
            .position(|entry| Some(entry.0) == state.selected.0)
            .map_or(0, |index| index + 1);
        spawn_button(
            &mut commands,
            panel,
            format!("候補 {index}/{} — 一覧を開く", snapshot.entries.len()),
            CandidateButton {
                target: None,
                revision: snapshot.revision,
                epoch: snapshot.epoch,
            },
            assets,
            &state.theme,
        );
        panel
    } else {
        queries.panels.single().unwrap()
    };
    if !rebuild && (close || (toggle && !queries.popups.is_empty())) {
        for popup in &queries.popups {
            commands.entity(popup).despawn();
        }
    } else if toggle && !rebuild {
        let popup = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Vh(45.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(state.theme.colors.bg_surface),
                CandidatePopup,
                ContextMenu,
                ChildOf(panel),
                UiInputBlocker,
                RelativeCursorPosition::default(),
            ))
            .id();
        let list = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollArea,
                ChildOf(popup),
                UiInputBlocker,
                RelativeCursorPosition::default(),
            ))
            .id();
        for (index, (target, class)) in snapshot.entries.iter().enumerate() {
            let fallback = match class {
                SelectionTargetClass::Familiar => "使い魔",
                SelectionTargetClass::Soul => "ソウル作業員",
                SelectionTargetClass::Resource => "資源",
                SelectionTargetClass::Object => "建物・作業対象",
                SelectionTargetClass::Floor => "床",
                SelectionTargetClass::TaskArea => "担当範囲",
            };
            let name = queries
                .names
                .get(*target)
                .ok()
                .and_then(|(soul, familiar, name)| {
                    soul.map(|soul| soul.name.as_str())
                        .or_else(|| familiar.map(|familiar| familiar.name.as_str()))
                        .or_else(|| name.map(Name::as_str))
                })
                .filter(|name| !name.is_empty())
                .unwrap_or(fallback);
            spawn_button(
                &mut commands,
                list,
                format!("{}. {name}", index + 1),
                CandidateButton {
                    target: Some(*target),
                    revision: snapshot.revision,
                    epoch: snapshot.epoch,
                },
                assets,
                &state.theme,
            );
        }
        let bar = commands
            .spawn((
                Node {
                    width: Val::Px(10.0),
                    ..default()
                },
                Scrollbar::new(list, ControlOrientation::Vertical, 24.0),
                ChildOf(popup),
            ))
            .id();
        commands.spawn((
            ScrollbarThumb::default(),
            BackgroundColor(state.theme.colors.text_muted),
            ChildOf(bar),
        ));
    }
}

fn spawn_button(
    commands: &mut Commands,
    parent: Entity,
    label: String,
    action: CandidateButton,
    assets: &GameAssets,
    theme: &UiTheme,
) {
    let button = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(32.0),
                flex_shrink: 0.0,
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            action,
            ChildOf(parent),
            BackgroundColor(theme.colors.button_default),
            UiInputBlocker,
            RelativeCursorPosition::default(),
        ))
        .id();
    commands.spawn((
        Text::new(label),
        TextFont {
            font: assets.font_ui.clone().into(),
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(theme.colors.text_primary_semantic),
        TextLayout::linebreak(LineBreak::WordOrCharacter),
        ChildOf(button),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::selection::SelectionHitKind;

    fn target(entity: Entity, class: SelectionTargetClass) -> SelectionCandidate {
        SelectionCandidate {
            entity,
            class,
            hit: SelectionHitKind::Direct,
            distance_px: 0.0,
            depth: 0.0,
        }
    }

    #[test]
    fn candidate_choice_rejects_old_point_membership_and_world_generation() {
        let first = Entity::from_bits(1);
        let second = Entity::from_bits(2);
        let entries = [
            target(first, SelectionTargetClass::Object),
            target(second, SelectionTargetClass::Floor),
        ];
        let mut state = OverlapCandidates::default();
        state.latch(Vec2::ZERO, &entries);
        let button = CandidateButton {
            target: Some(second),
            revision: state.revision,
            epoch: state.epoch,
        };
        assert_eq!(state.choose(button), Some(second));
        state.latch(Vec2::ZERO, &entries);
        assert_eq!(state.choose(button), Some(second));
        state.latch(Vec2::ONE, &entries);
        assert_eq!(state.choose(button), None);
        let current = CandidateButton {
            revision: state.revision,
            ..button
        };
        state.latch(Vec2::ONE, &entries[..1]);
        assert_eq!(state.choose(current), None);
        state.latch(Vec2::ONE, &entries);
        let current = CandidateButton {
            revision: state.revision,
            ..button
        };
        state.epoch.advance();
        assert_eq!(state.choose(current), None);
    }

    #[test]
    fn area_border_is_not_a_candidate_picker_action() {
        let mut state = OverlapCandidates::default();
        state.latch(
            Vec2::ZERO,
            &[
                target(Entity::from_bits(1), SelectionTargetClass::TaskArea),
                target(Entity::from_bits(2), SelectionTargetClass::Floor),
            ],
        );
        assert_eq!(
            state.entries,
            [(Entity::from_bits(2), SelectionTargetClass::Floor)]
        );
    }
}
