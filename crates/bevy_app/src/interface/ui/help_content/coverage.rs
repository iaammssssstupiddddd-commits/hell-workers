use std::collections::BTreeSet;

use hw_core::familiar::{FamiliarSettingsPatch, FamiliarWorkPriority};
use hw_core::game_state::{PlayMode, TaskMode, TaskModeZoneType, TimeSpeed};
use hw_core::jobs::WorkType;
use hw_jobs::{BuildingCategory, BuildingType};
use hw_logistics::transport_request::{TransportPriority, TransportRequestKind};
use hw_logistics::zone::ZoneType;
use hw_logistics::{ResourceType, StockpileAcceptance};
use hw_ui::UiIntent;
use hw_ui::components::MenuState;
use hw_ui::help::{
    HelpChromeSlot, HelpEntryId, HelpPanelContent, HelpScrollCommand, HelpTopicStep,
};
use hw_ui::panels::task_list::{
    TaskDashboardControl, TaskPriorityFilter, TaskSortDirection, TaskSortKey, TaskStatusFilter,
    TaskWorkTypeFilter, TaskWorkerFilter,
};

use crate::input_actions::InputAction;

use super::{HelpCatalogError, manifest::HelpOwnerId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpAudience {
    Player,
    Internal,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublishedTarget {
    Entry(HelpEntryId),
    Launcher,
    Chrome(HelpChromeSlot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockedTarget {
    Entry(HelpEntryId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpExclusionReason {
    InternalMechanism,
    DebugOnly,
    DependencyDefaultNotProjectContract,
    UnreachablePlayerFlow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpBlockerReason {
    MissingCompletionConsumer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpCoverage {
    Published(PublishedTarget),
    Excluded(HelpExclusionReason),
    Blocked {
        target: BlockedTarget,
        reason: HelpBlockerReason,
        owner: HelpOwnerId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SurfaceCoverage {
    audience: HelpAudience,
    coverage: HelpCoverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoverageRecord {
    surface: &'static str,
    decision: SurfaceCoverage,
}

const fn published(entry: &'static str) -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Player,
        coverage: HelpCoverage::Published(PublishedTarget::Entry(HelpEntryId::new(entry))),
    }
}

const fn launcher() -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Player,
        coverage: HelpCoverage::Published(PublishedTarget::Launcher),
    }
}

const fn chrome(slot: HelpChromeSlot) -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Player,
        coverage: HelpCoverage::Published(PublishedTarget::Chrome(slot)),
    }
}

const fn internal() -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Internal,
        coverage: HelpCoverage::Excluded(HelpExclusionReason::InternalMechanism),
    }
}

const fn debug_only() -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Debug,
        coverage: HelpCoverage::Excluded(HelpExclusionReason::DebugOnly),
    }
}

const fn dependency_default_camera_control() -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Player,
        coverage: HelpCoverage::Excluded(HelpExclusionReason::DependencyDefaultNotProjectContract),
    }
}

const fn unreachable_player_flow() -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Player,
        coverage: HelpCoverage::Excluded(HelpExclusionReason::UnreachablePlayerFlow),
    }
}

macro_rules! coverage_variant_pattern {
    ($enum:ident, unit($variant:ident)) => {
        $enum::$variant
    };
    ($enum:ident, tuple($variant:ident($($payload:pat),* $(,)?))) => {
        $enum::$variant($($payload),*)
    };
    ($enum:ident, record($variant:ident { $($fields:tt)* })) => {
        $enum::$variant { $($fields)* }
    };
}

/// Declares stable Help decision rows whose patterns each name one outer enum variant.
///
/// The `unit` / `tuple` / `record` forms deliberately accept only one outer
/// variant, so a new variant cannot be hidden in an existing top-level
/// or-pattern without adding a new stable surface ID. Payload carriers may use
/// multiple non-overlapping rows when their nested variants have distinct
/// player-facing decisions.
macro_rules! coverage_table {
    (
        enum $enum:ident;
        fn $classifier:ident($value:ident: $value_type:ty);
        fn $decisions:ident();
        {
            $($surface:literal => $shape:ident $payload:tt => $decision:expr),+ $(,)?
        }
    ) => {
        fn $classifier($value: $value_type) -> SurfaceCoverage {
            match $value {
                $(coverage_variant_pattern!($enum, $shape $payload) => $decision),+
            }
        }

        const _: fn($value_type) -> SurfaceCoverage = $classifier;

        fn $decisions() -> Vec<CoverageRecord> {
            vec![$(
                CoverageRecord {
                    surface: $surface,
                    decision: $decision,
                }
            ),+]
        }
    };
}

const fn familiar_build_coverage() -> SurfaceCoverage {
    SurfaceCoverage {
        audience: HelpAudience::Player,
        coverage: HelpCoverage::Blocked {
            target: BlockedTarget::Entry(HelpEntryId::new("familiar-build")),
            reason: HelpBlockerReason::MissingCompletionConsumer,
            owner: HelpOwnerId::FamiliarManagement,
        },
    }
}

