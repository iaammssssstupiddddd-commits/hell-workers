//! Blueprint関連のコンポーネント定義

use bevy::prelude::*;
use hw_core::visual_mirror::construction::BlueprintVisualState;
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
    /// Building 中だけ存在する、親 Sprite を書き換えずに pulse を描く child。
    pub pulse_overlay: Option<BlueprintPulseOverlay>,
    pub last_delivered: HashMap<ResourceType, u32>,
}

impl BlueprintVisual {
    /// Initializes delivery history from a rehydrated mirror so the first
    /// visual frame does not replay saved deliveries as new `+1` popups.
    pub fn from_visual_state(visual_state: &BlueprintVisualState) -> Self {
        Self {
            last_delivered: visual_state
                .material_counts
                .iter()
                .map(|(resource_type, delivered, _)| (*resource_type, *delivered))
                .collect(),
            ..default()
        }
    }
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

/// Blueprint が所有する進捗バーの visual-only link。
///
/// `ChildOf` だけを全件走査して親を探索しないために、Blueprint 側から背景と fill を
/// O(1) で参照する。`BlueprintVisualState` が除去されたときに一緒に破棄され、load 後は
/// visual state の再生成に伴って作り直される。
#[derive(Component, Clone, Copy)]
pub struct BlueprintProgressBars {
    pub background: Entity,
    pub fill: Entity,
}

/// `BlueprintVisual` が O(1) で更新する Building pulse child。
///
/// `ChildOf` の逆引きを毎 frame 行わず、root の static Sprite を animation の
/// ために changed にしない。`base_color` は visual state の差分同期で更新され、
/// child 側だけが alpha を変える。
#[derive(Clone, Copy)]
pub struct BlueprintPulseOverlay {
    pub entity: Entity,
    pub base_color: Color,
}

/// Building pulse 専用の visual child marker。
#[derive(Component)]
pub struct BlueprintPulseOverlayChild;
