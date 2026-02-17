//! パフォーマンス計測シナリオの構成
//!
//! Phase 5: perf scenario 関連を startup から分離。

use crate::entities::familiar::{ActiveCommand, FamiliarCommand, FamiliarOperation};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, Rock, TaskSlots, Tree, WorkType};
use bevy::prelude::*;
use std::env;

fn has_cli_flag(flag: &str) -> bool {
    env::args().any(|arg| arg == flag)
}

fn is_perf_scenario_enabled() -> bool {
    has_cli_flag("--perf-scenario") || env::var("HW_PERF_SCENARIO").is_ok_and(|v| v == "1")
}

#[derive(Resource, Default)]
pub(crate) struct PerfScenarioApplied(pub(crate) bool);

pub fn setup_perf_scenario_if_enabled(
    mut commands: Commands,
    mut q_familiars: Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_trees: Query<Entity, With<Tree>>,
    q_rocks: Query<Entity, With<Rock>>,
) {
    if !is_perf_scenario_enabled() {
        return;
    }

    let area = TaskArea {
        min: Vec2::new(-1600.0, -1600.0),
        max: Vec2::new(1600.0, 1600.0),
    };

    for (fam_entity, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(fam_entity).insert(area.clone());
    }

    for tree_entity in q_trees.iter() {
        commands.entity(tree_entity).insert((
            Designation {
                work_type: WorkType::Chop,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
    }

    for rock_entity in q_rocks.iter() {
        commands.entity(rock_entity).insert((
            Designation {
                work_type: WorkType::Mine,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
    }
}

pub fn setup_perf_scenario_runtime_if_enabled(
    mut commands: Commands,
    mut applied: ResMut<PerfScenarioApplied>,
    mut q_familiars: Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
) {
    if applied.0 {
        return;
    }
    if !is_perf_scenario_enabled() {
        return;
    }
    if q_familiars.is_empty() {
        return;
    }

    let area = TaskArea {
        min: Vec2::new(-1600.0, -1600.0),
        max: Vec2::new(1600.0, 1600.0),
    };
    for (fam_entity, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(fam_entity).insert(area.clone());
    }

    applied.0 = true;
}
