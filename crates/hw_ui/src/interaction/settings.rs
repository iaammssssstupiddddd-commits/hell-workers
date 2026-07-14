use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{Slider, SliderRange, SliderValue};

use crate::components::{
    MenuState, SettingsCheckboxMarker, SettingsCheckmarkMarker, SettingsPanel,
    SettingsSliderMarker, SettingsSliderThumbMarker,
};

pub fn is_settings_panel_open(q_settings_panel: &Query<&Node, With<SettingsPanel>>) -> bool {
    q_settings_panel
        .single()
        .is_ok_and(|node| node.display != Display::None)
}

pub fn update_settings_panel_visibility(
    menu_state: Res<MenuState>,
    mut q_settings_panel: Query<&mut Node, With<SettingsPanel>>,
) {
    let display = if matches!(*menu_state, MenuState::Settings) {
        Display::Flex
    } else {
        Display::None
    };

    if let Ok(mut node) = q_settings_panel.single_mut() {
        node.display = display;
    }
}

pub fn sync_settings_checkmarks_system(
    q_checkboxes: Query<(&SettingsCheckboxMarker, Has<Checked>)>,
    mut q_checkmarks: Query<(&SettingsCheckmarkMarker, &mut Node)>,
) {
    for (checkbox, checked) in q_checkboxes.iter() {
        for (checkmark, mut node) in q_checkmarks.iter_mut() {
            if checkmark.0 == checkbox.0 {
                node.display = if checked {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}

pub fn sync_settings_slider_thumbs_system(
    q_sliders: Query<(&SettingsSliderMarker, &SliderValue, &SliderRange, &Children), With<Slider>>,
    mut q_thumbs: Query<(&SettingsSliderThumbMarker, &mut Node)>,
) {
    for (marker, value, range, children) in q_sliders.iter() {
        let clamped = range.clamp(value.0);
        let pct = if range.span() > f32::EPSILON {
            (clamped - range.start()) / range.span() * 100.0
        } else {
            0.0
        };

        for child in children.iter() {
            if let Ok((thumb_marker, mut node)) = q_thumbs.get_mut(child)
                && thumb_marker.0 == marker.0
            {
                node.left = Val::Percent(pct);
            }
        }
    }
}
