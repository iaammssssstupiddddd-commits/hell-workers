use crate::models::inspection::{
    EntityInspectionModel, InspectionSoulGender, StockpileInspectionFields,
};
use bevy::prelude::*;
use hw_logistics::transport_request::TransportPriority;
use hw_logistics::{ResourceType, StockpileAcceptance, StockpilePolicyState};

#[derive(Clone, PartialEq)]
pub(super) enum InfoPanelViewModel {
    Soul(SoulInfoViewModel),
    Stockpile(StockpileInfoViewModel),
    Simple(SimpleInfoViewModel),
}

#[derive(Clone, PartialEq)]
pub(super) struct SoulInfoViewModel {
    pub(super) entity: Entity,
    pub(super) header: String,
    pub(super) gender: Option<InspectionSoulGender>,
    pub(super) motivation: String,
    pub(super) stress: String,
    pub(super) fatigue: String,
    pub(super) dream: String,
    pub(super) task: String,
    pub(super) inventory: String,
    pub(super) common: String,
}

#[derive(Clone, PartialEq)]
pub(super) struct SimpleInfoViewModel {
    pub(super) header: String,
    pub(super) common: String,
}

#[derive(Clone, PartialEq)]
pub(super) struct StockpileInfoViewModel {
    pub(super) entity: Entity,
    pub(super) header: String,
    pub(super) state: StockpilePolicyState,
    pub(super) current_amount: usize,
    pub(super) incoming_amount: usize,
    pub(super) capacity: usize,
    pub(super) current_resource: Option<ResourceType>,
    pub(super) acceptance: StockpileAcceptance,
    pub(super) inbound_priority: TransportPriority,
    pub(super) target_amount: usize,
    pub(super) allow_export: bool,
    pub(super) common: String,
}

pub(super) const fn next_stockpile_acceptance(current: StockpileAcceptance) -> StockpileAcceptance {
    match current {
        StockpileAcceptance::Any => StockpileAcceptance::Only(ResourceType::Wood),
        StockpileAcceptance::Only(ResourceType::Wood) => {
            StockpileAcceptance::Only(ResourceType::Rock)
        }
        StockpileAcceptance::Only(ResourceType::Rock) => {
            StockpileAcceptance::Only(ResourceType::Water)
        }
        StockpileAcceptance::Only(ResourceType::Water) => {
            StockpileAcceptance::Only(ResourceType::BucketEmpty)
        }
        StockpileAcceptance::Only(ResourceType::BucketEmpty) => {
            StockpileAcceptance::Only(ResourceType::BucketWater)
        }
        StockpileAcceptance::Only(ResourceType::BucketWater) => {
            StockpileAcceptance::Only(ResourceType::Sand)
        }
        StockpileAcceptance::Only(ResourceType::Sand) => {
            StockpileAcceptance::Only(ResourceType::Bone)
        }
        StockpileAcceptance::Only(ResourceType::Bone) => {
            StockpileAcceptance::Only(ResourceType::StasisMud)
        }
        StockpileAcceptance::Only(ResourceType::StasisMud) => {
            StockpileAcceptance::Only(ResourceType::Wheelbarrow)
        }
        StockpileAcceptance::Only(ResourceType::Wheelbarrow) => StockpileAcceptance::Any,
    }
}

pub(super) const fn next_stockpile_priority(current: TransportPriority) -> TransportPriority {
    match current {
        TransportPriority::Low => TransportPriority::Normal,
        TransportPriority::Normal => TransportPriority::High,
        TransportPriority::High => TransportPriority::Critical,
        TransportPriority::Critical => TransportPriority::Low,
    }
}

