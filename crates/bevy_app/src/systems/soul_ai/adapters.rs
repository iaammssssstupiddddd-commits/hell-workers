//! PopulationManager への書き込み adapter
//!
//! hw_ai 側の drifting システムが発行する `DriftingEscapeStarted` / `SoulEscaped` イベントを
//! 受信し、root-only リソース `PopulationManager` を更新する。

use bevy::prelude::*;
use hw_core::events::{DriftingEscapeStarted, SoulEscaped};

use crate::entities::damned_soul::spawn::PopulationManager;

/// `DriftingEscapeStarted` を受信して脱走クールダウンを開始する
pub fn on_drifting_escape_started(
    _trigger: On<DriftingEscapeStarted>,
    mut population: ResMut<PopulationManager>,
) {
    population.start_escape_cooldown();
}

/// `SoulEscaped` を受信して脱出カウンタを更新する
pub fn on_soul_escaped(trigger: On<SoulEscaped>, mut population: ResMut<PopulationManager>) {
    population.total_escaped += 1;
    info!(
        "SOUL_DRIFT: {:?} despawned at edge {:?} (total_escaped={})",
        trigger.event().entity,
        trigger.event().grid,
        population.total_escaped
    );
}
