use std::cmp::Ordering;

use bevy::prelude::*;

/// How the pointer reached a selectable target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionHitKind {
    Direct,
    Snapped,
}

/// Player-facing target classes used by the common world-selection resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionTargetClass {
    Familiar,
    Soul,
    Resource,
    Object,
    TaskArea,
    Floor,
}

/// A scored world-selection candidate.
#[derive(Debug, Clone, Copy)]
pub struct SelectionCandidate {
    pub entity: Entity,
    pub class: SelectionTargetClass,
    pub hit: SelectionHitKind,
    /// Distance from the pointer to the canonical screen-space shape.
    pub distance_px: f32,
    /// Larger values are visually closer to the pointer.
    pub depth: f32,
}

/// Latest common resolver output used by hover feedback and compatibility mirrors.
#[derive(Resource, Debug, Default)]
pub struct WorldPointerTarget {
    pub primary: Option<SelectionCandidate>,
    pub candidates: Vec<SelectionCandidate>,
    pub screen_pos: Option<Vec2>,
    pub world_pos: Option<Vec2>,
}

/// Transient presentation signal emitted after a Familiar move command is accepted.
#[derive(Resource, Debug, Default)]
pub struct FamiliarMoveFeedback {
    pub destination: Vec2,
    pub revision: u64,
}

impl FamiliarMoveFeedback {
    pub fn accepted(&mut self, destination: Vec2) {
        self.destination = destination;
        self.revision = self.revision.wrapping_add(1);
    }
}

impl SelectionCandidate {
    fn rank(self) -> u8 {
        use SelectionHitKind::{Direct, Snapped};
        use SelectionTargetClass::{Familiar, Floor, Object, Resource, Soul, TaskArea};

        match (self.hit, self.class) {
            (Direct, Familiar) => 1,
            (Direct, Soul) => 2,
            (Direct, Resource) => 3,
            (Direct, Object) => 4,
            (Direct, TaskArea) => 5,
            (Snapped, Familiar) => 6,
            (Snapped, Soul) => 7,
            (Snapped, Resource) => 8,
            (Snapped, Object) | (Snapped, TaskArea) => 9,
            (Direct, Floor) => 10,
            (Snapped, Floor) => 11,
        }
    }
}

/// Sorts candidates deterministically and removes duplicate target entities.
pub fn sort_selection_candidates(candidates: &mut Vec<SelectionCandidate>) {
    candidates.sort_by(|left, right| {
        left.rank()
            .cmp(&right.rank())
            .then_with(|| left.distance_px.total_cmp(&right.distance_px))
            .then_with(|| right.depth.total_cmp(&left.depth))
            .then_with(|| stable_entity_key(left.entity).cmp(&stable_entity_key(right.entity)))
    });
    candidates.dedup_by_key(|candidate| stable_entity_key(candidate.entity));
}

fn stable_entity_key(entity: Entity) -> (u32, u32) {
    (entity.index_u32(), entity.generation().to_bits())
}

/// Returns the distance from a point to an axis-aligned rectangle in pixels.
pub fn point_to_rect_distance(point: Vec2, min: Vec2, max: Vec2) -> f32 {
    let delta = (min - point).max(Vec2::ZERO) + (point - max).max(Vec2::ZERO);
    delta.length()
}

/// Classifies a screen-space distance using a target-specific snap margin.
pub fn classify_selection_distance(
    distance_px: f32,
    snap_margin_px: f32,
) -> Option<SelectionHitKind> {
    match distance_px.partial_cmp(&0.0) {
        Some(Ordering::Less | Ordering::Equal) => Some(SelectionHitKind::Direct),
        Some(Ordering::Greater) if distance_px <= snap_margin_px => Some(SelectionHitKind::Snapped),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_small_targets_win_and_order_is_stable() {
        let familiar = Entity::from_raw_u32(4).unwrap();
        let building = Entity::from_raw_u32(2).unwrap();
        let resource = Entity::from_raw_u32(8).unwrap();
        let mut candidates = vec![
            SelectionCandidate {
                entity: building,
                class: SelectionTargetClass::Object,
                hit: SelectionHitKind::Direct,
                distance_px: 0.0,
                depth: 0.0,
            },
            SelectionCandidate {
                entity: familiar,
                class: SelectionTargetClass::Familiar,
                hit: SelectionHitKind::Snapped,
                distance_px: 2.0,
                depth: 0.0,
            },
            SelectionCandidate {
                entity: resource,
                class: SelectionTargetClass::Resource,
                hit: SelectionHitKind::Direct,
                distance_px: 0.0,
                depth: 0.0,
            },
        ];

        sort_selection_candidates(&mut candidates);

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.entity)
                .collect::<Vec<_>>(),
            vec![resource, building, familiar]
        );
    }

    #[test]
    fn rectangle_distance_distinguishes_inside_edge_and_snap_margin() {
        let min = Vec2::new(10.0, 20.0);
        let max = Vec2::new(30.0, 40.0);
        assert_eq!(point_to_rect_distance(Vec2::new(10.0, 40.0), min, max), 0.0);
        assert_eq!(point_to_rect_distance(Vec2::new(34.0, 40.0), min, max), 4.0);
        assert_eq!(
            classify_selection_distance(4.0, 8.0),
            Some(SelectionHitKind::Snapped)
        );
        assert_eq!(classify_selection_distance(8.1, 8.0), None);
    }
}