coverage_table! {
    enum InputAction;
    fn input_action_coverage(action: InputAction);
    fn input_action_decisions();
    {
        "input::help-open" => unit(OpenHelp) => launcher(),
        "input::help-close" => unit(CloseHelp) => chrome(HelpChromeSlot::Close),
        "input::help-topic-previous" => unit(HelpPreviousTopic) => {
            chrome(HelpChromeSlot::TopicPrevious)
        },
        "input::help-topic-next" => unit(HelpNextTopic) => {
            chrome(HelpChromeSlot::TopicNext)
        },
        "input::help-page-up" => unit(HelpPageUp) => chrome(HelpChromeSlot::PageUp),
        "input::help-page-down" => unit(HelpPageDown) => chrome(HelpChromeSlot::PageDown),
        "input::help-home" => unit(HelpHome) => chrome(HelpChromeSlot::DocumentStart),
        "input::help-end" => unit(HelpEnd) => chrome(HelpChromeSlot::DocumentEnd),
        "input::save" => unit(SaveGame) => published("save-load"),
        "input::load" => unit(RequestLoadGame) => published("save-load"),
        "input::camera-elevation" => unit(CycleElevation) => published("camera-elevation"),
        "input::render-3d" => unit(ToggleRender3d) => debug_only(),
        "input::rtt-quality" => unit(CycleRttQuality) => debug_only(),
        "input::rtt-directional-light" => unit(ToggleRttDirectionalLight) => debug_only(),
        "input::rtt-terrain" => unit(ToggleRttTerrain) => debug_only(),
        "input::rtt-scene-objects" => unit(ToggleRttSceneObjects) => debug_only(),
        "input::debug-toggle" => unit(ToggleDebug) => debug_only(),
        "input::debug-spawn-soul" => unit(DebugSpawnSoul) => debug_only(),
        "input::debug-spawn-familiar" => unit(DebugSpawnFamiliar) => debug_only(),
        "input::architect" => unit(ToggleArchitect) => published("architect-building"),
        "input::zones" => unit(ToggleZones) => published("zones-workflow"),
        "input::pause-toggle" => unit(TogglePause) => published("time-controls"),
        "input::time-paused" => unit(TimePaused) => published("time-controls"),
        "input::time-normal" => unit(TimeNormal) => published("time-controls"),
        "input::time-fast" => unit(TimeFast) => published("time-controls"),
        "input::time-super" => unit(TimeSuper) => published("time-controls"),
        "input::familiar-chop" => unit(FamiliarChop) => published("familiar-designations"),
        "input::familiar-mine" => unit(FamiliarMine) => published("familiar-designations"),
        "input::familiar-haul" => unit(FamiliarHaul) => published("familiar-designations"),
        "input::familiar-build" => unit(FamiliarBuild) => familiar_build_coverage(),
        "input::familiar-cancel-designation" => unit(FamiliarCancelDesignation) => {
            published("familiar-designations")
        },
        "input::familiar-idle-patrol" => unit(ToggleFamiliarIdlePatrol) => {
            published("familiar-idle-patrol")
        },
        "input::load-cancel" => unit(CancelLoadConfirm) => published("save-load"),
        "input::settings-close" => unit(CloseSettings) => published("settings"),
        "input::operation-close" => unit(CloseOperationDialog) => {
            published("familiar-operation-policy")
        },
        "input::cancel-active-mode" => unit(CancelActiveMode) => {
            published("orders-designation")
        },
        "input::close-open-menu" => unit(CloseOpenMenu) => {
            published("orders-designation")
        },
        "input::area-copy" => unit(AreaCopy) => published("area-edit"),
        "input::area-paste" => unit(AreaPaste) => published("area-edit"),
        "input::area-undo" => unit(AreaUndo) => published("area-edit"),
        "input::area-redo" => unit(AreaRedo) => published("area-edit"),
        "input::area-save-preset-1" => unit(AreaSavePreset1) => published("area-edit"),
        "input::area-save-preset-2" => unit(AreaSavePreset2) => published("area-edit"),
        "input::area-save-preset-3" => unit(AreaSavePreset3) => published("area-edit"),
        "input::area-load-preset-1" => unit(AreaLoadPreset1) => published("area-edit"),
        "input::area-load-preset-2" => unit(AreaLoadPreset2) => published("area-edit"),
        "input::area-load-preset-3" => unit(AreaLoadPreset3) => published("area-edit"),
        "input::list-next" => unit(ListNext) => {
            published("entity-list-selection")
        },
        "input::list-previous" => unit(ListPrevious) => {
            published("entity-list-selection")
        }
    }
}

