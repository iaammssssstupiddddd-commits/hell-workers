mod builders;

use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::panels::InfoPanelPinState;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::ESCAPE_STRESS_THRESHOLD;
use hw_core::relationships::CommandedBy;
use hw_core::relationships::{IncomingDeliveries, StoredItems, TaskWorkers};
use hw_energy::{
    ConsumesFrom, GeneratesFor, PowerConsumer, PowerConsumerPolicy, PowerGenerator, PowerGrid,
    PowerGridAllocationSummary, PowerSupplyState, SoulSpaSite, SoulSpaTile, Unpowered,
};
use hw_soul_ai::soul_ai::perceive::escaping::is_escape_threat_close;
use hw_spatial::FamiliarSpatialGrid;
use hw_ui::components::TooltipTemplate;

pub use hw_ui::models::inspection::{
    EntityInspectionModel, EntityInspectionViewModel, PowerInspectionFields, SoulInspectionFields,
    SoulSpaInspectionFields, StockpileInspectionFields,
};

type SoulInspectionQuery<'w, 's> = Query<
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
>;

type DesignationInspectionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::systems::jobs::Designation,
        Option<&'static crate::systems::jobs::IssuedBy>,
        Option<&'static TaskWorkers>,
    ),
>;

type BuildingInspectionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::systems::jobs::Building,
        Option<&'static crate::systems::jobs::ProvisionalWall>,
        Option<&'static crate::systems::logistics::Stockpile>,
        Option<&'static hw_core::relationships::StoredItems>,
        Option<&'static crate::systems::jobs::MudMixerStorage>,
        Option<&'static crate::systems::jobs::RestArea>,
        Option<&'static hw_core::relationships::RestAreaOccupants>,
    ),
>;

type StockpileInspectionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::systems::logistics::Stockpile,
        &'static hw_logistics::StockpilePolicy,
        Option<&'static StoredItems>,
        Option<&'static IncomingDeliveries>,
    ),
>;

type PowerConsumerInspectionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static PowerConsumer,
        Option<&'static PowerConsumerPolicy>,
        Option<&'static PowerSupplyState>,
        Option<&'static ConsumesFrom>,
        Option<&'static Unpowered>,
        Option<&'static Transform>,
    ),
>;

#[derive(SystemParam)]
pub struct EntityInspectionQuery<'w, 's> {
    q_souls: SoulInspectionQuery<'w, 's>,
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
    q_designations: DesignationInspectionQuery<'w, 's>,
    q_buildings: BuildingInspectionQuery<'w, 's>,
    q_stockpiles: StockpileInspectionQuery<'w, 's>,
    pub(super) q_power_consumers: PowerConsumerInspectionQuery<'w, 's>,
    pub(super) q_power_grids: Query<
        'w,
        's,
        (
            &'static PowerGrid,
            Option<&'static PowerGridAllocationSummary>,
        ),
    >,
    pub(super) q_soul_spas: Query<
        'w,
        's,
        (
            &'static SoulSpaSite,
            &'static PowerGenerator,
            Option<&'static GeneratesFor>,
        ),
    >,
    pub(super) q_soul_spa_tiles:
        Query<'w, 's, (&'static SoulSpaTile, Option<&'static TaskWorkers>)>,
}

#[derive(Default)]
struct InspectionAccumulator {
    header: String,
    common_lines: Vec<String>,
    tooltip_lines: Vec<String>,
    soul_fields: Option<SoulInspectionFields>,
    stockpile_fields: Option<StockpileInspectionFields>,
    soul_spa_fields: Option<SoulSpaInspectionFields>,
    power_fields: Option<PowerInspectionFields>,
}

impl InspectionAccumulator {
    fn push_common(&mut self, line: impl Into<String>) {
        self.common_lines.push(line.into());
    }

    fn push_tooltip(&mut self, line: impl Into<String>) {
        self.tooltip_lines.push(line.into());
    }

    fn finalize(mut self, entity: Entity) -> Option<EntityInspectionModel> {
        if self.header.is_empty() && self.tooltip_lines.is_empty() {
            return None;
        }

        if self.tooltip_lines.is_empty() {
            self.tooltip_lines.push(self.header.clone());
        }

        Some(EntityInspectionModel {
            entity,
            header: self.header,
            common_text: self.common_lines.join("\n"),
            tooltip_lines: self.tooltip_lines,
            soul: self.soul_fields,
            stockpile: self.stockpile_fields,
            soul_spa: self.soul_spa_fields,
            power: self.power_fields,
        })
    }
}

