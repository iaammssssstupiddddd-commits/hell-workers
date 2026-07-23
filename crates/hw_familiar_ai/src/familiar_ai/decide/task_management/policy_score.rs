//! Worker ranking に B1/B2 の方針寄与を一度だけ合成する共有スカラー。

use hw_logistics::transport_request::TransportPriority;

pub(crate) const WORKER_PRIORITY_WEIGHT: f32 = 0.65;
pub(crate) const WORKER_DISTANCE_WEIGHT: f32 = 0.35;
pub(crate) const POLICY_SCORE_UNIT: f32 = WORKER_PRIORITY_WEIGHT / 40.0;

pub(crate) const TRANSPORT_LOW_UNITS: i16 = -10;
pub(crate) const TRANSPORT_NORMAL_UNITS: i16 = 0;
pub(crate) const TRANSPORT_HIGH_UNITS: i16 = 10;
pub(crate) const TRANSPORT_CRITICAL_UNITS: i16 = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolicyScoreContributions {
    pub transport_units: i16,
    pub familiar_units: i16,
}

impl PolicyScoreContributions {
    #[must_use]
    pub const fn new(transport_units: i16, familiar_units: i16) -> Self {
        Self {
            transport_units,
            familiar_units,
        }
    }

    #[must_use]
    pub const fn total_units(self) -> i16 {
        self.transport_units + self.familiar_units
    }
}

#[must_use]
pub(crate) const fn transport_policy_units(priority: TransportPriority) -> i16 {
    match priority {
        TransportPriority::Low => TRANSPORT_LOW_UNITS,
        TransportPriority::Normal => TRANSPORT_NORMAL_UNITS,
        TransportPriority::High => TRANSPORT_HIGH_UNITS,
        TransportPriority::Critical => TRANSPORT_CRITICAL_UNITS,
    }
}

#[must_use]
pub(crate) fn compose_worker_score(
    base_score: f32,
    contributions: PolicyScoreContributions,
) -> f32 {
    let total_units = contributions.total_units();
    if total_units == 0 {
        return base_score;
    }
    base_score + f32::from(total_units) * POLICY_SCORE_UNIT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_mapping_is_the_named_minus_ten_to_plus_twenty_contract() {
        assert_eq!(transport_policy_units(TransportPriority::Low), -10);
        assert_eq!(transport_policy_units(TransportPriority::Normal), 0);
        assert_eq!(transport_policy_units(TransportPriority::High), 10);
        assert_eq!(transport_policy_units(TransportPriority::Critical), 20);
    }

    #[test]
    fn normal_zero_is_bit_identical_and_nonzero_scores_are_not_clamped() {
        let base = 1.0f32;
        assert_eq!(
            compose_worker_score(base, PolicyScoreContributions::default()).to_bits(),
            base.to_bits()
        );
        assert!(
            compose_worker_score(
                base,
                PolicyScoreContributions::new(TRANSPORT_CRITICAL_UNITS, 0),
            ) > 1.0
        );
    }

    #[test]
    fn combined_transport_and_synthetic_familiar_span_is_priority_weight() {
        let low = compose_worker_score(0.5, PolicyScoreContributions::new(TRANSPORT_LOW_UNITS, -5));
        let high = compose_worker_score(
            0.5,
            PolicyScoreContributions::new(TRANSPORT_CRITICAL_UNITS, 5),
        );
        assert!(((high - low) - WORKER_PRIORITY_WEIGHT).abs() < f32::EPSILON * 4.0);
    }

    #[test]
    fn adjacent_transport_tier_is_smaller_than_full_distance_span() {
        let adjacent = 10.0 * POLICY_SCORE_UNIT;
        assert!(adjacent < WORKER_DISTANCE_WEIGHT);
    }

    #[test]
    fn scalar_composition_is_independent_of_track_order() {
        let first = compose_worker_score(0.25, PolicyScoreContributions::new(20, -5));
        let second = compose_worker_score(0.25, PolicyScoreContributions::new(-5, 20));
        assert_eq!(first.to_bits(), second.to_bits());
    }
}