coverage_table! {
    enum UiIntent;
    fn ui_intent_coverage(intent: &UiIntent);
    fn ui_intent_decisions();
    {
        "ui-intent::help-open" => record(OpenHelp { .. }) => launcher(),
        "ui-intent::help-close" => unit(CloseHelp) => chrome(HelpChromeSlot::Close),
        "ui-intent::help-topic-select" => tuple(SelectHelpTopic(_)) => internal(),
        "ui-intent::help-topic-step" => tuple(StepHelpTopic(_)) => internal(),
        "ui-intent::help-scroll" => tuple(ScrollHelp(_)) => internal(),
        "ui-intent::architect-toggle" => unit(ToggleArchitect) => published("architect-building"),
        "ui-intent::zones-toggle" => unit(ToggleZones) => published("zones-workflow"),
        "ui-intent::orders-toggle" => unit(ToggleOrders) => published("orders-designation"),
        "ui-intent::dream-toggle" => unit(ToggleDream) => published("dream-planting"),
        "ui-intent::settings-toggle" => unit(ToggleSettings) => published("settings"),
        "ui-intent::settings-close" => unit(CloseSettings) => published("settings"),
        "ui-intent::settings-ui-scale" => tuple(SetUiScale(_)) => published("settings"),
        "ui-intent::settings-camera-pan-speed" => tuple(SetCameraPanSpeed(_)) => {
            published("settings")
        },
        "ui-intent::settings-camera-mouse-pan" => tuple(SetCameraMousePanEnabled(_)) => {
            published("settings")
        },
        "ui-intent::settings-default-time-speed" => tuple(SetDefaultTimeSpeed(_)) => {
            published("settings")
        },
        "ui-intent::settings-debug-gizmos" => tuple(SetDebugGizmosEnabled(_)) => {
            published("settings")
        },
        "ui-intent::settings-fps-display" => tuple(SetFpsDisplayEnabled(_)) => {
            published("settings")
        },
        "ui-intent::inspect-entity" => tuple(InspectEntity(_)) => published("info-panel-pin"),
        "ui-intent::clear-inspect-pin" => unit(ClearInspectPin) => published("info-panel-pin"),
        "ui-intent::select-build" => tuple(SelectBuild(_)) => published("architect-building"),
        "ui-intent::select-floor-place" => unit(SelectFloorPlace) => {
            published("architect-building")
        },
        "ui-intent::select-zone" => tuple(SelectZone(_)) => published("zones-workflow"),
        "ui-intent::remove-stockpile-zone" => tuple(RemoveZone(ZoneType::Stockpile)) => {
            published("zones-workflow")
        },
        "ui-intent::remove-yard-zone" => tuple(RemoveZone(ZoneType::Yard)) => {
            unreachable_player_flow()
        },
        "ui-intent::task-mode-none" => tuple(SelectTaskMode(TaskMode::None)) => internal(),
        "ui-intent::task-mode-designate-chop" => tuple(SelectTaskMode(TaskMode::DesignateChop(_))) => {
            published("orders-designation")
        },
        "ui-intent::task-mode-designate-mine" => tuple(SelectTaskMode(TaskMode::DesignateMine(_))) => {
            published("orders-designation")
        },
        "ui-intent::task-mode-designate-haul" => tuple(SelectTaskMode(TaskMode::DesignateHaul(_))) => {
            published("orders-designation")
        },
        "ui-intent::task-mode-cancel-designation" => tuple(SelectTaskMode(
            TaskMode::CancelDesignation(_)
        )) => published("orders-designation"),
        "ui-intent::task-mode-familiar-build" => tuple(SelectTaskMode(
            TaskMode::SelectBuildTarget
        )) => familiar_build_coverage(),
        "ui-intent::task-mode-area-selection" => tuple(SelectTaskMode(TaskMode::AreaSelection(_))) => {
            published("area-edit")
        },
        "ui-intent::task-mode-assign-task" => tuple(SelectTaskMode(TaskMode::AssignTask(_))) => {
            published("orders-designation")
        },
        "ui-intent::task-mode-zone-placement" => tuple(SelectTaskMode(TaskMode::ZonePlacement(_, _))) => {
            published("zones-workflow")
        },
        "ui-intent::task-mode-remove-stockpile" => tuple(SelectTaskMode(TaskMode::ZoneRemoval(
            TaskModeZoneType::Stockpile,
            _
        ))) => published("zones-workflow"),
        "ui-intent::task-mode-remove-yard" => tuple(SelectTaskMode(TaskMode::ZoneRemoval(
            TaskModeZoneType::Yard,
            _
        ))) => unreachable_player_flow(),
        "ui-intent::task-mode-floor-place" => tuple(SelectTaskMode(TaskMode::FloorPlace(_))) => {
            published("architect-building")
        },
        "ui-intent::task-mode-wall-place" => tuple(SelectTaskMode(TaskMode::WallPlace(_))) => {
            published("architect-building")
        },
        "ui-intent::task-mode-dream-planting" => tuple(SelectTaskMode(TaskMode::DreamPlanting(_))) => {
            published("dream-planting")
        },
        "ui-intent::task-mode-stockpile-policy" => tuple(SelectTaskMode(
            TaskMode::StockpilePolicyEdit(_)
        )) => published("zones-workflow"),
        "ui-intent::task-mode-soul-spa-place" => tuple(SelectTaskMode(TaskMode::SoulSpaPlace(_))) => {
            published("architect-building")
        },
        "ui-intent::select-area-task" => unit(SelectAreaTask) => published("orders-designation"),
        "ui-intent::select-dream-planting" => unit(SelectDreamPlanting) => {
            published("dream-planting")
        },
        "ui-intent::door-lock" => tuple(ToggleDoorLock(_)) => published("world-selection"),
        "ui-intent::operation-open" => record(OpenOperationDialog { .. }) => {
            published("familiar-operation-policy")
        },
        "ui-intent::operation-settings-dialog" => record(ApplyFamiliarSettings { .. }) => {
            published("familiar-operation-policy")
        },
        "ui-intent::operation-settings-explicit" => record(ApplyFamiliarSettingsFor { .. }) => {
            published("familiar-operation-policy")
        },
        "ui-intent::operation-close" => unit(CloseDialog) => {
            published("familiar-operation-policy")
        },
        "ui-intent::time-speed" => tuple(SetTimeSpeed(_)) => published("time-controls"),
        "ui-intent::time-pause-toggle" => unit(TogglePause) => published("time-controls"),
        "ui-intent::save" => unit(SaveGame) => published("save-load"),
        "ui-intent::load-request" => unit(RequestLoadGame) => published("save-load"),
        "ui-intent::load-confirm" => unit(ConfirmLoadGame) => published("save-load"),
        "ui-intent::load-cancel" => unit(CancelLoadConfirm) => published("save-load"),
        "ui-intent::architect-category" => tuple(SelectArchitectCategory(_)) => {
            published("architect-building")
        },
        "ui-intent::move-plant-building" => tuple(MovePlantBuilding(_)) => {
            published("architect-building")
        },
        "ui-intent::stockpile-policy" => record(ApplyStockpilePolicy { .. }) => {
            published("zones-workflow")
        },
        "ui-intent::stockpile-policy-range" => record(BeginStockpilePolicyRangeEdit { .. }) => {
            published("zones-workflow")
        },
        "ui-intent::task-priority" => record(AdjustTaskPriority { .. }) => {
            published("task-dashboard-actions")
        },
        "ui-intent::task-cancel" => record(CancelTask { .. }) => {
            published("task-dashboard-actions")
        }
    }
}