pub(super) fn to_view_model(model: EntityInspectionModel) -> InfoPanelViewModel {
    if let Some(stockpile) = model.stockpile {
        InfoPanelViewModel::Stockpile(stockpile_view_model(
            model.entity,
            model.header,
            model.common_text,
            stockpile,
        ))
    } else if let Some(soul) = model.soul {
        InfoPanelViewModel::Soul(SoulInfoViewModel {
            entity: model.entity,
            header: model.header,
            gender: soul.gender,
            motivation: soul.motivation,
            stress: soul.stress,
            fatigue: soul.fatigue,
            dream: soul.dream,
            task: soul.task,
            inventory: soul.inventory,
            common: soul.common,
        })
    } else {
        InfoPanelViewModel::Simple(SimpleInfoViewModel {
            header: model.header,
            common: model.common_text,
        })
    }
}

fn stockpile_view_model(
    entity: Entity,
    header: String,
    common: String,
    fields: StockpileInspectionFields,
) -> StockpileInfoViewModel {
    StockpileInfoViewModel {
        entity,
        header,
        state: fields.state,
        current_amount: fields.current_amount,
        incoming_amount: fields.incoming_amount,
        capacity: fields.capacity,
        current_resource: fields.current_resource,
        acceptance: fields.acceptance,
        inbound_priority: fields.inbound_priority,
        target_amount: fields.target_amount,
        allow_export: fields.allow_export,
        common,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stockpile_acceptance_cycle_covers_every_resource_and_returns_to_any() {
        let mut value = StockpileAcceptance::Any;
        let mut visited = Vec::new();
        for _ in 0..10 {
            value = next_stockpile_acceptance(value);
            visited.push(value);
        }

        assert_eq!(
            visited,
            vec![
                StockpileAcceptance::Only(ResourceType::Wood),
                StockpileAcceptance::Only(ResourceType::Rock),
                StockpileAcceptance::Only(ResourceType::Water),
                StockpileAcceptance::Only(ResourceType::BucketEmpty),
                StockpileAcceptance::Only(ResourceType::BucketWater),
                StockpileAcceptance::Only(ResourceType::Sand),
                StockpileAcceptance::Only(ResourceType::Bone),
                StockpileAcceptance::Only(ResourceType::StasisMud),
                StockpileAcceptance::Only(ResourceType::Wheelbarrow),
                StockpileAcceptance::Any,
            ]
        );
    }

    #[test]
    fn stockpile_inspection_maps_to_an_entity_bound_editor_model() {
        let entity = Entity::from_raw_u32(7).expect("valid entity");
        let view = to_view_model(EntityInspectionModel {
            entity,
            header: "Stockpile".to_string(),
            common_text: "Managed".to_string(),
            tooltip_lines: Vec::new(),
            soul: None,
            stockpile: Some(StockpileInspectionFields {
                state: StockpilePolicyState::Draining,
                current_amount: 4,
                incoming_amount: 1,
                capacity: 10,
                current_resource: Some(ResourceType::Bone),
                acceptance: StockpileAcceptance::Only(ResourceType::Wood),
                inbound_priority: TransportPriority::Critical,
                target_amount: 6,
                allow_export: false,
            }),
        });

        let InfoPanelViewModel::Stockpile(stockpile) = view else {
            panic!("expected stockpile view model");
        };
        assert_eq!(stockpile.entity, entity);
        assert_eq!(stockpile.state, StockpilePolicyState::Draining);
        assert_eq!(stockpile.current_amount, 4);
        assert_eq!(stockpile.incoming_amount, 1);
        assert_eq!(
            stockpile.acceptance,
            StockpileAcceptance::Only(ResourceType::Wood)
        );
        assert_eq!(stockpile.inbound_priority, TransportPriority::Critical);
        assert!(!stockpile.allow_export);
    }

    #[test]
    fn stockpile_priority_cycle_is_exhaustive() {
        assert_eq!(
            next_stockpile_priority(TransportPriority::Low),
            TransportPriority::Normal
        );
        assert_eq!(
            next_stockpile_priority(TransportPriority::Normal),
            TransportPriority::High
        );
        assert_eq!(
            next_stockpile_priority(TransportPriority::High),
            TransportPriority::Critical
        );
        assert_eq!(
            next_stockpile_priority(TransportPriority::Critical),
            TransportPriority::Low
        );
    }
}
