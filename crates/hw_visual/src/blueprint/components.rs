//! Blueprint関連のコンポーネント定義

use bevy::prelude::*;
use std::collections::HashMap;

use crate::animations::{BounceAnimation, PulseAnimation};
use crate::floating_text::FloatingText;
use hw_core::logistics::ResourceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlueprintState {
    #[default]
    NeedsMaterials,
    Preparing,
    ReadyToBuild,
    Building,
}

#[derive(Component, Default)]
pub struct BlueprintVisual {
    pub state: BlueprintState,
    pub pulse_animation: Option<PulseAnimation>,
    pub last_delivered: HashMap<ResourceType, u32>,
}

#[derive(Component)]
pub struct MaterialIcon {
    pub _resource_type: ResourceType,
}

#[derive(Component)]
pub struct MaterialCounter {
    pub resource_type: ResourceType,
}

#[derive(Component)]
pub struct DeliveryPopup {
    pub floating_text: FloatingText,
}

#[derive(Component)]
pub struct CompletionText {
    pub floating_text: FloatingText,
}

#[derive(Component)]
pub struct BuildingBounceEffect {
    pub bounce_animation: BounceAnimation,
}

#[derive(Component)]
pub struct WorkerHammerIcon;

#[derive(Component)]
pub struct HasWorkerIndicator;

#[derive(Component)]
pub struct ProgressBar;