coverage_table! {
    enum FamiliarSettingsPatch;
    fn familiar_settings_patch_coverage(patch: FamiliarSettingsPatch);
    fn familiar_settings_patch_decisions();
    {
        "familiar-settings-patch::fatigue-threshold" => record(
            AdjustFatigueThreshold { .. }
        ) => published("familiar-operation-policy"),
        "familiar-settings-patch::max-controlled-soul" => record(
            AdjustMaxControlledSoul { .. }
        ) => published("familiar-operation-policy"),
        "familiar-settings-patch::work-allowed" => record(
            SetWorkAllowed { .. }
        ) => published("familiar-operation-policy"),
        "familiar-settings-patch::work-priority" => record(
            SetWorkPriority { .. }
        ) => published("familiar-operation-policy"),
        "familiar-settings-patch::all-work-allowed" => record(
            SetAllWorkAllowed { .. }
        ) => published("familiar-operation-policy")
    }
}

coverage_table! {
    enum FamiliarWorkPriority;
    fn familiar_work_priority_coverage(priority: FamiliarWorkPriority);
    fn familiar_work_priority_decisions();
    {
        "familiar-work-priority::low" => unit(Low) => {
            published("familiar-operation-policy")
        },
        "familiar-work-priority::normal" => unit(Normal) => {
            published("familiar-operation-policy")
        },
        "familiar-work-priority::high" => unit(High) => {
            published("familiar-operation-policy")
        }
    }
}

coverage_table! {
    enum HelpTopicStep;
    fn help_topic_step_coverage(step: HelpTopicStep);
    fn help_topic_step_decisions();
    {
        "help-topic-step::previous" => unit(Previous) => chrome(HelpChromeSlot::TopicPrevious),
        "help-topic-step::next" => unit(Next) => chrome(HelpChromeSlot::TopicNext)
    }
}

coverage_table! {
    enum HelpScrollCommand;
    fn help_scroll_command_coverage(command: HelpScrollCommand);
    fn help_scroll_command_decisions();
    {
        "help-scroll::page-up" => unit(PageUp) => chrome(HelpChromeSlot::PageUp),
        "help-scroll::page-down" => unit(PageDown) => chrome(HelpChromeSlot::PageDown),
        "help-scroll::start" => unit(Start) => chrome(HelpChromeSlot::DocumentStart),
        "help-scroll::end" => unit(End) => chrome(HelpChromeSlot::DocumentEnd)
    }
}

coverage_table! {
    enum MenuState;
    fn menu_state_coverage(state: MenuState);
    fn menu_state_decisions();
    {
        "menu-state::hidden" => unit(Hidden) => internal(),
        "menu-state::architect" => unit(Architect) => published("architect-building"),
        "menu-state::zones" => unit(Zones) => published("zones-workflow"),
        "menu-state::orders" => unit(Orders) => published("orders-designation"),
        "menu-state::dream" => unit(Dream) => published("dream-planting"),
        "menu-state::settings" => unit(Settings) => published("settings")
    }
}

coverage_table! {
    enum PlayMode;
    fn play_mode_coverage(mode: PlayMode);
    fn play_mode_decisions();
    {
        "play-mode::normal" => unit(Normal) => published("getting-started-work-loop"),
        "play-mode::building-place" => unit(BuildingPlace) => published("architect-building"),
        "play-mode::task-designation" => unit(TaskDesignation) => {
            published("orders-designation")
        },
        "play-mode::floor-place" => unit(FloorPlace) => published("architect-building"),
        "play-mode::building-move" => unit(BuildingMove) => published("architect-building")
    }
}

coverage_table! {
    enum BuildingType;
    fn building_type_coverage(kind: BuildingType);
    fn building_type_decisions();
    {
        "building-type::wall" => unit(Wall) => published("architect-building"),
        "building-type::door" => unit(Door) => published("architect-building"),
        "building-type::floor" => unit(Floor) => published("architect-building"),
        "building-type::tank" => unit(Tank) => published("architect-building"),
        "building-type::mud-mixer" => unit(MudMixer) => published("architect-building"),
        "building-type::rest-area" => unit(RestArea) => published("architect-building"),
        "building-type::bridge" => unit(Bridge) => published("architect-building"),
        "building-type::sand-pile" => unit(SandPile) => published("architect-building"),
        "building-type::bone-pile" => unit(BonePile) => published("architect-building"),
        "building-type::wheelbarrow-parking" => unit(WheelbarrowParking) => {
            published("architect-building")
        },
        "building-type::soul-spa" => unit(SoulSpa) => published("architect-building"),
        "building-type::outdoor-lamp" => unit(OutdoorLamp) => published("architect-building")
    }
}

coverage_table! {
    enum BuildingCategory;
    fn building_category_coverage(category: BuildingCategory);
    fn building_category_decisions();
    {
        "building-category::structure" => unit(Structure) => published("architect-building"),
        "building-category::architecture" => unit(Architecture) => {
            published("architect-building")
        },
        "building-category::plant" => unit(Plant) => published("architect-building"),
        "building-category::temporary" => unit(Temporary) => published("architect-building")
    }
}

coverage_table! {
    enum ResourceType;
    fn resource_type_coverage(resource: ResourceType);
    fn resource_type_decisions();
    {
        "resource-type::wood" => unit(Wood) => published("zones-workflow"),
        "resource-type::rock" => unit(Rock) => published("zones-workflow"),
        "resource-type::water" => unit(Water) => published("zones-workflow"),
        "resource-type::bucket-empty" => unit(BucketEmpty) => published("zones-workflow"),
        "resource-type::bucket-water" => unit(BucketWater) => published("zones-workflow"),
        "resource-type::sand" => unit(Sand) => published("zones-workflow"),
        "resource-type::bone" => unit(Bone) => published("zones-workflow"),
        "resource-type::stasis-mud" => unit(StasisMud) => published("zones-workflow"),
        "resource-type::wheelbarrow" => unit(Wheelbarrow) => published("zones-workflow")
    }
}

