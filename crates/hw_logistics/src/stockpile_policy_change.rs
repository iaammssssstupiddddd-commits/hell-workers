use std::collections::HashSet;

use bevy::prelude::*;

use crate::{Stockpile, StockpilePolicy, StockpilePolicyPatch};

/// Canonical domain request shared by single-cell and rectangular policy edits.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct StockpilePolicyChangeRequest {
    pub targets: Vec<Entity>,
    pub patch: StockpilePolicyPatch,
}

/// Terminal result for one [`StockpilePolicyChangeRequest`].
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockpilePolicyChangeOutcome {
    /// Number of targets supplied by the adapter before defensive deduplication.
    pub requested: usize,
    /// Number of distinct targets evaluated by the domain handler.
    pub unique: usize,
    /// Number of managed cells whose durable policy changed.
    pub applied: usize,
    /// Number of managed cells already equal to the requested result.
    pub unchanged: usize,
    /// Number of targets that no longer exist.
    pub skipped_stale: usize,
    /// Number of live targets outside the `Stockpile + StockpilePolicy` boundary.
    pub skipped_unmanaged: usize,
    /// Number of managed cells whose requested target was clamped to physical capacity.
    pub target_clamped: usize,
}

impl StockpilePolicyChangeOutcome {
    #[must_use]
    pub const fn eligible(self) -> usize {
        self.applied + self.unchanged
    }

    #[must_use]
    pub const fn has_adjustments_or_skips(self) -> bool {
        self.skipped_stale > 0 || self.skipped_unmanaged > 0 || self.target_clamped > 0
    }
}

fn canonical_targets(targets: &[Entity]) -> Vec<Entity> {
    let mut seen = HashSet::with_capacity(targets.len());
    let mut unique: Vec<Entity> = targets
        .iter()
        .copied()
        .filter(|entity| seen.insert(*entity))
        .collect();
    unique.sort_unstable_by_key(|entity| (entity.index_u32(), entity.generation().to_bits()));
    unique
}

/// Applies policy changes without touching inventory or in-flight delivery relationships.
pub fn apply_stockpile_policy_change_requests_system(
    mut requests: MessageReader<StockpilePolicyChangeRequest>,
    q_existing: Query<()>,
    mut q_managed: Query<(&Stockpile, &mut StockpilePolicy)>,
    mut outcomes: MessageWriter<StockpilePolicyChangeOutcome>,
) {
    for request in requests.read() {
        let targets = canonical_targets(&request.targets);
        let mut outcome = StockpilePolicyChangeOutcome {
            requested: request.targets.len(),
            unique: targets.len(),
            applied: 0,
            unchanged: 0,
            skipped_stale: 0,
            skipped_unmanaged: 0,
            target_clamped: 0,
        };

        for entity in targets {
            if !q_existing.contains(entity) {
                outcome.skipped_stale += 1;
                continue;
            }

            let Ok((stockpile, mut current)) = q_managed.get_mut(entity) else {
                outcome.skipped_unmanaged += 1;
                continue;
            };
            let result = request.patch.apply(*current, stockpile.capacity);
            if result.target_clamped {
                outcome.target_clamped += 1;
            }
            if result.policy == *current {
                outcome.unchanged += 1;
            } else {
                *current = result.policy;
                outcome.applied += 1;
            }
        }

        outcomes.write(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StockpileAcceptance;
    use crate::transport_request::TransportPriority;

    #[derive(Resource, Default)]
    struct Receipts(Vec<StockpilePolicyChangeOutcome>);

    fn collect_outcomes(
        mut outcomes: MessageReader<StockpilePolicyChangeOutcome>,
        mut receipts: ResMut<Receipts>,
    ) {
        receipts.0.extend(outcomes.read().copied());
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<StockpilePolicyChangeRequest>()
            .add_message::<StockpilePolicyChangeOutcome>()
            .init_resource::<Receipts>()
            .add_systems(
                Update,
                (
                    apply_stockpile_policy_change_requests_system,
                    collect_outcomes,
                )
                    .chain(),
            );
        app
    }

    fn managed_cell(app: &mut App, capacity: usize) -> Entity {
        app.world_mut()
            .spawn((
                Stockpile {
                    capacity,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(capacity),
            ))
            .id()
    }

    #[test]
    fn mixed_targets_are_deduplicated_validated_and_clamped_per_cell() {
        let mut app = app();
        let managed_a = managed_cell(&mut app, 3);
        let managed_b = managed_cell(&mut app, 7);
        let special = app
            .world_mut()
            .spawn(Stockpile {
                capacity: 50,
                resource_type: None,
            })
            .id();
        let stale = app.world_mut().spawn_empty().id();
        assert!(app.world_mut().despawn(stale));

        app.world_mut().write_message(StockpilePolicyChangeRequest {
            targets: vec![managed_b, special, stale, managed_a, managed_a],
            patch: StockpilePolicyPatch {
                inbound_priority: Some(TransportPriority::High),
                target_amount: Some(99),
                ..default()
            },
        });
        app.update();

        assert_eq!(
            app.world().resource::<Receipts>().0,
            vec![StockpilePolicyChangeOutcome {
                requested: 5,
                unique: 4,
                applied: 2,
                unchanged: 0,
                skipped_stale: 1,
                skipped_unmanaged: 1,
                target_clamped: 2,
            }]
        );
        for (entity, capacity) in [(managed_a, 3), (managed_b, 7)] {
            assert_eq!(
                *app.world().get::<StockpilePolicy>(entity).unwrap(),
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Any,
                    inbound_priority: TransportPriority::High,
                    target_amount: capacity,
                    allow_export: true,
                }
            );
        }
    }

    #[test]
    fn no_op_patch_reports_unchanged_without_triggering_component_change() {
        let mut app = app();
        let managed = managed_cell(&mut app, 10);
        app.update();
        app.world_mut().resource_mut::<Receipts>().0.clear();

        app.world_mut().write_message(StockpilePolicyChangeRequest {
            targets: vec![managed],
            patch: StockpilePolicyPatch::default(),
        });
        app.update();

        assert_eq!(
            app.world().resource::<Receipts>().0,
            vec![StockpilePolicyChangeOutcome {
                requested: 1,
                unique: 1,
                applied: 0,
                unchanged: 1,
                skipped_stale: 0,
                skipped_unmanaged: 0,
                target_clamped: 0,
            }]
        );
        assert!(
            !app.world()
                .entity(managed)
                .get_ref::<StockpilePolicy>()
                .unwrap()
                .is_changed()
        );
    }
}
