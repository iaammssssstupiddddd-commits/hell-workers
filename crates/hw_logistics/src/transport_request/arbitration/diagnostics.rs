use std::collections::HashMap;

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelbarrowArbitrationOutcome {
    LeaseGranted,
    NotApplicable,
    NoAvailableWheelbarrow,
    NoSourceItems,
    SourceReserved,
    NoDestinationCapacity,
    CapacityReserved,
    DemandGone,
    PreferredBatchWaiting,
    ArbitrationContention,
    StaleInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelbarrowArbitrationHeader {
    pub generation: u64,
    pub availability_generation: u64,
    pub any_vehicle_exists: bool,
    pub available_vehicle_count: u32,
    pub leased_vehicle_count: u32,
}

#[derive(Resource, Debug, Default)]
pub struct WheelbarrowArbitrationDiagnostics {
    header: Option<WheelbarrowArbitrationHeader>,
    outcomes: HashMap<Entity, WheelbarrowArbitrationOutcome>,
}

impl WheelbarrowArbitrationDiagnostics {
    #[must_use]
    pub fn next_generation(&self) -> u64 {
        self.header
            .map_or(1, |header| header.generation.wrapping_add(1))
    }

    pub fn publish(
        &mut self,
        header: WheelbarrowArbitrationHeader,
        outcomes: HashMap<Entity, WheelbarrowArbitrationOutcome>,
    ) {
        self.header = Some(header);
        self.outcomes = outcomes;
    }

    #[must_use]
    pub const fn header(&self) -> Option<&WheelbarrowArbitrationHeader> {
        self.header.as_ref()
    }

    #[must_use]
    pub fn outcome(&self, request: Entity) -> Option<WheelbarrowArbitrationOutcome> {
        self.outcomes.get(&request).copied()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_is_latest_only() {
        let request = Entity::from_raw_u32(5).expect("valid test entity");
        let mut diagnostics = WheelbarrowArbitrationDiagnostics::default();
        let header = WheelbarrowArbitrationHeader {
            generation: 1,
            availability_generation: 0,
            any_vehicle_exists: true,
            available_vehicle_count: 1,
            leased_vehicle_count: 0,
        };
        diagnostics.publish(
            header,
            HashMap::from([(request, WheelbarrowArbitrationOutcome::LeaseGranted)]),
        );
        assert_eq!(
            diagnostics.outcome(request),
            Some(WheelbarrowArbitrationOutcome::LeaseGranted)
        );

        diagnostics.publish(
            WheelbarrowArbitrationHeader {
                generation: 2,
                ..header
            },
            HashMap::new(),
        );
        assert_eq!(diagnostics.outcome(request), None);
    }
}
