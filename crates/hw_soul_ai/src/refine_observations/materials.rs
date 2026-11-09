//! Actual material-to-counter conversions, not inferred delivery or acceptance.
use super::{CAP, RefineCommitReceipts};
use bevy::prelude::*;
use hw_core::{WorldEpoch, constants::MUD_MIXER_CAPACITY};
use hw_jobs::{Blueprint, Building, BuildingType, WorkType, mud_mixer::MudMixerStorage};
use hw_logistics::{ResourceType, Wheelbarrow};

#[derive(Clone, Debug)]
pub struct MixerMaterialReceipt {
    pub worker: Entity,
    pub assignment: Entity,
    pub target: Entity,
    pub work_type: WorkType,
    pub owner: Entity,
    pub item: Entity,
    pub resource: ResourceType,
    pub actual_resource: Option<ResourceType>,
    pub stored_before: Option<Entity>,
    pub loaded_before: Option<Entity>,
    pub carried_before: Option<Entity>,
    pub wheelbarrow: Option<Entity>,
    pub world_epoch: u64,
    /// Branch-local Sand/Rock/Mud counters, not the later snapshot totals.
    pub before: [u32; 3],
    pub after: [u32; 3],
}

impl MixerMaterialReceipt {
    pub(crate) fn capture(
        ctx: &crate::soul_ai::execute::task_execution::context::TaskExecutionContext,
        owner: Entity,
        item: Entity,
        resource: ResourceType,
        wheelbarrow: Option<Entity>,
    ) -> Self {
        let observed = ctx.queries.resource_items.get(item).ok();
        let identity = ctx.task_identity();
        Self {
            worker: ctx.soul_entity,
            assignment: identity.assignment_entity,
            target: identity.current_target_entity,
            work_type: identity.current_work_type,
            owner,
            item,
            resource,
            actual_resource: observed.as_ref().map(|row| row.3.0),
            stored_before: observed.as_ref().and_then(|row| row.4.map(|r| r.0)),
            loaded_before: observed.as_ref().and_then(|row| row.5.map(|r| r.0)),
            carried_before: ctx.inventory.0,
            wheelbarrow,
            world_epoch: ctx.world_epoch,
            before: [0; 3],
            after: [0; 3],
        }
    }

    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<RefineCommitReceipts>() {
                return;
            }
            let axis = match self.resource {
                ResourceType::Sand => Some(0),
                ResourceType::Rock => Some(1),
                _ => None,
            };
            let valid = world.get_resource::<WorldEpoch>().map(|e| e.get())
                == Some(self.world_epoch)
                && self.worker != self.owner
                && self.item != self.worker
                && self.item != self.owner
                && world
                    .get::<hw_core::soul::DamnedSoul>(self.worker)
                    .is_some()
                && world.get_entity(self.item).is_err()
                && world.get::<Blueprint>(self.owner).is_none()
                && world
                    .get::<Building>(self.owner)
                    .is_some_and(|b| b.kind == BuildingType::MudMixer && !b.is_provisional)
                && world.get::<MudMixerStorage>(self.owner).is_some()
                && self.actual_resource == Some(self.resource)
                && self.stored_before.is_none()
                && axis.is_some_and(|axis| {
                    (0..3).all(|i| {
                        if i == axis {
                            self.before[i].checked_add(1) == Some(self.after[i])
                                && self.after[i] <= MUD_MIXER_CAPACITY
                        } else {
                            self.before[i] == self.after[i]
                        }
                    })
                })
                && match self.wheelbarrow {
                    None => {
                        self.work_type == WorkType::HaulToMixer
                            && self.loaded_before.is_none()
                            && self.carried_before == Some(self.item)
                    }
                    Some(wheelbarrow) => {
                        self.work_type == WorkType::WheelbarrowHaul
                            && ![self.owner, self.worker, self.item].contains(&wheelbarrow)
                            && world.get::<Wheelbarrow>(wheelbarrow).is_some()
                            && self.carried_before == Some(wheelbarrow)
                            && self.loaded_before == Some(wheelbarrow)
                    }
                };
            let mut receipts = world.resource_mut::<RefineCommitReceipts>();
            if !valid {
                receipts
                    .failure
                    .get_or_insert("Mixer material conversion context differs");
            } else if receipts.total >= CAP {
                receipts
                    .failure
                    .get_or_insert("refine/material receipt cap exceeded");
            } else {
                receipts.total += 1;
                receipts.materials.push(self);
            }
        });
    }
}
