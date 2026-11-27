//! Terms of the first temporary electricity service and its durable site records.
use bevy::prelude::*;

#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct ReceptionTerms {
    pub point: IVec2,
    pub floor_start: IVec2,
    pub intake: IVec2,
    pub basin: IVec2,
    pub capacity_w: f32,
    pub setup_fee: f32,
    pub scheduled_at: f32,
    pub deadline: f32,
    pub handover_deadline: f32,
}

impl ReceptionTerms {
    pub fn riverfront(
        point: IVec2,
        floor_start: IVec2,
        intake: IVec2,
        basin: IVec2,
        now: f32,
    ) -> Self {
        let day = (now / 86400.).floor() * 86400.;
        let tonight = day + 18. * 3600.;
        let deadline = if now >= tonight {
            tonight + 86400.
        } else {
            tonight
        };
        Self {
            point,
            floor_start,
            intake,
            basin,
            capacity_w: 2.,
            setup_fee: 1.,
            scheduled_at: deadline - 1800.,
            deadline,
            handover_deadline: deadline + 12. * 3600.,
        }
    }
}

/// Marks the physical yard awarded this contract, including after world remapping.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ReceptionYard;

#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct ReceptionContract {
    pub terms: ReceptionTerms,
    pub accepted_at: f32,
    pub designation: u32,
    pub first_received_at: Option<f32>,
}

impl ReceptionContract {
    pub fn new(terms: ReceptionTerms, now: f32) -> Self {
        Self {
            terms,
            accepted_at: now,
            designation: 0,
            first_received_at: None,
        }
    }

    /// The utility's work proceeds independently of delivery and installation.
    pub fn try_energize(&mut self, panel: &mut ReceivingPanel, now: f32) -> bool {
        if panel.designation != self.designation
            || panel.live
            || !panel.reported
            || now < self.terms.scheduled_at
        {
            return false;
        }
        panel.live = true;
        if self.first_received_at.is_none() {
            self.first_received_at = Some(now);
            return true;
        }
        false
    }
}

/// Only the nominated blueprint and its completed successor carry this component.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct ReceivingPanel {
    pub designation: u32,
    pub reported: bool,
    pub live: bool,
}