coverage_table! {
    enum WorkType;
    fn work_type_coverage(work: WorkType);
    fn work_type_decisions();
    {
        "work-type::chop" => unit(Chop) => published("task-dashboard-focus"),
        "work-type::mine" => unit(Mine) => published("task-dashboard-focus"),
        "work-type::build" => unit(Build) => published("task-dashboard-focus"),
        "work-type::move" => unit(Move) => published("task-dashboard-focus"),
        "work-type::haul" => unit(Haul) => published("task-dashboard-focus"),
        "work-type::haul-to-mixer" => unit(HaulToMixer) => published("task-dashboard-focus"),
        "work-type::gather-water" => unit(GatherWater) => published("task-dashboard-focus"),
        "work-type::collect-bone" => unit(CollectBone) => published("task-dashboard-focus"),
        "work-type::refine" => unit(Refine) => published("task-dashboard-focus"),
        "work-type::haul-water-to-mixer" => unit(HaulWaterToMixer) => {
            published("task-dashboard-focus")
        },
        "work-type::wheelbarrow-haul" => unit(WheelbarrowHaul) => {
            published("task-dashboard-focus")
        },
        "work-type::reinforce-floor-tile" => unit(ReinforceFloorTile) => {
            published("task-dashboard-focus")
        },
        "work-type::pour-floor-tile" => unit(PourFloorTile) => {
            published("task-dashboard-focus")
        },
        "work-type::frame-wall-tile" => unit(FrameWallTile) => {
            published("task-dashboard-focus")
        },
        "work-type::coat-wall" => unit(CoatWall) => published("task-dashboard-focus"),
        "work-type::generate-power" => unit(GeneratePower) => published("task-dashboard-focus")
    }
}

coverage_table! {
    enum TaskMode;
    fn task_mode_coverage(mode: TaskMode);
    fn task_mode_decisions();
    {
        "task-mode::none" => unit(None) => internal(),
        "task-mode::designate-chop" => tuple(DesignateChop(_)) => published("orders-designation"),
        "task-mode::designate-mine" => tuple(DesignateMine(_)) => published("orders-designation"),
        "task-mode::designate-haul" => tuple(DesignateHaul(_)) => published("orders-designation"),
        "task-mode::cancel-designation" => tuple(CancelDesignation(_)) => {
            published("orders-designation")
        },
        "task-mode::familiar-build" => unit(SelectBuildTarget) => {
            familiar_build_coverage()
        },
        "task-mode::area-selection" => tuple(AreaSelection(_)) => published("area-edit"),
        "task-mode::assign-task" => tuple(AssignTask(_)) => published("orders-designation"),
        "task-mode::zone-placement" => tuple(ZonePlacement(_, _)) => published("zones-workflow"),
        "task-mode::remove-stockpile" => tuple(ZoneRemoval(TaskModeZoneType::Stockpile, _)) => {
            published("zones-workflow")
        },
        "task-mode::remove-yard" => tuple(ZoneRemoval(TaskModeZoneType::Yard, _)) => {
            unreachable_player_flow()
        },
        "task-mode::floor-place" => tuple(FloorPlace(_)) => published("architect-building"),
        "task-mode::wall-place" => tuple(WallPlace(_)) => published("architect-building"),
        "task-mode::dream-planting" => tuple(DreamPlanting(_)) => published("dream-planting"),
        "task-mode::stockpile-policy" => tuple(StockpilePolicyEdit(_)) => {
            published("zones-workflow")
        },
        "task-mode::soul-spa-place" => tuple(SoulSpaPlace(_)) => published("architect-building")
    }
}

coverage_table! {
    enum TaskModeZoneType;
    fn task_mode_zone_type_coverage(kind: TaskModeZoneType);
    fn task_mode_zone_type_decisions();
    {
        "task-mode-zone-type::stockpile" => unit(Stockpile) => published("zones-workflow"),
        "task-mode-zone-type::yard" => unit(Yard) => published("zones-workflow")
    }
}

coverage_table! {
    enum TimeSpeed;
    fn time_speed_coverage(speed: TimeSpeed);
    fn time_speed_decisions();
    {
        "time-speed::paused" => unit(Paused) => published("time-controls"),
        "time-speed::normal" => unit(Normal) => published("time-controls"),
        "time-speed::fast" => unit(Fast) => published("time-controls"),
        "time-speed::super" => unit(Super) => published("time-controls")
    }
}

coverage_table! {
    enum ZoneType;
    fn zone_type_coverage(kind: ZoneType);
    fn zone_type_decisions();
    {
        "zone-type::stockpile" => unit(Stockpile) => published("zones-workflow"),
        "zone-type::yard" => unit(Yard) => published("zones-workflow")
    }
}

coverage_table! {
    enum StockpileAcceptance;
    fn stockpile_acceptance_coverage(acceptance: StockpileAcceptance);
    fn stockpile_acceptance_decisions();
    {
        "stockpile-acceptance::any" => unit(Any) => published("zones-workflow"),
        "stockpile-acceptance::only" => tuple(Only(_)) => published("zones-workflow"),
        "stockpile-acceptance::selected" => tuple(Selected(_)) => published("zones-workflow")
    }
}

coverage_table! {
    enum TransportPriority;
    fn transport_priority_coverage(priority: TransportPriority);
    fn transport_priority_decisions();
    {
        "transport-priority::low" => unit(Low) => published("zones-workflow"),
        "transport-priority::normal" => unit(Normal) => published("zones-workflow"),
        "transport-priority::high" => unit(High) => published("zones-workflow"),
        "transport-priority::critical" => unit(Critical) => published("zones-workflow")
    }
}

