use bevy::prelude::*;

/// RtT 解像度係数を決める品質プリセット。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum RttQualityPreset {
    Low,
    Medium,
    #[default]
    High,
}

impl RttQualityPreset {
    pub fn rtt_scale(self) -> f32 {
        match self {
            Self::High => 1.0,
            Self::Medium => 0.75,
            Self::Low => 0.5,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::High => Self::Medium,
            Self::Medium => Self::Low,
            Self::Low => Self::High,
        }
    }
}

/// 描画品質設定。
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct QualitySettings {
    pub rtt: RttQualityPreset,
}

impl QualitySettings {
    pub fn rtt_scale(self) -> f32 {
        self.rtt.rtt_scale()
    }
}
