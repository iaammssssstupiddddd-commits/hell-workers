use super::*;

mod inventory_containers;
mod ownership;
mod rest;
mod transport;
mod wheelbarrow;

#[derive(Default)]
struct LogisticsIndex {
    inventory_owners: HashSet<Entity>,
    loaded_sources: HashMap<Entity, HashSet<Entity>>,
    stored_sources: HashMap<Entity, HashSet<Entity>>,
    mixer_sources: HashMap<Entity, HashSet<Entity>>,
}

pub(super) fn validate(candidate: &World) -> Result<(), String> {
    ownership::validate_durable_owner_links(candidate)?;
    let navigation = DurableNavigationView::from_world(candidate)?;
    let mut index = LogisticsIndex::default();

    inventory_containers::validate_inventories(
        candidate,
        &navigation,
        &mut index.inventory_owners,
    )?;
    inventory_containers::validate_resource_items(candidate, &navigation, &mut index)?;
    inventory_containers::validate_loaded_capacity(candidate, &index.loaded_sources)?;
    inventory_containers::validate_mixers(candidate, &index.mixer_sources)?;
    inventory_containers::validate_stockpiles(candidate, &index.stored_sources)?;
    wheelbarrow::validate(
        candidate,
        &navigation,
        &index.inventory_owners,
        &index.loaded_sources,
    )?;
    ownership::validate_managed_by(candidate)?;
    ownership::validate_managed_tasks(candidate)?;
    transport::validate(candidate)?;
    rest::validate(candidate)?;
    Ok(())
}
