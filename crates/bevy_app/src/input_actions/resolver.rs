use bevy::prelude::*;
use hw_ui::area_edit::AreaEditSession;
use hw_ui::components::UiInputState;

use super::bindings::{DEFAULT_BINDINGS, InputBinding, actions_are_compatible};
use super::{InputAction, InputChord, InputContextSnapshot, InputModifiers};

/// Actions and modifier state resolved once for the current frame.
#[derive(Resource, Debug, Default)]
pub struct ResolvedInputFrame {
    actions: Vec<InputAction>,
    pub modifiers: InputModifiers,
}

impl ResolvedInputFrame {
    pub fn actions(&self) -> &[InputAction] {
        &self.actions
    }

    pub fn contains(&self, action: InputAction) -> bool {
        self.actions.contains(&action)
    }

    pub(crate) fn replace(&mut self, modifiers: InputModifiers, actions: Vec<InputAction>) {
        self.modifiers = modifiers;
        self.actions = actions;
    }
}

/// Resolves exact chords without reading or mutating Bevy input resources.
pub fn resolve_input_chords(
    pressed_chords: &[InputChord],
    context: InputContextSnapshot,
) -> Vec<InputAction> {
    resolve_input_chords_with_bindings(pressed_chords, context, DEFAULT_BINDINGS)
}

pub(super) fn resolve_input_chords_with_bindings(
    pressed_chords: &[InputChord],
    context: InputContextSnapshot,
    bindings: &[InputBinding],
) -> Vec<InputAction> {
    if context.text_input_blocks_keybinds {
        return Vec::new();
    }

    let candidates = bindings
        .iter()
        .filter(|binding| pressed_chords.contains(&binding.chord))
        .filter(|binding| {
            !(context.has_in_progress_gesture && binding.action == InputAction::SaveGame)
        });

    let mut family_winners: Vec<&InputBinding> = Vec::new();
    for binding in candidates {
        if family_winners
            .iter()
            .any(|existing| existing.action == binding.action)
        {
            continue;
        }

        let Some(family) = binding.exclusive_family else {
            family_winners.push(binding);
            continue;
        };

        if let Some(existing) = family_winners
            .iter_mut()
            .find(|existing| existing.exclusive_family == Some(family))
        {
            if binding.family_priority > existing.family_priority {
                *existing = binding;
            }
        } else {
            family_winners.push(binding);
        }
    }

    family_winners.sort_by(|left, right| {
        right
            .resolution_priority
            .cmp(&left.resolution_priority)
            .then_with(|| {
                let order = |action| {
                    bindings
                        .iter()
                        .position(|binding| binding.action == action)
                        .unwrap_or(usize::MAX)
                };
                order(left.action).cmp(&order(right.action))
            })
    });

    let mut resolved: Vec<&InputBinding> = Vec::new();
    for binding in family_winners {
        if resolved
            .iter()
            .all(|existing| actions_are_compatible(existing, binding))
        {
            resolved.push(binding);
        }
    }
    resolved.iter().map(|binding| binding.action).collect()
}

pub(crate) fn resolve_input_frame_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    area_edit_session: Option<Res<AreaEditSession>>,
    mut resolved_frame: ResMut<ResolvedInputFrame>,
) {
    let modifiers = InputModifiers::from_keyboard(&keyboard);
    let pressed_chords: Vec<InputChord> = DEFAULT_BINDINGS
        .iter()
        .filter(|binding| keyboard.just_pressed(binding.chord.key))
        .map(|binding| InputChord {
            key: binding.chord.key,
            modifiers,
        })
        .collect();
    let context = InputContextSnapshot {
        text_input_blocks_keybinds: hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state),
        has_in_progress_gesture: area_edit_session.is_some_and(|session| session.is_dragging()),
    };

    resolved_frame.replace(modifiers, resolve_input_chords(&pressed_chords, context));
}
