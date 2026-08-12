use super::config::PerfScenarioConfig;
use super::indoor_light_fixture::{IndoorLightFixturePhase, IndoorLightFixtureState};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use hw_core::camera::MainCamera;
use hw_jobs::Building;
use hw_visual::visual3d::LegacyStructural2dMirror;
use hw_world::WorldMap;
use std::time::Duration;

const ACCEPTANCE_ENV: &str = "HW_P02_PRESENTATION_ACTUAL_WINDOW";
const PULSE_DURATION: Duration = Duration::from_secs(7);
const FIXTURE_CENTER: (i32, i32) = (23, 27);

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

pub(crate) fn animate_p02_actual_window_foreground_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<P02ActualWindowAcceptance>,
    owners: Query<&Building>,
    mut visuals: Query<
        (&ChildOf, &mut Transform),
        (With<Sprite>, Without<LegacyStructural2dMirror>),
    >,
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
    let scale_multiplier = foreground_pulse_scale(elapsed);
    for (parent, mut transform) in &mut visuals {
        let Ok(building) = owners.get(parent.parent()) else {
            continue;
        };
        if crate::systems::jobs::presentation_class(building.kind)
            != crate::systems::jobs::RenderPresentationClass::Foreground2d
        {
            continue;
        }
        let expected = Vec3::splat(scale_multiplier);
        if transform.scale != expected {
            transform.scale = expected;
        }
    }
}

pub(crate) fn prepare_p02_actual_window_view_system(
    config: Res<PerfScenarioConfig>,
    acceptance: Res<P02ActualWindowAcceptance>,
    mut camera: Query<(&mut Transform, &mut Projection, &mut PanCamera), With<MainCamera>>,
    mut ui_roots: Query<&mut Node, Without<ChildOf>>,
) {
    if !acceptance.requested
        || config
            .rtt_light_selection()
            .is_none_or(|selection| selection.stage_id() != "p02" || selection.lane() != "static")
    {
        return;
    }

    if let Ok((mut transform, mut projection, mut pan)) = camera.single_mut() {
        let center = WorldMap::grid_to_world(FIXTURE_CENTER.0, FIXTURE_CENTER.1);
        transform.translation.x = center.x;
        transform.translation.y = center.y;
        pan.enabled = false;
        if let Projection::Orthographic(orthographic) = projection.as_mut() {
            orthographic.scale = 0.75;
        }
    }
    for mut node in &mut ui_roots {
        node.display = Display::None;
    }
}

fn foreground_pulse_scale(elapsed: Duration) -> f32 {
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
        assert_ne!(foreground_pulse_scale(Duration::from_secs_f32(0.75)), 1.0);
        assert_eq!(foreground_pulse_scale(PULSE_DURATION), 1.0);
        assert_eq!(foreground_pulse_scale(Duration::from_secs(20)), 1.0);
    }
}
