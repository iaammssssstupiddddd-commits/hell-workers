use super::config::PerfScenarioConfig;
use super::indoor_light_fixture::{IndoorLightFixturePhase, IndoorLightFixtureState};
use crate::systems::visual::building3d_cleanup::building_presentation_transform;
use bevy::prelude::*;
use hw_jobs::{Building, BuildingType};
use hw_visual::visual3d::Building3dVisual;
use std::time::Duration;

const ACCEPTANCE_ENV: &str = "HW_P02_PRESENTATION_ACTUAL_WINDOW";
const PULSE_DURATION: Duration = Duration::from_secs(7);

#[derive(Resource, Debug)]
pub(crate) struct P02ActualWindowAcceptance {
    requested: bool,
    started_at: Option<Duration>,
}

impl Default for P02ActualWindowAcceptance {
    fn default() -> Self {
        Self {
            requested: std::env::var(ACCEPTANCE_ENV).is_ok_and(|value| value == "1"),
            started_at: None,
        }
    }
}

pub(crate) fn animate_p02_actual_window_bridge_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<P02ActualWindowAcceptance>,
    owners: Query<(&Building, &Transform), Without<Building3dVisual>>,
    mut visuals: Query<(&Building3dVisual, &mut Transform), Without<Building>>,
) {
    if !acceptance.requested
        || config
            .rtt_light_selection()
            .is_none_or(|selection| selection.stage_id() != "p02" || selection.lane() != "static")
        || fixture.phase != IndoorLightFixturePhase::Ready
    {
        return;
    }

    let started_at = *acceptance.started_at.get_or_insert(time.elapsed());
    let elapsed = time.elapsed().saturating_sub(started_at);
    let scale_multiplier = bridge_pulse_scale(elapsed);
    for (visual, mut transform) in &mut visuals {
        let Ok((building, owner)) = owners.get(visual.owner) else {
            continue;
        };
        if building.kind != BuildingType::Bridge {
            continue;
        }
        let mut expected = building_presentation_transform(building.kind, owner);
        expected.scale *= scale_multiplier;
        if *transform != expected {
            *transform = expected;
        }
    }
}

fn bridge_pulse_scale(elapsed: Duration) -> f32 {
    if elapsed >= PULSE_DURATION {
        return 1.0;
    }
    1.0 + (elapsed.as_secs_f32() * 2.0).sin() * 0.18
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceptance_pulse_is_visible_then_restores_the_production_transform() {
        assert_ne!(bridge_pulse_scale(Duration::from_secs_f32(0.75)), 1.0);
        assert_eq!(bridge_pulse_scale(PULSE_DURATION), 1.0);
        assert_eq!(bridge_pulse_scale(Duration::from_secs(20)), 1.0);
    }
}
