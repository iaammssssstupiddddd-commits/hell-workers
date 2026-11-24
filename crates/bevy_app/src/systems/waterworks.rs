use bevy::prelude::*;
use hw_core::{GameTime, soul::DreamPool};
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_ui::notifications::{NotificationRetention, NotificationSeverity, UserFacingNotification};
use hw_ui::selection::{PlacementRejectReason, PlacementValidation};
use hw_world::{GridRect, WorldMap};

use crate::world::map::GeneratedWorldLayoutResource;

pub(crate) const HOURS: u32 = 12;
pub(crate) const FULL_REWARD: u32 = 40;
pub(crate) const LATE_REWARD: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub(crate) struct ContractDistrict {
    pub min: IVec2,
    pub max: IVec2,
}

impl From<GridRect> for ContractDistrict {
    fn from(yard: GridRect) -> Self {
        Self {
            min: IVec2::new(yard.min_x, yard.min_y),
            max: IVec2::new(yard.max_x, yard.max_y),
        }
    }
}

impl ContractDistrict {
    pub fn contains(self, (x, y): (i32, i32)) -> bool {
        (self.min.x..=self.max.x).contains(&x) && (self.min.y..=self.max.y).contains(&y)
    }

    pub fn validate(self, cells: &[(i32, i32)]) -> PlacementValidation {
        if let Some(cell) = cells.iter().find(|cell| !self.contains(**cell)) {
            PlacementValidation::rejected_at(PlacementRejectReason::OutsideContractDistrict, *cell)
        } else {
            PlacementValidation::ok()
        }
    }
}

#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub(crate) struct WaterworksTarget(pub u32);

#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub(crate) enum ContractOutcome {
    Active,
    Inspected { at: f32, reward: u32 },
    Paid { at: f32, reward: u32 },
}

#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub(crate) struct WaterworksContract {
    pub district: ContractDistrict,
    pub accepted: f32,
    pub deadline: f32,
    pub full_reward: u32,
    pub late_reward: u32,
    pub designation: u32,
    pub outcome: ContractOutcome,
}

impl WaterworksContract {
    pub fn new(district: ContractDistrict, now: f32) -> Self {
        Self {
            district,
            accepted: now,
            deadline: now + (HOURS * 3600) as f32,
            full_reward: FULL_REWARD,
            late_reward: LATE_REWARD,
            designation: 0,
            outcome: ContractOutcome::Active,
        }
    }

    fn reward(&self, at: f32) -> u32 {
        if at <= self.deadline {
            self.full_reward
        } else {
            self.late_reward
        }
    }

    fn inspect(&mut self, now: f32) -> Option<u32> {
        if self.outcome != ContractOutcome::Active {
            return None;
        }
        let reward = self.reward(now);
        self.outcome = ContractOutcome::Inspected { at: now, reward };
        Some(reward)
    }

    fn pay(&mut self, pool: &mut DreamPool) -> Option<u32> {
        let ContractOutcome::Inspected { at, reward } = self.outcome else {
            return None;
        };
        pool.points += reward as f32;
        self.outcome = ContractOutcome::Paid { at, reward };
        Some(reward)
    }
}

pub(crate) fn accept(world: &mut World) {
    if world
        .query::<&WaterworksContract>()
        .iter(world)
        .next()
        .is_some()
    {
        return;
    }
    let yard = world
        .resource::<GeneratedWorldLayoutResource>()
        .layout
        .anchors
        .yard;
    let now = world.resource::<GameTime>().seconds;
    world.spawn(WaterworksContract::new(yard.into(), now));
}

pub(crate) fn claim(world: &mut World) {
    let reward = world.resource_scope(|world, mut pool: Mut<DreamPool>| {
        world
            .query::<&mut WaterworksContract>()
            .single_mut(world)
            .ok()?
            .pay(&mut pool)
    });
    if let Some(reward) = reward {
        world.write_message(notice(
            "waterworks-paid",
            "給水所の報酬を受け取りました",
            format!("Dream +{reward}"),
        ));
    }
}

fn notice(key: &'static str, title: &'static str, body: String) -> UserFacingNotification {
    UserFacingNotification::new(
        key,
        NotificationSeverity::Success,
        title,
        body,
        NotificationRetention::Important,
    )
}

fn tank_footprint(district: ContractDistrict, transform: &Transform) -> Vec<(i32, i32)> {
    use crate::interface::selection::placement_geometry::{
        existing_movable_building_anchor, move_occupied_grids,
    };
    let anchor =
        existing_movable_building_anchor(BuildingType::Tank, transform.translation.truncate());
    if !transform.translation.is_finite() || !district.contains(anchor) {
        return Vec::new();
    }
    move_occupied_grids(BuildingType::Tank, anchor)
}

mod stations;
pub(crate) use stations::{StationProgress, Stations};

pub(crate) fn inspect_delivery(
    delivery: On<hw_jobs::events::TankWaterDelivered>,
    clock: Res<GameTime>,
    mut contracts: Query<&mut WaterworksContract>,
    stations: Stations,
    targets: Query<Entity, With<WaterworksTarget>>,
    mut commands: Commands,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    let Ok(mut contract) = contracts.single_mut() else {
        return;
    };
    if contract.outcome != ContractOutcome::Active
        || !stations.eligible(contract.district, delivery.tank)
    {
        return;
    }
    if let Some(reward) = contract.inspect(clock.seconds) {
        for entity in &targets {
            commands.entity(entity).try_remove::<WaterworksTarget>();
        }
        notifications.write(notice(
            "waterworks-inspected",
            "給水所の初回給水を確認",
            format!("管理 → 契約から {reward} Dream を受け取れます。"),
        ));
    }
}

type MovedTanks<'w, 's> =
    Query<'w, 's, Entity, (With<WaterworksTarget>, With<Building>, Changed<Transform>)>;

pub(crate) fn release_moved_target(
    contracts: Query<&WaterworksContract>,
    targets: MovedTanks,
    stations: Stations,
    mut commands: Commands,
) {
    let Ok(tank) = targets.single() else {
        return;
    };
    if let Ok(contract) = contracts.single()
        && !stations.eligible(contract.district, tank)
    {
        commands.entity(tank).remove::<WaterworksTarget>();
    }
}

mod candidate;
pub(crate) use candidate::validate_candidate;

#[cfg(test)]
mod tests;
