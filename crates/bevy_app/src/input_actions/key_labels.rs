use std::fmt;

use bevy::prelude::KeyCode;

use super::bindings::DEFAULT_BINDINGS;
use super::{InputAction, InputChord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnsupportedPublicKey(pub KeyCode);

impl fmt::Display for UnsupportedPublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported public key: {:?}", self.0)
    }
}

pub(crate) fn binding_labels_for_action(
    action: InputAction,
) -> Result<Vec<String>, UnsupportedPublicKey> {
    let mut labels = Vec::new();
    for binding in DEFAULT_BINDINGS
        .iter()
        .filter(|binding| binding.action == action)
    {
        let label = format_input_chord(binding.chord)?;
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    Ok(labels)
}

pub(crate) fn format_input_chord(chord: InputChord) -> Result<String, UnsupportedPublicKey> {
    let mut parts = Vec::with_capacity(5);
    if chord.modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if chord.modifiers.alt {
        parts.push("Alt".to_string());
    }
    if chord.modifiers.shift {
        parts.push("Shift".to_string());
    }
    if chord.modifiers.super_key {
        parts.push("Super".to_string());
    }
    parts.push(key_label(chord.key)?.to_string());
    Ok(parts.join("+"))
}

fn key_label(key: KeyCode) -> Result<&'static str, UnsupportedPublicKey> {
    let label = match key {
        KeyCode::KeyA => "A",
        KeyCode::KeyB => "B",
        KeyCode::KeyC => "C",
        KeyCode::KeyD => "D",
        KeyCode::KeyE => "E",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyI => "I",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyM => "M",
        KeyCode::KeyN => "N",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyR => "R",
        KeyCode::KeyS => "S",
        KeyCode::KeyT => "T",
        KeyCode::KeyU => "U",
        KeyCode::KeyV => "V",
        KeyCode::KeyW => "W",
        KeyCode::KeyX => "X",
        KeyCode::KeyY => "Y",
        KeyCode::KeyZ => "Z",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::Escape => "Esc",
        KeyCode::Delete => "Delete",
        KeyCode::Tab => "Tab",
        KeyCode::Space => "Space",
        KeyCode::ArrowUp => "↑",
        KeyCode::ArrowDown => "↓",
        KeyCode::ArrowLeft => "←",
        KeyCode::ArrowRight => "→",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        _ => return Err(UnsupportedPublicKey(key)),
    };
    Ok(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::InputModifiers;

    #[test]
    fn public_key_labels_use_stable_modifier_and_alias_format() {
        assert_eq!(
            format_input_chord(InputChord {
                key: KeyCode::KeyZ,
                modifiers: InputModifiers {
                    ctrl: true,
                    shift: true,
                    ..Default::default()
                },
            }),
            Ok("Ctrl+Shift+Z".to_string())
        );
        assert_eq!(
            format_input_chord(InputChord::plain(KeyCode::Escape)),
            Ok("Esc".to_string())
        );
        assert_eq!(
            format_input_chord(InputChord::plain(KeyCode::Digit1)),
            Ok("1".to_string())
        );
        assert_eq!(
            format_input_chord(InputChord::plain(KeyCode::F1)),
            Ok("F1".to_string())
        );
    }

    #[test]
    fn action_lookup_preserves_binding_alias_order() {
        assert_eq!(
            binding_labels_for_action(InputAction::FamiliarChop),
            Ok(vec!["C".to_string(), "1".to_string()])
        );
        assert_eq!(
            binding_labels_for_action(InputAction::AreaRedo),
            Ok(vec!["Ctrl+Y".to_string(), "Ctrl+Shift+Z".to_string()])
        );
    }
}
