//! Task labels shared by roster, inspection, and Tooltip adapters.
use hw_jobs::{AssignedTask, WorkType};
use hw_ui::list::TaskVisual;

pub(crate) struct TaskKindPresentation {
    pub list_visual: TaskVisual,
    pub detail_name: &'static str,
}

pub(crate) fn task_kind_presentation(task: &AssignedTask) -> TaskKindPresentation {
    let (list_visual, detail_name) = match task {
        AssignedTask::None => (TaskVisual::Idle, "Idle"),
        AssignedTask::Gather(data) => (
            match data.work_type {
                WorkType::Chop => TaskVisual::Chop,
                WorkType::Mine => TaskVisual::Mine,
                _ => TaskVisual::GatherDefault,
            },
            "Gather",
        ),
        AssignedTask::Haul(_) => (TaskVisual::Haul, "Haul"),
        AssignedTask::HaulToBlueprint(_) => (TaskVisual::HaulToBlueprint, "HaulToBp"),
        AssignedTask::Build(_) => (TaskVisual::Build, "Build"),
        AssignedTask::MovePlant(_) => (TaskVisual::Move, "MovePlant"),
        AssignedTask::BucketTransport(_) => (TaskVisual::Water, "BucketTransport"),
        AssignedTask::CollectBone(_) => (TaskVisual::CollectBone, "CollectBone"),
        AssignedTask::Refine(_) => (TaskVisual::Refine, "Refine"),
        AssignedTask::HaulToMixer(_) => (TaskVisual::HaulToBlueprint, "HaulToMixer"),
        AssignedTask::HaulWithWheelbarrow(_) => (TaskVisual::Haul, "HaulWheelbarrow"),
        AssignedTask::ReinforceFloorTile(_) => (TaskVisual::Build, "ReinforceFloor"),
        AssignedTask::PourFloorTile(_) => (TaskVisual::Build, "PourFloor"),
        AssignedTask::FrameWallTile(_) => (TaskVisual::Build, "FrameWall"),
        AssignedTask::CoatWall(_) => (TaskVisual::Build, "CoatWall"),
        AssignedTask::GeneratePower(_) => (TaskVisual::GeneratePower, "GeneratePower"),
        AssignedTask::Deconstruct(_) => (TaskVisual::Deconstruct, "Deconstruct"),
    };
    TaskKindPresentation {
        list_visual,
        detail_name,
    }
}

fn format_task_phase(task: &AssignedTask) -> Option<String> {
    Some(match task {
        AssignedTask::None => return None,
        AssignedTask::Gather(data) => format!("{:?}", data.phase),
        AssignedTask::Haul(data) => format!("{:?}", data.phase),
        AssignedTask::HaulToBlueprint(data) => format!("{:?}", data.phase),
        AssignedTask::Build(data) => format!("{:?}", data.phase),
        AssignedTask::MovePlant(data) => format!("{:?}", data.phase),
        AssignedTask::BucketTransport(data) => format!("{:?}", data.phase),
        AssignedTask::CollectBone(data) => format!("{:?}", data.phase),
        AssignedTask::Refine(data) => format!("{:?}", data.phase),
        AssignedTask::HaulToMixer(data) => format!("{:?}", data.phase),
        AssignedTask::HaulWithWheelbarrow(data) => format!("{:?}", data.phase),
        AssignedTask::ReinforceFloorTile(data) => format!("{:?}", data.phase),
        AssignedTask::PourFloorTile(data) => format!("{:?}", data.phase),
        AssignedTask::FrameWallTile(data) => format!("{:?}", data.phase),
        AssignedTask::CoatWall(data) => format!("{:?}", data.phase),
        AssignedTask::GeneratePower(data) => format!("{:?}", data.phase),
        AssignedTask::Deconstruct(data) => format!("{:?}", data.phase),
    })
}

pub(crate) fn format_task_str(task: &AssignedTask) -> String {
    let name = task_kind_presentation(task).detail_name;
    match format_task_phase(task) {
        Some(phase) => format!("{name} ({phase})"),
        None => name.to_owned(),
    }
}

#[cfg(test)]
mod tests;
