use bevy::prelude::*;
use hw_core::constants::WHEELBARROW_CAPACITY;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};

use super::super::super::builders::WheelbarrowHaulSpec;
use super::{source_selector, wheelbarrow};
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub(super) struct ConstructionMudInput {
    pub site: Entity,
    pub site_pos: Vec2,
    pub remaining_needed: u32,
}

pub(super) fn select_construction_mud_haul(
    input: ConstructionMudInput,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    resource_grid: &hw_spatial::ResourceSpatialGrid,
) -> Option<WheelbarrowHaulSpec> {
    let max_items = input.remaining_needed.min(WHEELBARROW_CAPACITY as u32) as usize;
    let mut item_sources = source_selector::collect_nearby_items_for_wheelbarrow(
        ResourceType::StasisMud,
        input.site_pos,
        max_items,
        queries,
        shadow,
        resource_grid,
    );
    if item_sources.is_empty() {
        item_sources = source_selector::collect_items_for_wheelbarrow_unbounded(
            ResourceType::StasisMud,
            input.site_pos,
            max_items,
            queries,
            shadow,
            resource_grid,
        );
    }
    if item_sources.is_empty() {
        return None;
    }

    let source_pos = item_sources
        .iter()
        .map(|(_, pos)| *pos)
        .reduce(|left, right| left + right)
        .expect("item_sources is non-empty: checked above")
        / item_sources.len() as f32;
    let wheelbarrow = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow)?;

    Some(WheelbarrowHaulSpec {
        wheelbarrow,
        source_pos,
        destination: WheelbarrowDestination::Stockpile(input.site),
        items: item_sources.into_iter().map(|(entity, _)| entity).collect(),
    })
}
