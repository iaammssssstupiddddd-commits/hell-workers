use super::{
    super::*, LogisticsIndex, ownership::is_bucket_storage_role,
    wheelbarrow::validate_wheelbarrow_home,
};

pub(super) fn validate_inventories(
    candidate: &World,
    navigation: &DurableNavigationView<'_>,
    inventory_owners: &mut HashSet<Entity>,
) -> Result<(), String> {
    for owner in candidate.iter_entities() {
        let Some(inventory) = owner.get::<Inventory>() else {
            continue;
        };
        let Some(held) = inventory.0 else {
            continue;
        };
        if !owner.contains::<DamnedSoul>() {
            return Err(format!(
                "Inventory(Some) owner {:?} is not a DamnedSoul",
                owner.id()
            ));
        }
        if !inventory_owners.insert(held) {
            return Err(format!(
                "entity {held:?} is referenced by more than one Soul inventory"
            ));
        }
        let held_ref = candidate
            .get_entity(held)
            .map_err(|_| format!("Soul inventory references missing entity {held:?}"))?;
        if !held_ref.contains::<ResourceItem>() && !held_ref.contains::<Wheelbarrow>() {
            return Err(format!(
                "Soul inventory entity {held:?} is neither a ResourceItem nor a Wheelbarrow"
            ));
        }
        let transform = owner.get::<Transform>().ok_or_else(|| {
            format!(
                "Soul {:?} with Inventory(Some) has no Transform",
                owner.id()
            )
        })?;
        if navigation
            .nearest_walkable_position(transform.translation.truncate())
            .is_none()
        {
            return Err(format!(
                "Soul {:?} has no walkable inventory drop cell",
                owner.id()
            ));
        }
        if held_ref.contains::<ResourceItem>()
            && (held_ref.contains::<LoadedIn>()
                || held_ref.contains::<StoredIn>()
                || held_ref.contains::<StoredByMixer>())
        {
            return Err(format!(
                "ResourceItem {held:?} has both a Soul inventory owner and a durable container owner"
            ));
        }
        if held_ref.contains::<Wheelbarrow>() {
            validate_wheelbarrow_home(candidate, held, &held_ref)?;
        }
    }
    Ok(())
}

pub(super) fn validate_resource_items(
    candidate: &World,
    navigation: &DurableNavigationView<'_>,
    index: &mut LogisticsIndex,
) -> Result<(), String> {
    let LogisticsIndex {
        inventory_owners,
        loaded_sources,
        stored_sources,
        mixer_sources,
    } = index;
    for item in candidate.iter_entities() {
        let loaded_in = item.get::<LoadedIn>();
        let stored_in = item.get::<StoredIn>();
        let stored_by_mixer = item.get::<StoredByMixer>();
        let has_container_owner =
            loaded_in.is_some() || stored_in.is_some() || stored_by_mixer.is_some();
        if !item.contains::<ResourceItem>() && has_container_owner {
            return Err(format!(
                "durable item owner relation source {:?} is not a ResourceItem",
                item.id()
            ));
        }
        if !item.contains::<ResourceItem>() {
            continue;
        }
        let resource = item.get::<ResourceItem>().expect("filtered ResourceItem").0;
        if (resource == hw_core::logistics::ResourceType::Wheelbarrow)
            != item.contains::<Wheelbarrow>()
        {
            return Err(format!(
                "ResourceItem {:?} has an invalid Wheelbarrow marker/type shape",
                item.id()
            ));
        }
        let owner_count = usize::from(inventory_owners.contains(&item.id()))
            + usize::from(loaded_in.is_some())
            + usize::from(stored_in.is_some())
            + usize::from(stored_by_mixer.is_some());
        if owner_count > 1 {
            return Err(format!(
                "ResourceItem {:?} has more than one inventory/container owner",
                item.id()
            ));
        }
        if item.contains::<Wheelbarrow>() && has_container_owner {
            return Err(format!(
                "Wheelbarrow {:?} cannot have a durable container owner",
                item.id()
            ));
        }

        // LoadedIn/LoadedItems survive staging only long enough to carry the
        // remapped wheelbarrow location into RuntimeNormalize. Other items are
        // already ground cargo and need their own final-topology safe cell.
        if !item.contains::<Wheelbarrow>()
            && loaded_in.is_none()
            && stored_in.is_none()
            && stored_by_mixer.is_none()
            && !inventory_owners.contains(&item.id())
        {
            let transform = item
                .get::<Transform>()
                .ok_or_else(|| format!("ground ResourceItem {:?} has no Transform", item.id()))?;
            if navigation
                .nearest_walkable_position(transform.translation.truncate())
                .is_none()
            {
                return Err(format!(
                    "ResourceItem {:?} has no walkable ground normalization cell",
                    item.id()
                ));
            }
        }

        if let Some(loaded_in) = loaded_in {
            if !resource.is_loadable() {
                return Err(format!(
                    "LoadedIn source {:?} has non-loadable resource type {:?}",
                    item.id(),
                    resource
                ));
            }
            let carrier = candidate
                .get_entity(loaded_in.0)
                .map_err(|_| format!("LoadedIn references missing carrier {:?}", loaded_in.0))?;
            if !carrier.contains::<Wheelbarrow>() {
                return Err(format!(
                    "LoadedIn carrier {:?} is not a Wheelbarrow",
                    loaded_in.0
                ));
            }
            loaded_sources
                .entry(loaded_in.0)
                .or_default()
                .insert(item.id());
        }
        if let Some(stored_in) = stored_in {
            let storage = candidate
                .get_entity(stored_in.0)
                .map_err(|_| format!("StoredIn references missing storage {:?}", stored_in.0))?;
            if !storage.contains::<Stockpile>() {
                return Err(format!(
                    "StoredIn target {:?} is not a Stockpile",
                    stored_in.0
                ));
            }
            stored_sources
                .entry(stored_in.0)
                .or_default()
                .insert(item.id());
        }
        if let Some(stored_by_mixer) = stored_by_mixer {
            if resource != hw_core::logistics::ResourceType::StasisMud {
                return Err(format!(
                    "StoredByMixer source {:?} is not StasisMud",
                    item.id()
                ));
            }
            let mixer = candidate.get_entity(stored_by_mixer.0).map_err(|_| {
                format!(
                    "StoredByMixer references missing mixer {:?}",
                    stored_by_mixer.0
                )
            })?;
            if !mixer.contains::<MudMixerStorage>() {
                return Err(format!(
                    "StoredByMixer target {:?} has no MudMixerStorage",
                    stored_by_mixer.0
                ));
            }
            mixer_sources
                .entry(stored_by_mixer.0)
                .or_default()
                .insert(item.id());
        }
    }
    Ok(())
}