pub fn update_entity_inspection_view_model_system(
    selected_entity: Res<SelectedEntity>,
    mut pin_state: ResMut<InfoPanelPinState>,
    inspection: EntityInspectionQuery,
    mut view_model: ResMut<EntityInspectionViewModel>,
) {
    let mut inspected_entity = pin_state.entity.or(selected_entity.0);
    let mut model = inspected_entity.and_then(|entity| inspection.build_model(entity));

    if pin_state.entity.is_some() && model.is_none() {
        pin_state.entity = None;
        inspected_entity = selected_entity.0;
        model = inspected_entity.and_then(|entity| inspection.build_model(entity));
    }

    let _ = inspected_entity;
    view_model.model = model;
}

impl EntityInspectionQuery<'_, '_> {
    pub fn build_model(&self, entity: Entity) -> Option<EntityInspectionModel> {
        let mut model = InspectionAccumulator::default();

        let _ = self.build_soul_model(entity, &mut model)
            || self.build_blueprint_model(entity, &mut model)
            || self.build_familiar_model(entity, &mut model)
            || self.build_item_model(entity, &mut model)
            || self.build_tree_model(entity, &mut model)
            || self.build_rock_model(entity, &mut model)
            || self.build_stockpile_model(entity, &mut model);

        self.append_soul_spa_model(entity, &mut model);
        self.append_building_model(entity, &mut model);
        self.append_power_consumer_model(entity, &mut model);
        self.append_designation_model(entity, &mut model);

        model.finalize(entity)
    }

