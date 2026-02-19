mod builders;

use crate::constants::ESCAPE_STRESS_THRESHOLD;
use crate::entities::damned_soul::{DamnedSoul, Gender, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::interface::ui::components::TooltipTemplate;
use crate::relationships::CommandedBy;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::perceive::escaping::is_escape_threat_close;
use crate::systems::spatial::FamiliarSpatialGrid;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(Clone, PartialEq)]
pub struct SoulInspectionFields {
    pub gender: Option<Gender>,
    pub motivation: String,
    pub stress: String,
    pub fatigue: String,
    pub task: String,
    pub inventory: String,
    pub common: String,
}

#[derive(Clone, PartialEq)]
pub struct EntityInspectionModel {
    pub header: String,
    pub common_text: String,
    pub tooltip_lines: Vec<String>,
    pub soul: Option<SoulInspectionFields>,
}

#[derive(SystemParam)]
pub struct EntityInspectionQuery<'w, 's> {
    q_souls: Query<
        'w,
        's,
        (
            &'static DamnedSoul,
            &'static AssignedTask,
            &'static Transform,
            &'static IdleState,
            Option<&'static CommandedBy>,
            Option<&'static crate::systems::logistics::Inventory>,
            Option<&'static crate::entities::damned_soul::SoulIdentity>,
        ),
    >,
    q_blueprints: Query<'w, 's, &'static Blueprint>,
    q_familiars: Query<
        'w,
        's,
        (
            &'static Familiar,
            &'static crate::entities::familiar::FamiliarOperation,
        ),
    >,
    q_familiars_escape: Query<'w, 's, (&'static Transform, &'static Familiar)>,
    familiar_grid: Res<'w, FamiliarSpatialGrid>,
    q_items: Query<'w, 's, &'static crate::systems::logistics::ResourceItem>,
    q_trees: Query<'w, 's, &'static crate::systems::jobs::Tree>,
    q_rocks: Query<'w, 's, &'static crate::systems::jobs::Rock>,
    q_designations: Query<
        'w,
        's,
        (
            &'static crate::systems::jobs::Designation,
            Option<&'static crate::systems::jobs::IssuedBy>,
            Option<&'static TaskWorkers>,
        ),
    >,
    q_buildings: Query<
        'w,
        's,
        (
            &'static crate::systems::jobs::Building,
            Option<&'static crate::systems::jobs::ProvisionalWall>,
            Option<&'static crate::systems::logistics::Stockpile>,
            Option<&'static crate::relationships::StoredItems>,
            Option<&'static crate::systems::jobs::MudMixerStorage>,
            Option<&'static crate::systems::jobs::RestArea>,
            Option<&'static crate::relationships::RestAreaOccupants>,
        ),
    >,
}

#[derive(Default)]
struct InspectionAccumulator {
    header: String,
    common_lines: Vec<String>,
    tooltip_lines: Vec<String>,
    soul_fields: Option<SoulInspectionFields>,
}

impl InspectionAccumulator {
    fn push_common(&mut self, line: impl Into<String>) {
        self.common_lines.push(line.into());
    }

    fn push_tooltip(&mut self, line: impl Into<String>) {
        self.tooltip_lines.push(line.into());
    }

    fn finalize(mut self) -> Option<EntityInspectionModel> {
        if self.header.is_empty() && self.tooltip_lines.is_empty() {
            return None;
        }

        if self.tooltip_lines.is_empty() {
            self.tooltip_lines.push(self.header.clone());
        }

        Some(EntityInspectionModel {
            header: self.header,
            common_text: self.common_lines.join("\n"),
            tooltip_lines: self.tooltip_lines,
            soul: self.soul_fields,
        })
    }
}

impl EntityInspectionQuery<'_, '_> {
    pub fn build_model(&self, entity: Entity) -> Option<EntityInspectionModel> {
        let mut model = InspectionAccumulator::default();

        let _ = self.build_soul_model(entity, &mut model)
            || self.build_blueprint_model(entity, &mut model)
            || self.build_familiar_model(entity, &mut model)
            || self.build_item_model(entity, &mut model)
            || self.build_tree_model(entity, &mut model)
            || self.build_rock_model(entity, &mut model);

        self.append_building_model(entity, &mut model);
        self.append_designation_model(entity, &mut model);

        model.finalize()
    }