coverage_table! {
    enum TransportRequestKind;
    fn transport_request_kind_coverage(kind: TransportRequestKind);
    fn transport_request_kind_decisions();
    {
        "transport-request-kind::deposit-stockpile" => unit(DepositToStockpile) => internal(),
        "transport-request-kind::deliver-blueprint" => unit(DeliverToBlueprint) => internal(),
        "transport-request-kind::deliver-floor" => unit(DeliverToFloorConstruction) => internal(),
        "transport-request-kind::deliver-wall" => unit(DeliverToWallConstruction) => internal(),
        "transport-request-kind::deliver-provisional-wall" => unit(DeliverToProvisionalWall) => {
            internal()
        },
        "transport-request-kind::deliver-mixer-solid" => unit(DeliverToMixerSolid) => internal(),
        "transport-request-kind::deliver-water-mixer" => unit(DeliverWaterToMixer) => internal(),
        "transport-request-kind::gather-water-tank" => unit(GatherWaterToTank) => internal(),
        "transport-request-kind::return-bucket" => unit(ReturnBucket) => internal(),
        "transport-request-kind::return-wheelbarrow" => unit(ReturnWheelbarrow) => internal(),
        "transport-request-kind::batch-wheelbarrow" => unit(BatchWheelbarrow) => internal(),
        "transport-request-kind::consolidate-stockpile" => unit(ConsolidateStockpile) => internal(),
        "transport-request-kind::deliver-soul-spa" => unit(DeliverToSoulSpa) => internal()
    }
}

coverage_table! {
    enum TaskDashboardControl;
    fn task_dashboard_control_coverage(control: TaskDashboardControl);
    fn task_dashboard_control_decisions();
    {
        "task-dashboard-control::work-type" => unit(WorkTypeFilter) => {
            published("task-dashboard-filter-sort")
        },
        "task-dashboard-control::status" => unit(StatusFilter) => {
            published("task-dashboard-filter-sort")
        },
        "task-dashboard-control::priority" => unit(PriorityFilter) => {
            published("task-dashboard-filter-sort")
        },
        "task-dashboard-control::worker" => unit(WorkerFilter) => {
            published("task-dashboard-filter-sort")
        },
        "task-dashboard-control::sort-key" => unit(SortKey) => {
            published("task-dashboard-filter-sort")
        },
        "task-dashboard-control::sort-direction" => unit(SortDirection) => {
            published("task-dashboard-filter-sort")
        }
    }
}

coverage_table! {
    enum TaskWorkTypeFilter;
    fn task_work_type_filter_coverage(filter: TaskWorkTypeFilter);
    fn task_work_type_filter_decisions();
    {
        "task-work-type-filter::all" => unit(All) => published("task-dashboard-filter-sort"),
        "task-work-type-filter::only" => tuple(Only(_)) => published("task-dashboard-filter-sort")
    }
}

coverage_table! {
    enum TaskStatusFilter;
    fn task_status_filter_coverage(filter: TaskStatusFilter);
    fn task_status_filter_decisions();
    {
        "task-status-filter::all" => unit(All) => published("task-dashboard-filter-sort"),
        "task-status-filter::working" => unit(Working) => published("task-dashboard-filter-sort"),
        "task-status-filter::blocked" => unit(Blocked) => published("task-dashboard-filter-sort"),
        "task-status-filter::pending" => unit(Pending) => published("task-dashboard-filter-sort")
    }
}

coverage_table! {
    enum TaskPriorityFilter;
    fn task_priority_filter_coverage(filter: TaskPriorityFilter);
    fn task_priority_filter_decisions();
    {
        "task-priority-filter::all" => unit(All) => published("task-dashboard-filter-sort"),
        "task-priority-filter::normal" => unit(Normal) => published("task-dashboard-filter-sort"),
        "task-priority-filter::high" => unit(High) => published("task-dashboard-filter-sort"),
        "task-priority-filter::critical" => unit(Critical) => {
            published("task-dashboard-filter-sort")
        }
    }
}

coverage_table! {
    enum TaskWorkerFilter;
    fn task_worker_filter_coverage(filter: TaskWorkerFilter);
    fn task_worker_filter_decisions();
    {
        "task-worker-filter::all" => unit(All) => published("task-dashboard-filter-sort"),
        "task-worker-filter::assigned" => unit(Assigned) => {
            published("task-dashboard-filter-sort")
        },
        "task-worker-filter::unassigned" => unit(Unassigned) => {
            published("task-dashboard-filter-sort")
        }
    }
}

coverage_table! {
    enum TaskSortKey;
    fn task_sort_key_coverage(key: TaskSortKey);
    fn task_sort_key_decisions();
    {
        "task-sort-key::work-type" => unit(WorkType) => published("task-dashboard-filter-sort"),
        "task-sort-key::status" => unit(Status) => published("task-dashboard-filter-sort"),
        "task-sort-key::priority" => unit(Priority) => published("task-dashboard-filter-sort"),
        "task-sort-key::worker-count" => unit(WorkerCount) => {
            published("task-dashboard-filter-sort")
        }
    }
}

coverage_table! {
    enum TaskSortDirection;
    fn task_sort_direction_coverage(direction: TaskSortDirection);
    fn task_sort_direction_decisions();
    {
        "task-sort-direction::ascending" => unit(Ascending) => {
            published("task-dashboard-filter-sort")
        },
        "task-sort-direction::descending" => unit(Descending) => {
            published("task-dashboard-filter-sort")
        }
    }
}

