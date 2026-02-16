//! 使い魔の「口癖」傾向

use bevy::prelude::*;
use rand::Rng;

use crate::systems::visual::speech::phrases::LatinPhrase;

/// 使い魔の「口癖」傾向
#[derive(Component)]
pub struct FamiliarVoice {
    pub preferences: [usize; LatinPhrase::COUNT],
    pub preference_weight: f32,
}

impl FamiliarVoice {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let mut preferences = [0; LatinPhrase::COUNT];
        for p in preferences.iter_mut() {
            *p = rng.gen_range(0..5);
        }
        Self {
            preferences,
            preference_weight: rng.gen_range(0.6..0.9),
        }
    }

    pub fn get_preference(&self, phrase: LatinPhrase) -> usize {
        let idx = phrase.index();
        if idx < self.preferences.len() {
            self.preferences[idx]
        } else {
            0
        }
    }
}