pub(super) fn validate_loaded_capacity(
    candidate: &World,
    loaded_sources: &HashMap<Entity, HashSet<Entity>>,
) -> Result<(), String> {
    for (carrier, items) in loaded_sources {
        let capacity = candidate
            .get::<Wheelbarrow>(*carrier)
            .expect("validated Wheelbarrow carrier")
            .capacity;
        if items.len() > capacity {
            return Err(format!(
                "Wheelbarrow {carrier:?} contains {} item(s), exceeding capacity {capacity}",
                items.len()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_mixers(
    candidate: &World,
    mixer_sources: &HashMap<Entity, HashSet<Entity>>,
) -> Result<(), String> {
    for mixer in candidate.iter_entities() {
        let Some(storage) = mixer.get::<MudMixerStorage>() else {
            continue;
        };
        let expected = mixer_sources.get(&mixer.id()).map_or(0, HashSet::len);
        if storage.mud as usize != expected {
            return Err(format!(
                "MudMixer {:?} stores {} mud unit(s), but owns {expected} StasisMud item(s)",
                mixer.id(),
                storage.mud
            ));
        }
        if storage.sand > MUD_MIXER_CAPACITY
            || storage.rock > MUD_MIXER_CAPACITY
            || storage.mud > MUD_MIXER_MUD_CAPACITY
        {
            return Err(format!(
                "MudMixer {:?} exceeds durable capacity",
                mixer.id()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_stockpiles(
    candidate: &World,
    stored_sources: &HashMap<Entity, HashSet<Entity>>,
) -> Result<(), String> {
    for storage in candidate.iter_entities() {
        let Some(actual_items) = storage.get::<StoredItems>() else {
            if storage.contains::<Stockpile>() && stored_sources.contains_key(&storage.id()) {
                return Err(format!(
                    "Stockpile {:?} is missing StoredItems",
                    storage.id()
                ));
            }
            continue;
        };
        if !storage.contains::<Stockpile>() {
            return Err(format!(
                "StoredItems target {:?} is not a Stockpile",
                storage.id()
            ));
        }
        let actual: HashSet<_> = actual_items.iter().collect();
        if actual.len() != actual_items.len() {
            return Err(format!(
                "StoredItems for Stockpile {:?} contains duplicate sources",
                storage.id()
            ));
        }
        let expected = stored_sources
            .get(&storage.id())
            .cloned()
            .unwrap_or_default();
        if actual != expected {
            return Err(format!(
                "StoredIn/StoredItems are not symmetric for Stockpile {:?}",
                storage.id()
            ));
        }
        let stockpile = storage.get::<Stockpile>().expect("filtered Stockpile");
        let capacity = stockpile.capacity;
        if expected.len() > capacity {
            return Err(format!(
                "Stockpile {:?} contains {} item(s), exceeding capacity {capacity}",
                storage.id(),
                expected.len()
            ));
        }
        if is_bucket_storage_role(candidate, &storage) {
            if !matches!(
                stockpile.resource_type,
                None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
            ) {
                return Err(format!(
                    "BucketStorage {:?} has an incompatible Stockpile resource type",
                    storage.id()
                ));
            }
            let storage_owner = storage.get::<BelongsTo>().map(|owner| owner.0);
            for item_entity in &expected {
                let item = candidate
                    .get::<ResourceItem>(*item_entity)
                    .expect("StoredIn source was validated as a ResourceItem");
                if !matches!(
                    item.0,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                ) {
                    return Err(format!(
                        "BucketStorage {:?} contains non-bucket ResourceItem {:?}",
                        storage.id(),
                        item_entity
                    ));
                }
                if storage_owner.is_none()
                    || candidate
                        .get::<BelongsTo>(*item_entity)
                        .map(|owner| owner.0)
                        != storage_owner
                {
                    return Err(format!(
                        "BucketStorage {:?} and stored bucket {:?} have different durable owners",
                        storage.id(),
                        item_entity
                    ));
                }
            }
        } else {
            for item_entity in &expected {
                let item = candidate
                    .get::<ResourceItem>(*item_entity)
                    .expect("StoredIn source was validated as a ResourceItem");
                if stockpile.resource_type != Some(item.0) {
                    return Err(format!(
                        "Stockpile {:?} resource type does not match stored ResourceItem {:?}",
                        storage.id(),
                        item_entity
                    ));
                }
            }
        }
    }
    Ok(())
}