/// Player-facing concepts whose display contract is not represented by a
/// project-owned input/state enum.
///
/// Every catalog entry must still be reachable from either one of the
/// exhaustive enum tables above or one of these explicit descriptive surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DescriptiveSurface {
    GettingStartedFirstSteps,
    CameraPanZoom,
    HelpPauseBehavior,
    SoulEnergyStatus,
    SoulEnergyRecovery,
    SoulAssignment,
    SoulRename,
    Notifications,
}

impl DescriptiveSurface {
    const ALL: [Self; 8] = [
        Self::GettingStartedFirstSteps,
        Self::CameraPanZoom,
        Self::HelpPauseBehavior,
        Self::SoulEnergyStatus,
        Self::SoulEnergyRecovery,
        Self::SoulAssignment,
        Self::SoulRename,
        Self::Notifications,
    ];

    const fn as_str(self) -> &'static str {
        match self {
            Self::GettingStartedFirstSteps => "descriptive::getting-started-first-steps",
            Self::CameraPanZoom => "descriptive::camera-pan-zoom",
            Self::HelpPauseBehavior => "descriptive::help-pause-behavior",
            Self::SoulEnergyStatus => "descriptive::soul-energy-status",
            Self::SoulEnergyRecovery => "descriptive::soul-energy-recovery",
            Self::SoulAssignment => "descriptive::soul-assignment",
            Self::SoulRename => "descriptive::soul-rename",
            Self::Notifications => "descriptive::notifications",
        }
    }
}

const fn descriptive_surface_coverage(surface: DescriptiveSurface) -> SurfaceCoverage {
    match surface {
        DescriptiveSurface::GettingStartedFirstSteps => published("getting-started-first-steps"),
        DescriptiveSurface::CameraPanZoom => published("camera-pan-zoom"),
        DescriptiveSurface::HelpPauseBehavior => published("help-pause-behavior"),
        DescriptiveSurface::SoulEnergyStatus => published("soul-energy-status"),
        DescriptiveSurface::SoulEnergyRecovery => published("soul-energy-recovery"),
        DescriptiveSurface::SoulAssignment => published("soul-assignment"),
        DescriptiveSurface::SoulRename => published("soul-rename"),
        DescriptiveSurface::Notifications => published("notifications"),
    }
}

fn descriptive_surface_decisions() -> Vec<CoverageRecord> {
    DescriptiveSurface::ALL
        .into_iter()
        .map(|surface| CoverageRecord {
            surface: surface.as_str(),
            decision: descriptive_surface_coverage(surface),
        })
        .collect()
}

pub(super) fn validate_surface_coverage(
    content: &HelpPanelContent,
) -> Result<(), HelpCatalogError> {
    let records = all_coverage_records();
    validate_unique_surface_ids(&records)?;

    let known_entries: BTreeSet<_> = content
        .topics()
        .flat_map(|topic| topic.entries())
        .map(|entry| entry.id())
        .collect();

    let mut published_entries = BTreeSet::new();
    let mut blocked_entries = BTreeSet::new();
    let mut published_chrome = BTreeSet::new();
    let mut launcher_published = false;

    for record in records {
        if record.decision.audience != HelpAudience::Player {
            continue;
        }
        match record.decision.coverage {
            HelpCoverage::Published(PublishedTarget::Entry(entry)) => {
                published_entries.insert(entry);
            }
            HelpCoverage::Published(PublishedTarget::Launcher) => {
                launcher_published = true;
            }
            HelpCoverage::Published(PublishedTarget::Chrome(slot)) => {
                published_chrome.insert(slot);
            }
            HelpCoverage::Blocked {
                target: BlockedTarget::Entry(entry),
                ..
            } => {
                blocked_entries.insert(entry);
            }
            HelpCoverage::Excluded(_) => {}
        }
    }

    if let Some(entry) = published_entries.intersection(&blocked_entries).next() {
        return Err(HelpCatalogError::new(format!(
            "Help entry is both published and blocked: {}",
            entry.as_str()
        )));
    }
    if let Some(entry) = known_entries.intersection(&blocked_entries).next() {
        return Err(HelpCatalogError::new(format!(
            "blocked surface is present in the Help catalog: {}",
            entry.as_str()
        )));
    }
    if known_entries != published_entries {
        let missing = published_entries
            .difference(&known_entries)
            .map(|entry| entry.as_str())
            .collect::<Vec<_>>();
        let unpublished = known_entries
            .difference(&published_entries)
            .map(|entry| entry.as_str())
            .collect::<Vec<_>>();
        return Err(HelpCatalogError::new(format!(
            "catalog entries and published surface targets differ; missing=[{}], unpublished=[{}]",
            missing.join(","),
            unpublished.join(",")
        )));
    }
    let expected_chrome: BTreeSet<_> = HelpChromeSlot::ALL.into_iter().collect();
    if published_chrome != expected_chrome {
        return Err(HelpCatalogError::new(
            "published Help chrome targets do not match HelpPanelChrome",
        ));
    }
    if !launcher_published {
        return Err(HelpCatalogError::new(
            "Help launcher has no published surface",
        ));
    }

    Ok(())
}

fn validate_unique_surface_ids(records: &[CoverageRecord]) -> Result<(), HelpCatalogError> {
    let mut seen = BTreeSet::new();
    if let Some(duplicate) = records
        .iter()
        .map(|record| record.surface)
        .find(|surface| !seen.insert(*surface))
    {
        return Err(HelpCatalogError::new(format!(
            "duplicate Help coverage surface id: {duplicate}"
        )));
    }
    Ok(())
}

