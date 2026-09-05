use std::collections::HashMap;

use bevy::prelude::Entity;
use hw_core::constants::{MUD_MIXER_CAPACITY, MUD_MIXER_MUD_CAPACITY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolatileMixerStorage {
    pub sand: u32,
    pub rock: u32,
    pub mud: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolatileRecoveryCandidate {
    pub receiver: Entity,
    pub storage: VolatileMixerStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolatileMixerIncrement {
    pub receiver: Entity,
    pub expected: VolatileMixerStorage,
    pub sand: u32,
    pub mud: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolatileMudTransfer {
    pub entity: Entity,
    pub receiver: Entity,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VolatileRecoveryPlan {
    pub increments: Vec<VolatileMixerIncrement>,
    pub mud_transfers: Vec<VolatileMudTransfer>,
}

/// Allocates all sand first, then all materialized mud, across candidates in
/// caller-provided priority order. Returns `None` without exposing a partial
/// plan when either resource cannot be placed completely.
pub fn allocate_volatile_recovery(
    ordered_candidates: &[VolatileRecoveryCandidate],
    mut sand_remaining: u32,
    ordered_mud_entities: &[Entity],
) -> Option<VolatileRecoveryPlan> {
    let mut projected = ordered_candidates.to_vec();
    let mut increments = HashMap::<Entity, (VolatileMixerStorage, u32, u32)>::new();
    for candidate in &mut projected {
        let capacity = MUD_MIXER_CAPACITY.saturating_sub(candidate.storage.sand);
        let amount = sand_remaining.min(capacity);
        if amount > 0 {
            increments.insert(candidate.receiver, (candidate.storage, amount, 0));
            candidate.storage.sand += amount;
            sand_remaining -= amount;
        }
        if sand_remaining == 0 {
            break;
        }
    }
    if sand_remaining > 0 {
        return None;
    }

    let mut mud_transfers = Vec::with_capacity(ordered_mud_entities.len());
    let mut mud_index = 0;
    for candidate in &mut projected {
        let capacity = MUD_MIXER_MUD_CAPACITY.saturating_sub(candidate.storage.mud);
        let remaining = (ordered_mud_entities.len() - mud_index) as u32;
        let amount = remaining.min(capacity);
        if amount > 0 {
            let entry = increments
                .entry(candidate.receiver)
                .or_insert((candidate.storage, 0, 0));
            entry.2 += amount;
            candidate.storage.mud += amount;
            for &entity in &ordered_mud_entities[mud_index..mud_index + amount as usize] {
                mud_transfers.push(VolatileMudTransfer {
                    entity,
                    receiver: candidate.receiver,
                });
            }
            mud_index += amount as usize;
        }
        if mud_index == ordered_mud_entities.len() {
            break;
        }
    }
    if mud_index != ordered_mud_entities.len() {
        return None;
    }

    let mut increments = increments
        .into_iter()
        .map(|(receiver, (expected, sand, mud))| VolatileMixerIncrement {
            receiver,
            expected,
            sand,
            mud,
        })
        .collect::<Vec<_>>();
    increments.sort_unstable_by_key(|increment| increment.receiver.to_bits());
    Some(VolatileRecoveryPlan {
        increments,
        mud_transfers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity index is valid")
    }

    #[test]
    fn sand_then_mud_uses_candidate_order_and_preserves_expected_storage() {
        let candidates = [
            VolatileRecoveryCandidate {
                receiver: entity(9),
                storage: VolatileMixerStorage {
                    sand: MUD_MIXER_CAPACITY - 1,
                    rock: 7,
                    mud: MUD_MIXER_MUD_CAPACITY - 1,
                },
            },
            VolatileRecoveryCandidate {
                receiver: entity(2),
                storage: VolatileMixerStorage {
                    sand: 0,
                    rock: 11,
                    mud: 0,
                },
            },
        ];
        let mud = [entity(20), entity(10)];

        let plan = allocate_volatile_recovery(&candidates, 2, &mud).expect("capacity exists");

        let receiver_2 = plan
            .increments
            .iter()
            .find(|increment| increment.receiver == entity(2))
            .expect("second candidate receives recovered inventory");
        assert_eq!(receiver_2.expected.rock, 11);
        assert_eq!(receiver_2.sand, 1);
        assert_eq!(receiver_2.mud, 1);
        let receiver_9 = plan
            .increments
            .iter()
            .find(|increment| increment.receiver == entity(9))
            .expect("first candidate receives recovered inventory");
        assert_eq!(receiver_9.expected.rock, 7);
        assert_eq!(receiver_9.sand, 1);
        assert_eq!(receiver_9.mud, 1);
        assert_eq!(
            plan.mud_transfers,
            vec![
                VolatileMudTransfer {
                    entity: entity(20),
                    receiver: entity(9),
                },
                VolatileMudTransfer {
                    entity: entity(10),
                    receiver: entity(2),
                },
            ]
        );
        assert_eq!(candidates[0].storage.sand, MUD_MIXER_CAPACITY - 1);
    }

    #[test]
    fn insufficient_capacity_returns_no_partial_plan() {
        let full = [VolatileRecoveryCandidate {
            receiver: entity(1),
            storage: VolatileMixerStorage {
                sand: MUD_MIXER_CAPACITY,
                rock: 0,
                mud: MUD_MIXER_MUD_CAPACITY,
            },
        }];

        assert_eq!(allocate_volatile_recovery(&full, 1, &[]), None);
        assert_eq!(allocate_volatile_recovery(&full, 0, &[entity(2)]), None);
    }
}