    pub fn classify_template(&self, entity: Entity) -> TooltipTemplate {
        if self.q_souls.get(entity).is_ok() {
            TooltipTemplate::Soul
        } else if self.q_buildings.get(entity).is_ok()
            || self.q_blueprints.get(entity).is_ok()
            || self.q_stockpiles.get(entity).is_ok()
        {
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
    if let Some(data) = task.bucket_transport_data() {
        return format!("BucketTransport ({:?})", data.phase);
    }

    match task {
        AssignedTask::None => "Idle".to_string(),
        AssignedTask::Gather(data) => format!("Gather ({:?})", data.phase),
        AssignedTask::Haul(data) => format!("Haul ({:?})", data.phase),
        AssignedTask::HaulToBlueprint(data) => format!("HaulToBp ({:?})", data.phase),
        AssignedTask::Build(data) => format!("Build ({:?})", data.phase),
        AssignedTask::MovePlant(data) => format!("MovePlant ({:?})", data.phase),
        AssignedTask::CollectBone(data) => format!("CollectBone ({:?})", data.phase),
        AssignedTask::Refine(data) => format!("Refine ({:?})", data.phase),
        AssignedTask::HaulToMixer(data) => format!("HaulToMixer ({:?})", data.phase),
        AssignedTask::HaulWithWheelbarrow(data) => format!("HaulWheelbarrow ({:?})", data.phase),
        AssignedTask::ReinforceFloorTile(data) => format!("ReinforceFloor ({:?})", data.phase),
        AssignedTask::PourFloorTile(data) => format!("PourFloor ({:?})", data.phase),
        AssignedTask::FrameWallTile(data) => format!("FrameWall ({:?})", data.phase),
        AssignedTask::CoatWall(data) => format!("CoatWall ({:?})", data.phase),
        AssignedTask::GeneratePower(data) => format!("GeneratePower ({:?})", data.phase),
        _ => "BucketTransport".to_string(),
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
    let mut scratch = Vec::new();
    let escape_threat_close = is_escape_threat_close(
        transform.translation.truncate(),
        familiar_grid,
        q_familiars_escape,
        &mut scratch,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;
    use hw_core::relationships::{DeliveringTo, StoredIn, WorkingOn};
    use hw_logistics::transport_request::TransportPriority;
    use hw_logistics::{
        ResourceType, Stockpile, StockpileAcceptance, StockpilePolicy, StockpilePolicyState,
    };

    #[derive(Resource)]
    struct InspectionTarget(Entity);

    #[derive(Resource, Default)]
    struct InspectionReceipt(Option<EntityInspectionModel>);

    fn inspect(
        target: Res<InspectionTarget>,
        inspection: EntityInspectionQuery,
        mut receipt: ResMut<InspectionReceipt>,
    ) {
        receipt.0 = inspection.build_model(target.0);
    }

    #[test]
    fn stockpile_policy_inspection_reports_live_counts_and_draining_state() {
        let mut app = minimal_app();
        app.init_resource::<FamiliarSpatialGrid>()
            .init_resource::<InspectionReceipt>()
            .add_systems(Update, inspect);
        let acceptance = StockpileAcceptance::none()
            .with_resource(ResourceType::BucketEmpty, true)
            .with_resource(ResourceType::StasisMud, true);
        let stockpile = app
            .world_mut()
            .spawn((
                Stockpile {
                    capacity: 10,
                    resource_type: Some(ResourceType::Bone),
                },
                StockpilePolicy {
                    acceptance,
                    inbound_priority: TransportPriority::High,
                    target_amount: 8,
                    allow_export: false,
                },
            ))
            .id();
        for _ in 0..3 {
            app.world_mut().spawn(StoredIn(stockpile));
        }
        app.world_mut().spawn(DeliveringTo(stockpile));
        app.insert_resource(InspectionTarget(stockpile));

        app.update();

        let model = app
            .world()
            .resource::<InspectionReceipt>()
            .0
            .as_ref()
            .expect("managed stockpile must be inspectable");
        let fields = model
            .stockpile
            .as_ref()
            .expect("managed stockpile must expose an editor model");
        assert_eq!(fields.state, StockpilePolicyState::Draining);
        assert_eq!(fields.current_amount, 3);
        assert_eq!(fields.incoming_amount, 1);
        assert_eq!(fields.target_amount, 8);
        assert_eq!(fields.acceptance, acceptance);
        assert_eq!(fields.inbound_priority, TransportPriority::High);
        assert!(!fields.allow_export);
        assert!(
            model
                .tooltip_lines
                .iter()
                .any(|line| line.contains("draining override active"))
        );
        assert!(
            model
                .tooltip_lines
                .iter()
                .any(|line| line == "Acceptance: Empty Bucket, Stasis Mud")
        );
    }

    #[test]
    fn special_storage_does_not_expose_the_stockpile_policy_editor() {
        let mut app = minimal_app();
        app.init_resource::<FamiliarSpatialGrid>()
            .init_resource::<InspectionReceipt>()
            .add_systems(Update, inspect);
        let tank = app
            .world_mut()
            .spawn((
                crate::systems::jobs::Building {
                    kind: crate::systems::jobs::BuildingType::Tank,
                    is_provisional: false,
                },
                Stockpile {
                    capacity: 20,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        app.insert_resource(InspectionTarget(tank));

        app.update();

        let model = app
            .world()
            .resource::<InspectionReceipt>()
            .0
            .as_ref()
            .expect("building storage remains inspectable");
        assert!(model.stockpile.is_none());
    }

    #[test]
    fn soul_spa_inspection_counts_parent_site_workers_and_reports_draining() {
        use hw_energy::{PowerGenerator, SoulSpaPhase, SoulSpaSite, SoulSpaTile};

        let mut app = minimal_app();
        app.init_resource::<FamiliarSpatialGrid>()
            .init_resource::<InspectionReceipt>()
            .add_systems(Update, inspect);
        let site = app
            .world_mut()
            .spawn((
                SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    active_slots: 2,
                    ..default()
                },
                PowerGenerator {
                    current_output: 4.0,
                    ..default()
                },
            ))
            .id();
        for x in 0..4 {
            let tile = app
                .world_mut()
                .spawn(SoulSpaTile {
                    parent_site: site,
                    grid_pos: (x, 0),
                })
                .id();
            app.world_mut().spawn(WorkingOn(tile));
        }
        app.insert_resource(InspectionTarget(site));

        app.update();

        let model = app
            .world()
            .resource::<InspectionReceipt>()
            .0
            .as_ref()
            .expect("Soul Spa must be inspectable");
        let fields = model
            .soul_spa
            .as_ref()
            .expect("Soul Spa must expose slot controls");
        assert!(fields.operational);
        assert_eq!(fields.occupied_slots, 4);
        assert_eq!(fields.active_slots, 2);
        assert_eq!(fields.output_watts, 4.0);
        assert!(
            model
                .tooltip_lines
                .iter()
                .any(|line| line == "Draining (4 active / 2 configured)")
        );
        assert!(app.world().get::<Children>(site).is_none());
    }

    #[test]
    fn constructing_soul_spa_exposes_progress_without_operational_power_controls() {
        use hw_energy::SoulSpaSite;

        let mut app = minimal_app();
        app.init_resource::<FamiliarSpatialGrid>()
            .init_resource::<InspectionReceipt>()
            .add_systems(Update, inspect);
        let site = app
            .world_mut()
            .spawn(SoulSpaSite {
                bones_delivered: 7,
                bones_required: 20,
                ..default()
            })
            .id();
        app.insert_resource(InspectionTarget(site));

        app.update();

        let model = app
            .world()
            .resource::<InspectionReceipt>()
            .0
            .as_ref()
            .expect("constructing Soul Spa must be inspectable");
        let fields = model.soul_spa.as_ref().expect("typed Soul Spa progress");
        assert!(!fields.operational);
        assert_eq!(fields.bones_delivered, 7);
        assert_eq!(fields.bones_required, 20);
        assert!(model.power.is_none());
        assert!(
            model
                .tooltip_lines
                .iter()
                .any(|line| line == "Status: Constructing (7/20)")
        );
    }

    #[test]
    fn power_consumer_inspection_exposes_policy_connection_and_shed_reason() {
        use hw_energy::{
            ConsumesFrom, PowerAllocationMode, PowerConsumer, PowerConsumerPolicy, PowerGrid,
            PowerGridAllocationSummary, PowerPriority, PowerShedReason, PowerSupplyState,
        };
        use hw_ui::power::{PowerPriorityValue, PowerShedReasonValue, PowerSupplyStateValue};

        let mut app = minimal_app();
        app.init_resource::<FamiliarSpatialGrid>()
            .init_resource::<InspectionReceipt>()
            .add_systems(Update, inspect);
        let grid = app
            .world_mut()
            .spawn((
                PowerGrid {
                    generation: 1.0,
                    consumption: 1.5,
                    powered: false,
                },
                PowerGridAllocationSummary {
                    mode: PowerAllocationMode::PriorityPrefix,
                    generation: 1.0,
                    total_demand: 1.5,
                    served_demand: 1.0,
                    consumer_count: 2,
                    supplied_count: 1,
                    shed_count: 1,
                    invalid_count: 0,
                    shed_order: Vec::new(),
                },
            ))
            .id();
        let consumer = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.5 },
                PowerConsumerPolicy {
                    priority: PowerPriority::Low,
                },
                PowerSupplyState::Shed {
                    reason: PowerShedReason::RestoreMargin,
                },
                ConsumesFrom(grid),
                Transform::from_translation(hw_world::WorldMap::grid_to_world(2, 1).extend(0.0)),
            ))
            .id();
        app.world_mut()
            .get_mut::<PowerGridAllocationSummary>(grid)
            .unwrap()
            .shed_order = vec![consumer];
        app.insert_resource(InspectionTarget(consumer));

        app.update();

        let model = app
            .world()
            .resource::<InspectionReceipt>()
            .0
            .as_ref()
            .expect("power consumer must be inspectable");
        let power = model.power.as_ref().expect("typed power inspection");
        assert_eq!(power.grid, Some(grid));
        assert_eq!(power.priority, Some(PowerPriorityValue::Low));
        assert_eq!(
            power.supply_state,
            Some(PowerSupplyStateValue::Shed {
                reason: PowerShedReasonValue::RestoreMargin,
            })
        );
        assert_eq!(power.served_demand_watts, Some(1.0));
        assert_eq!(power.deficit_watts, Some(0.5));
        assert_eq!(power.consumer_count, Some(2));
        assert_eq!(power.shed_order_labels, vec!["(2, 1)"]);
        assert!(
            model
                .tooltip_lines
                .iter()
                .any(|line| line == "Supply: Shed: waiting for restore margin")
        );
    }
}
