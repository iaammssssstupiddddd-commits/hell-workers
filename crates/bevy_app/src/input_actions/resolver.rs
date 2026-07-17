use bevy::prelude::*;
use hw_ui::area_edit::AreaEditSession;

use super::bindings::{
    DEFAULT_BINDINGS, InputBinding, actions_are_compatible, binding_matches_context,
};
use super::context::InputContextParams;
use super::{InputAction, InputChord, InputContextSnapshot, InputModifiers};

/// Actions and modifier state resolved once for the current frame.
#[derive(Resource, Debug, Default)]
pub struct ResolvedInputFrame {
    actions: Vec<InputAction>,
    pub modifiers: InputModifiers,
    selected_familiar: Option<Entity>,
    pointer_selection_suppressed: bool,
}

impl ResolvedInputFrame {
    pub fn actions(&self) -> &[InputAction] {
        &self.actions
    }

    pub fn contains(&self, action: InputAction) -> bool {
        self.actions.contains(&action)
    }

    pub fn selected_familiar(&self) -> Option<Entity> {
        self.selected_familiar
    }

    pub fn pointer_selection_suppressed(&self) -> bool {
        self.pointer_selection_suppressed
    }

    pub(crate) fn replace(
        &mut self,
        modifiers: InputModifiers,
        actions: Vec<InputAction>,
        selected_familiar: Option<Entity>,
        pointer_selection_suppressed: bool,
    ) {
        self.modifiers = modifiers;
        self.actions = actions;
        self.selected_familiar = selected_familiar;
        self.pointer_selection_suppressed = pointer_selection_suppressed;
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
    if context.top_overlay.is_none() && context.text_input_blocks_keybinds {
        return Vec::new();
    }

    let candidates = bindings
        .iter()
        .filter(|binding| pressed_chords.contains(&binding.chord))
        .filter(|binding| binding_matches_context(binding, &context))
        .filter(|binding| {
            !(context.has_in_progress_gesture && binding.action == InputAction::SaveGame)
        });

    let mut chord_winners: Vec<&InputBinding> = Vec::new();
    for binding in candidates {
        if let Some(existing) = chord_winners
            .iter_mut()
            .find(|existing| existing.chord == binding.chord)
        {
            if binding_priority(binding) > binding_priority(existing) {
                *existing = binding;
            }
        } else {
            chord_winners.push(binding);
        }
    }

    let mut family_winners: Vec<&InputBinding> = Vec::new();
    for binding in chord_winners {
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
            if (binding.family_priority, binding.resolution_priority)
                > (existing.family_priority, existing.resolution_priority)
            {
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
            .all(|existing| actions_are_compatible(existing, binding, &context))
        {
            resolved.push(binding);
        }
    }
    resolved.iter().map(|binding| binding.action).collect()
}

fn binding_priority(binding: &InputBinding) -> (u8, u8, u8) {
    (
        binding.context_priority,
        binding.family_priority,
        binding.resolution_priority,
    )
}

pub(crate) fn resolve_input_frame_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    context_params: InputContextParams,
    area_edit_session: Option<Res<AreaEditSession>>,
    mut resolved_frame: ResMut<ResolvedInputFrame>,
) {
    let modifiers = InputModifiers::from_keyboard(&keyboard);
    let mut pressed_chords: Vec<InputChord> = Vec::new();
    for binding in DEFAULT_BINDINGS {
        let chord = InputChord {
            key: binding.chord.key,
            modifiers,
        };
        if keyboard.just_pressed(chord.key) && !pressed_chords.contains(&chord) {
            pressed_chords.push(chord);
        }
    }
    let has_in_progress_gesture = area_edit_session.is_some_and(|session| session.is_dragging());
    let (context, selected_familiar) = context_params.snapshot(has_in_progress_gesture);
    let actions = resolve_input_chords(&pressed_chords, context);
    let pointer_selection_suppressed = DEFAULT_BINDINGS
        .iter()
        .any(|binding| binding.suppresses_pointer_selection && actions.contains(&binding.action));

    resolved_frame.replace(
        modifiers,
        actions,
        selected_familiar,
        pointer_selection_suppressed,
    );
}