fn all_coverage_records() -> Vec<CoverageRecord> {
    let mut decisions = input_action_decisions();
    decisions.extend(ui_intent_decisions());
    decisions.extend(familiar_settings_patch_decisions());
    decisions.extend(familiar_work_priority_decisions());
    decisions.extend(help_topic_step_decisions());
    decisions.extend(help_scroll_command_decisions());
    decisions.extend(menu_state_decisions());
    decisions.extend(play_mode_decisions());
    decisions.extend(building_type_decisions());
    decisions.extend(building_category_decisions());
    decisions.extend(resource_type_decisions());
    decisions.extend(work_type_decisions());
    decisions.extend(task_mode_decisions());
    decisions.extend(task_mode_zone_type_decisions());
    decisions.extend(time_speed_decisions());
    decisions.extend(zone_type_decisions());
    decisions.extend(stockpile_acceptance_decisions());
    decisions.extend(transport_priority_decisions());
    decisions.extend(transport_request_kind_decisions());
    decisions.extend(task_dashboard_control_decisions());
    decisions.extend(task_work_type_filter_decisions());
    decisions.extend(task_status_filter_decisions());
    decisions.extend(task_priority_filter_decisions());
    decisions.extend(task_worker_filter_decisions());
    decisions.extend(task_sort_key_decisions());
    decisions.extend(task_sort_direction_decisions());
    decisions.extend(descriptive_surface_decisions());
    decisions.push(CoverageRecord {
        surface: "dependency::default-camera-aliases",
        decision: dependency_default_camera_control(),
    });
    decisions
}

#[cfg(test)]
pub(super) fn normalized_approval_manifest() -> Vec<String> {
    let mut normalized = all_coverage_records()
        .into_iter()
        .map(|record| {
            let audience = match record.decision.audience {
                HelpAudience::Player => "player",
                HelpAudience::Internal => "internal",
                HelpAudience::Debug => "debug",
            };
            let coverage = match record.decision.coverage {
                HelpCoverage::Published(target) => {
                    format!("published:{}", normalized_target(target))
                }
                HelpCoverage::Excluded(reason) => format!(
                    "excluded:{}",
                    match reason {
                        HelpExclusionReason::InternalMechanism => "internal-mechanism",
                        HelpExclusionReason::DebugOnly => "debug-only",
                        HelpExclusionReason::DependencyDefaultNotProjectContract => {
                            "dependency-default"
                        }
                        HelpExclusionReason::UnreachablePlayerFlow => "unreachable-player-flow",
                    }
                ),
                HelpCoverage::Blocked {
                    target,
                    reason,
                    owner,
                } => format!(
                    "blocked:{}:{}:{}",
                    match target {
                        BlockedTarget::Entry(entry) => {
                            format!("entry:{}", entry.as_str())
                        }
                    },
                    match reason {
                        HelpBlockerReason::MissingCompletionConsumer => {
                            "missing-completion-consumer"
                        }
                    },
                    owner.as_str()
                ),
            };
            format!("coverage|{}|{}|{}", record.surface, audience, coverage)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}

#[cfg(test)]
fn normalized_target(target: PublishedTarget) -> String {
    match target {
        PublishedTarget::Entry(entry) => format!("entry:{}", entry.as_str()),
        PublishedTarget::Launcher => "launcher".to_string(),
        PublishedTarget::Chrome(slot) => format!("chrome:{}", slot.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Entity;

    #[test]
    fn familiar_build_binding_stays_unpublished_until_completion_exists() {
        assert_eq!(
            crate::input_actions::binding_labels_for_action(InputAction::FamiliarBuild),
            Ok(Vec::new())
        );
        assert_eq!(
            input_action_coverage(InputAction::FamiliarBuild),
            SurfaceCoverage {
                audience: HelpAudience::Player,
                coverage: HelpCoverage::Blocked {
                    target: BlockedTarget::Entry(HelpEntryId::new("familiar-build")),
                    reason: HelpBlockerReason::MissingCompletionConsumer,
                    owner: HelpOwnerId::FamiliarManagement,
                },
            }
        );
    }

    #[test]
    fn dependency_default_camera_aliases_are_explicitly_excluded() {
        let decision = dependency_default_camera_control();
        assert!(matches!(
            decision.coverage,
            HelpCoverage::Excluded(HelpExclusionReason::DependencyDefaultNotProjectContract)
        ));
    }

    #[test]
    fn representative_entity_payload_keeps_ui_intent_match_exhaustive() {
        assert_eq!(
            ui_intent_coverage(&UiIntent::InspectEntity(Entity::PLACEHOLDER)),
            published("info-panel-pin")
        );
    }

    #[test]
    fn task_dashboard_controls_and_filters_are_player_documented() {
        assert_eq!(
            task_dashboard_control_coverage(TaskDashboardControl::SortDirection),
            published("task-dashboard-filter-sort")
        );
        assert_eq!(
            task_work_type_filter_coverage(TaskWorkTypeFilter::Only(WorkType::Build)),
            published("task-dashboard-filter-sort")
        );
        assert_eq!(
            task_sort_key_coverage(TaskSortKey::WorkerCount),
            published("task-dashboard-filter-sort")
        );
    }

    #[test]
    fn duplicate_surface_ids_are_rejected() {
        let duplicate = CoverageRecord {
            surface: "fixture::duplicate",
            decision: internal(),
        };
        let error = validate_unique_surface_ids(&[duplicate, duplicate])
            .expect_err("duplicate stable IDs must fail validation");
        assert!(error.to_string().contains("fixture::duplicate"));
    }

    #[test]
    fn yard_removal_stays_excluded_until_a_player_completion_path_exists() {
        assert_eq!(
            ui_intent_coverage(&UiIntent::RemoveZone(ZoneType::Yard)),
            unreachable_player_flow()
        );
        assert_eq!(
            task_mode_coverage(TaskMode::ZoneRemoval(TaskModeZoneType::Yard, None)),
            unreachable_player_flow()
        );
    }
}
