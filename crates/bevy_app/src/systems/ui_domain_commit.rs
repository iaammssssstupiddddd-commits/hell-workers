//! Simulation-owned commit boundary for typed UI requests.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_energy::{
    PowerConsumer, PowerConsumerPolicy, PowerConsumerPolicyChangeOutcome,
    PowerConsumerPolicyChangeStatus, PowerPriority, SoulSpaConstructionCancelOutcome,
    SoulSpaConstructionCancelRequest, SoulSpaConstructionCancelResult, SoulSpaPhase, SoulSpaSite,
    SoulSpaSlotsChangeOutcome, SoulSpaSlotsChangeStatus,
};
use hw_logistics::{StockpilePolicyChangeRequest, StockpilePolicyPatch};
use hw_spatial::StockpileSpatialGrid;
use hw_ui::UiIntent;
use hw_ui::intents::StockpilePolicyEditTarget;
use hw_ui::power::PowerPriorityValue;
use hw_world::DoorLockToggleRequest;

use crate::systems::save::{SaveRecoveryMode, SaveRecoveryMode::RecoveryFailed};

/// Live simulation mutation phase for typed UI requests.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct UiDomainCommitSet;

#[derive(SystemParam)]
pub(crate) struct UiDomainCommitCtx<'w, 's> {
    door_lock_requests: MessageWriter<'w, DoorLockToggleRequest>,
    stockpile_grid: Res<'w, StockpileSpatialGrid>,
    stockpile_policy_requests: MessageWriter<'w, StockpilePolicyChangeRequest>,
    soul_spa_slot_outcomes: MessageWriter<'w, SoulSpaSlotsChangeOutcome>,
    soul_spa_cancel_requests: MessageWriter<'w, SoulSpaConstructionCancelRequest>,
    soul_spa_cancel_outcomes: MessageWriter<'w, SoulSpaConstructionCancelOutcome>,
    power_consumer_policy_outcomes: MessageWriter<'w, PowerConsumerPolicyChangeOutcome>,
    q_soul_spas: Query<'w, 's, &'static mut SoulSpaSite>,
    q_power_consumers: Query<'w, 's, Option<&'static mut PowerConsumerPolicy>, With<PowerConsumer>>,
    q_entities: Query<'w, 's, ()>,
}

/// Applies simulation-facing UI intents before generic UI/save handling in the same frame.
pub(crate) fn apply_ui_domain_intents_system(
    mut ui_intents: MessageReader<UiIntent>,
    mut domain: UiDomainCommitCtx,
    time: Res<Time<Virtual>>,
    recovery: Res<SaveRecoveryMode>,
) {
    for intent in ui_intents.read().cloned() {
        if *recovery == RecoveryFailed {
            continue;
        }
        match intent {
            UiIntent::ToggleDoorLock(entity) => {
                domain
                    .door_lock_requests
                    .write(DoorLockToggleRequest { owner: entity });
            }
            UiIntent::ApplyStockpilePolicy { target, patch } => {
                apply_stockpile_policy(&mut domain, target, patch);
            }
            UiIntent::SetSoulSpaActiveSlots {
                target,
                active_slots,
            } => apply_soul_spa_slots(&mut domain, target, active_slots),
            UiIntent::CancelSoulSpaConstruction { target } => {
                apply_soul_spa_cancel(&mut domain, target, time.is_paused());
            }
            UiIntent::SetPowerConsumerPriority { target, priority } => {
                apply_power_priority(&mut domain, target, priority);
            }
            _ => {}
        }
    }
}

fn apply_stockpile_policy(
    domain: &mut UiDomainCommitCtx<'_, '_>,
    target: StockpilePolicyEditTarget,
    patch: StockpilePolicyPatch,
) {
    let targets =
        crate::systems::command::resolve_stockpile_policy_targets(target, &domain.stockpile_grid);
    domain
        .stockpile_policy_requests
        .write(StockpilePolicyChangeRequest { targets, patch });
}

fn apply_soul_spa_slots(domain: &mut UiDomainCommitCtx<'_, '_>, target: Entity, requested: u32) {
    let status = match domain.q_soul_spas.get_mut(target) {
        Ok(site) if site.phase != SoulSpaPhase::Operational => {
            SoulSpaSlotsChangeStatus::PhaseUnavailable
        }
        Ok(mut site) => {
            let applied = SoulSpaSite::clamped_active_slots(requested);
            if site.active_slots != applied {
                site.set_active_slots(applied);
            }
            SoulSpaSlotsChangeStatus::Applied {
                requested,
                applied,
                clamped: requested != applied,
            }
        }
        Err(_) if domain.q_entities.get(target).is_ok() => {
            SoulSpaSlotsChangeStatus::UnsupportedTarget
        }
        Err(_) => SoulSpaSlotsChangeStatus::StaleTarget,
    };
    domain
        .soul_spa_slot_outcomes
        .write(SoulSpaSlotsChangeOutcome { target, status });
}

fn apply_soul_spa_cancel(domain: &mut UiDomainCommitCtx<'_, '_>, target: Entity, paused: bool) {
    if paused {
        domain
            .soul_spa_cancel_outcomes
            .write(SoulSpaConstructionCancelOutcome {
                target,
                result: SoulSpaConstructionCancelResult::Paused,
            });
    } else {
        domain
            .soul_spa_cancel_requests
            .write(SoulSpaConstructionCancelRequest { target });
    }
}

fn apply_power_priority(
    domain: &mut UiDomainCommitCtx<'_, '_>,
    target: Entity,
    requested: PowerPriorityValue,
) {
    let requested = match requested {
        PowerPriorityValue::Low => PowerPriority::Low,
        PowerPriorityValue::Normal => PowerPriority::Normal,
        PowerPriorityValue::High => PowerPriority::High,
    };
    let status = match domain.q_power_consumers.get_mut(target) {
        Ok(Some(mut policy)) => {
            let previous = policy.priority;
            if previous != requested {
                policy.priority = requested;
            }
            PowerConsumerPolicyChangeStatus::Applied {
                previous,
                applied: requested,
            }
        }
        Ok(None) => PowerConsumerPolicyChangeStatus::MissingPolicy,
        Err(_) if domain.q_entities.get(target).is_ok() => {
            PowerConsumerPolicyChangeStatus::UnsupportedTarget
        }
        Err(_) => PowerConsumerPolicyChangeStatus::StaleTarget,
    };
    domain
        .power_consumer_policy_outcomes
        .write(PowerConsumerPolicyChangeOutcome { target, status });
}
