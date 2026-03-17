use bevy::prelude::*;
use hw_core::system_sets::SoulAiSystemSet;

use crate::apply_reservation_requests_system;

pub struct LogisticsPlugin;

impl Plugin for LogisticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            apply_reservation_requests_system.in_set(SoulAiSystemSet::Execute),
        );
    }
}