    pub fn classify_template(&self, entity: Entity) -> TooltipTemplate {
        if self.q_souls.get(entity).is_ok() {
            TooltipTemplate::Soul
        } else if self.q_buildings.get(entity).is_ok() || self.q_blueprints.get(entity).is_ok() {
            TooltipTemplate::Building
        } else if self.q_items.get(entity).is_ok()
            || self.q_trees.get(entity).is_ok()
            || self.q_rocks.get(entity).is_ok()
        {
            TooltipTemplate::Resource
        } else {
            TooltipTemplate::Generic
        }
    }
}

pub(super) fn format_task_str(task: &AssignedTask) -> String {
    match task {
        AssignedTask::None => "Idle".to_string(),
        AssignedTask::Gather(data) => format!("Gather ({:?})", data.phase),
        AssignedTask::Haul(data) => format!("Haul ({:?})", data.phase),
        AssignedTask::HaulToBlueprint(data) => format!("HaulToBp ({:?})", data.phase),
        AssignedTask::Build(data) => format!("Build ({:?})", data.phase),
        AssignedTask::GatherWater(data) => format!("GatherWater ({:?})", data.phase),
        AssignedTask::CollectSand(data) => format!("CollectSand ({:?})", data.phase),
        AssignedTask::CollectBone(data) => format!("CollectBone ({:?})", data.phase),
        AssignedTask::Refine(data) => format!("Refine ({:?})", data.phase),
        AssignedTask::HaulToMixer(data) => format!("HaulToMixer ({:?})", data.phase),
        AssignedTask::HaulWaterToMixer(data) => format!("HaulWaterToMixer ({:?})", data.phase),
        AssignedTask::HaulWithWheelbarrow(data) => format!("HaulWheelbarrow ({:?})", data.phase),
        AssignedTask::ReinforceFloorTile(data) => format!("ReinforceFloor ({:?})", data.phase),
        AssignedTask::PourFloorTile(data) => format!("PourFloor ({:?})", data.phase),
        AssignedTask::FrameWallTile(data) => format!("FrameWall ({:?})", data.phase),
        AssignedTask::CoatWall(data) => format!("CoatWall ({:?})", data.phase),
    }
}

pub(super) fn format_inventory_str(
    inventory_opt: Option<&crate::systems::logistics::Inventory>,
    q_items: &Query<&crate::systems::logistics::ResourceItem>,
) -> String {
    if let Some(crate::systems::logistics::Inventory(Some(item_entity))) = inventory_opt {
        if let Ok(item) = q_items.get(*item_entity) {
            format!("Carrying: {:?}", item.0)
        } else {
            format!("Carrying: Entity {:?}", item_entity)
        }
    } else {
        "Carrying: None".to_string()
    }
}

pub(super) fn format_escape_info(
    soul: &DamnedSoul,
    transform: &Transform,
    idle: &IdleState,
    under_command: Option<&CommandedBy>,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars_escape: &Query<(&Transform, &Familiar)>,
) -> String {
    let escape_threat_close = is_escape_threat_close(
        transform.translation.truncate(),
        familiar_grid,
        q_familiars_escape,
    );
    let escape_allowed = under_command.is_none()
        && idle.behavior != IdleBehavior::ExhaustedGathering
        && soul.stress > ESCAPE_STRESS_THRESHOLD
        && escape_threat_close;
    format!(
        "Idle: {:?}\nEscape: {}\n- stress_ok: {}\n- threat_close: {}\n- commanded: {}\n- exhausted: {}",
        idle.behavior,
        if escape_allowed {
            "eligible"
        } else {
            "blocked"
        },
        soul.stress > ESCAPE_STRESS_THRESHOLD,
        escape_threat_close,
        under_command.is_some(),
        idle.behavior == IdleBehavior::ExhaustedGathering
    )
}
