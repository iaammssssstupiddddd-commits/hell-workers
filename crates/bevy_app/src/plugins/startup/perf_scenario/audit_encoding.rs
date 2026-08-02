use super::*;

#[cfg(feature = "profiling")]
pub(super) fn write_transform(
    record: &mut Vec<u8>,
    transform: &Transform,
    label: &str,
) -> Result<(), String> {
    write_f32(record, transform.translation.x, label)?;
    write_f32(record, transform.translation.y, label)?;
    write_f32(record, transform.translation.z, label)
}

#[cfg(feature = "profiling")]
pub(super) fn write_vec2(record: &mut Vec<u8>, value: Vec2, label: &str) -> Result<(), String> {
    write_f32(record, value.x, label)?;
    write_f32(record, value.y, label)
}

#[cfg(feature = "profiling")]
pub(super) fn write_f32(record: &mut Vec<u8>, value: f32, label: &str) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{label} contains non-finite value {value}"));
    }
    let normalized = if value == 0.0 { 0.0 } else { value };
    record.extend_from_slice(&normalized.to_bits().to_le_bytes());
    Ok(())
}

#[cfg(feature = "profiling")]
pub(super) fn write_u64(record: &mut Vec<u8>, value: u64) {
    record.extend_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "profiling")]
pub(super) fn write_option_u32(record: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            record.push(1);
            record.extend_from_slice(&value.to_le_bytes());
        }
        None => record.push(0),
    }
}

#[cfg(feature = "profiling")]
pub(super) fn write_option_u64(record: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            record.push(1);
            write_u64(record, value);
        }
        None => record.push(0),
    }
}

#[cfg(feature = "profiling")]
pub(super) fn write_idle_state(record: &mut Vec<u8>, idle: &IdleState) -> Result<(), String> {
    write_f32(record, idle.idle_timer, "idle timer")?;
    write_f32(record, idle.total_idle_time, "total idle time")?;
    write_idle_behavior(record, idle.behavior);
    write_f32(record, idle.behavior_duration, "idle behavior duration")?;
    write_gathering_behavior(record, idle.gathering_behavior);
    write_f32(
        record,
        idle.gathering_behavior_timer,
        "gathering behavior timer",
    )?;
    write_f32(
        record,
        idle.gathering_behavior_duration,
        "gathering behavior duration",
    )?;
    record.push(u8::from(idle.needs_separation));
    Ok(())
}

