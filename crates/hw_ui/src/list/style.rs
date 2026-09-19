use super::models::{SoulGender, StressBucket, TaskVisual};
use crate::{setup::UiAssets, theme::UiTheme};
use bevy::prelude::*;

pub(super) fn get_gender_icon_and_color(
    gender: SoulGender,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match gender {
        SoulGender::Male => (assets.icon_male().clone(), theme.colors.male),
        SoulGender::Female => (assets.icon_female().clone(), theme.colors.female),
    }
}

pub(super) fn get_task_icon_and_color(
    task: TaskVisual,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match task {
        TaskVisual::Idle => (assets.icon_idle().clone(), theme.colors.idle),
        TaskVisual::Chop => (assets.icon_axe().clone(), theme.colors.chop),
        TaskVisual::Mine => (assets.icon_pick().clone(), theme.colors.mine),
        TaskVisual::GatherDefault => (assets.icon_pick().clone(), theme.colors.gather_default),
        TaskVisual::Haul => (assets.icon_haul().clone(), theme.colors.haul),
        TaskVisual::Build => (assets.icon_pick().clone(), theme.colors.build),
        TaskVisual::HaulToBlueprint => (assets.icon_haul().clone(), theme.colors.haul_to_bp),
        TaskVisual::Water => (assets.icon_haul().clone(), theme.colors.water),
        TaskVisual::GeneratePower => (assets.icon_fatigue().clone(), theme.colors.text_accent),
        TaskVisual::Deconstruct => (assets.icon_hammer().clone(), theme.colors.stress_high),
        TaskVisual::Move => (assets.icon_haul().clone(), theme.colors.build),
        TaskVisual::Refine => (assets.icon_hammer().clone(), theme.colors.build),
        TaskVisual::CollectBone => (
            assets.icon_bone_small().clone(),
            theme.colors.gather_default,
        ),
    }
}

pub(super) fn get_stress_color(bucket: StressBucket, theme: &UiTheme) -> Color {
    match bucket {
        StressBucket::Low => Color::WHITE,
        StressBucket::Medium => theme.colors.stress_medium,
        StressBucket::High => theme.colors.stress_high,
    }
}

pub(super) fn get_dream_color(dream_empty: bool, theme: &UiTheme) -> Color {
    if dream_empty {
        theme.colors.stress_medium
    } else {
        theme.colors.fatigue_text
    }
}

pub(super) fn stress_weight(bucket: StressBucket) -> FontWeight {
    match bucket {
        StressBucket::High => FontWeight::BOLD,
        _ => FontWeight::default(),
    }
}