#[cfg(feature = "profiling")]
pub(super) fn write_idle_behavior(record: &mut Vec<u8>, behavior: IdleBehavior) {
    record.push(match behavior {
        IdleBehavior::Wandering => 0,
        IdleBehavior::Sitting => 1,
        IdleBehavior::Sleeping => 2,
        IdleBehavior::Gathering => 3,
        IdleBehavior::ExhaustedGathering => 4,
        IdleBehavior::Resting => 5,
        IdleBehavior::GoingToRest => 6,
        IdleBehavior::Escaping => 7,
        IdleBehavior::Drifting => 8,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_gathering_behavior(record: &mut Vec<u8>, behavior: GatheringBehavior) {
    record.push(match behavior {
        GatheringBehavior::Wandering => 0,
        GatheringBehavior::Sleeping => 1,
        GatheringBehavior::Standing => 2,
        GatheringBehavior::Dancing => 3,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_path(record: &mut Vec<u8>, path: &Path, label: &str) -> Result<(), String> {
    write_u64(record, path.waypoints.len() as u64);
    write_u64(record, path.current_index as u64);
    for waypoint in &path.waypoints {
        write_vec2(record, *waypoint, label)?;
    }
    match path.planned_destination {
        Some(destination) => {
            record.push(1);
            write_vec2(record, destination, label)?;
        }
        None => record.push(0),
    }
    write_u64(record, path.validated_obstacle_version);
    Ok(())
}

#[cfg(feature = "profiling")]
pub(super) fn write_assigned_task(
    record: &mut Vec<u8>,
    task: &AssignedTask,
    target_transforms: &Query<&Transform>,
) -> Result<(), String> {
    match task {
        AssignedTask::None => record.push(0),
        AssignedTask::Gather(data) => {
            record.push(1);
            write_work_type(record, data.work_type);
            match data.phase {
                GatherPhase::GoingToResource => record.push(0),
                GatherPhase::Collecting { progress } => {
                    record.push(1);
                    write_f32(record, progress, "gather progress")?;
                }
                GatherPhase::Done => record.push(2),
            }
            let target = target_transforms
                .get(data.target)
                .map_err(|_| "gather task references an entity without a transform".to_string())?;
            write_transform(record, target, "gather target transform")?;
        }
        AssignedTask::Haul(data) => {
            record.push(2);
            record.push(match data.phase {
                HaulPhase::GoingToItem => 0,
                HaulPhase::GoingToStockpile => 1,
                HaulPhase::Dropping => 2,
            });
            let item = target_transforms
                .get(data.item)
                .map_err(|_| "haul task references an item without a transform".to_string())?;
            write_transform(record, item, "haul item transform")?;
            let stockpile = target_transforms
                .get(data.stockpile)
                .map_err(|_| "haul task references a stockpile without a transform".to_string())?;
            write_transform(record, stockpile, "haul stockpile transform")?;
        }
        _ => {
            return Err(
                "gather determinism audit encountered an unsupported AssignedTask variant"
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(feature = "profiling")]
pub(super) fn write_familiar_state(
    record: &mut Vec<u8>,
    familiar: &Familiar,
    command: &ActiveCommand,
    operation: &FamiliarOperation,
    policy: &FamiliarPolicy,
    ai_state: &FamiliarAiState,
) -> Result<(), String> {
    record.push(match familiar.familiar_type {
        hw_core::familiar::FamiliarType::Imp => 0,
    });
    write_f32(record, familiar.command_radius, "familiar command radius")?;
    write_f32(record, familiar.efficiency, "familiar efficiency")?;
    record.extend_from_slice(&familiar.color_index.to_le_bytes());
    record.push(match command.command {
        FamiliarCommand::Idle => 0,
        FamiliarCommand::GatherResources => 1,
        FamiliarCommand::Patrol => 2,
    });
    write_f32(
        record,
        operation.fatigue_threshold,
        "familiar fatigue threshold",
    )?;
    write_u64(record, operation.max_controlled_soul as u64);
    for work_type in WorkType::ALL {
        let rule = policy.rule_for(work_type);
        record.push(u8::from(rule.allowed));
        record.push(match rule.priority {
            hw_core::familiar::FamiliarWorkPriority::Low => 0,
            hw_core::familiar::FamiliarWorkPriority::Normal => 1,
            hw_core::familiar::FamiliarWorkPriority::High => 2,
        });
    }
    match ai_state {
        FamiliarAiState::Idle => record.push(0),
        FamiliarAiState::SearchingTask => record.push(1),
        FamiliarAiState::Scouting { .. } => record.push(2),
        FamiliarAiState::Supervising { target, timer } => {
            record.push(3);
            record.push(u8::from(target.is_some()));
            write_f32(record, *timer, "familiar supervising timer")?;
        }
    }
    Ok(())
}

#[cfg(feature = "profiling")]
pub(super) fn write_work_type(record: &mut Vec<u8>, work_type: WorkType) {
    record.push(match work_type {
        WorkType::Chop => 0,
        WorkType::Mine => 1,
        WorkType::Build => 2,
        WorkType::Move => 3,
        WorkType::Haul => 4,
        WorkType::HaulToMixer => 5,
        WorkType::GatherWater => 6,
        WorkType::CollectBone => 7,
        WorkType::Refine => 8,
        WorkType::HaulWaterToMixer => 9,
        WorkType::WheelbarrowHaul => 10,
        WorkType::ReinforceFloorTile => 11,
        WorkType::PourFloorTile => 12,
        WorkType::FrameWallTile => 13,
        WorkType::CoatWall => 14,
        WorkType::GeneratePower => 15,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_door_state(record: &mut Vec<u8>, state: DoorState) {
    record.push(match state {
        DoorState::Open => 0,
        DoorState::Closed => 1,
        DoorState::Locked => 2,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_floor_phase(record: &mut Vec<u8>, phase: FloorConstructionPhase) {
    record.push(match phase {
        FloorConstructionPhase::Reinforcing => 0,
        FloorConstructionPhase::Pouring => 1,
        FloorConstructionPhase::Curing => 2,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_floor_tile_state(record: &mut Vec<u8>, state: FloorTileState) {
    record.push(match state {
        FloorTileState::WaitingBones => 0,
        FloorTileState::ReinforcingReady => 1,
        FloorTileState::Reinforcing { .. } => 2,
        FloorTileState::ReinforcedComplete => 3,
        FloorTileState::WaitingMud => 4,
        FloorTileState::PouringReady => 5,
        FloorTileState::Pouring { .. } => 6,
        FloorTileState::Complete => 7,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_building_type(record: &mut Vec<u8>, kind: BuildingType) {
    record.push(match kind {
        BuildingType::Wall => 0,
        BuildingType::Door => 1,
        BuildingType::Floor => 2,
        BuildingType::Tank => 3,
        BuildingType::MudMixer => 4,
        BuildingType::RestArea => 5,
        BuildingType::Bridge => 6,
        BuildingType::SandPile => 7,
        BuildingType::BonePile => 8,
        BuildingType::WheelbarrowParking => 9,
        BuildingType::SoulSpa => 10,
        BuildingType::OutdoorLamp => 11,
    });
}

#[cfg(feature = "profiling")]
pub(super) fn write_grid_pos(record: &mut Vec<u8>, grid: (i32, i32)) {
    record.extend_from_slice(&grid.0.to_le_bytes());
    record.extend_from_slice(&grid.1.to_le_bytes());
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    use hw_core::familiar::{FamiliarWorkPriority, FamiliarWorkRule, FamiliarWorkRuleOverride};
    use hw_jobs::HaulData;

    fn encoded(policy: &FamiliarPolicy) -> Vec<u8> {
        let mut record = Vec::new();
        write_familiar_state(
            &mut record,
            &Familiar::default(),
            &ActiveCommand::default(),
            &FamiliarOperation::default(),
            policy,
            &FamiliarAiState::default(),
        )
        .unwrap();
        record
    }

    #[test]
    fn familiar_audit_encodes_effective_policy_not_raw_override_shape() {
        let disabled_high = FamiliarWorkRule {
            allowed: false,
            priority: FamiliarWorkPriority::High,
        };
        let mut canonical = FamiliarPolicy::default();
        canonical.set_rule(WorkType::Chop, disabled_high);
        let noncanonical_same = FamiliarPolicy {
            default_rule: FamiliarWorkRule::default(),
            overrides: vec![
                FamiliarWorkRuleOverride {
                    work_type: WorkType::Mine,
                    rule: FamiliarWorkRule::default(),
                },
                FamiliarWorkRuleOverride {
                    work_type: WorkType::Chop,
                    rule: FamiliarWorkRule::default(),
                },
                FamiliarWorkRuleOverride {
                    work_type: WorkType::Chop,
                    rule: disabled_high,
                },
            ],
        };

        assert_eq!(encoded(&canonical), encoded(&noncanonical_same));
        assert_ne!(encoded(&canonical), encoded(&FamiliarPolicy::default()));
    }

    #[test]
    fn haul_audit_encodes_each_phase_without_entity_ids() {
        let mut world = World::new();
        let item = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();
        let stockpile = world.spawn(Transform::from_xyz(4.0, 5.0, 6.0)).id();
        let mut query_state = SystemState::<Query<&Transform>>::new(&mut world);
        let query = query_state.get(&world).expect("transform query validates");

        let mut encodings = Vec::new();
        for phase in [
            HaulPhase::GoingToItem,
            HaulPhase::GoingToStockpile,
            HaulPhase::Dropping,
        ] {
            let mut record = Vec::new();
            write_assigned_task(
                &mut record,
                &AssignedTask::Haul(HaulData {
                    item,
                    stockpile,
                    phase,
                }),
                &query,
            )
            .unwrap();
            encodings.push(record);
        }

        assert_eq!(encodings[0][0..2], [2, 0]);
        assert_eq!(encodings[1][0..2], [2, 1]);
        assert_eq!(encodings[2][0..2], [2, 2]);
        assert_ne!(encodings[0], encodings[1]);
        assert_ne!(encodings[1], encodings[2]);
    }
}
