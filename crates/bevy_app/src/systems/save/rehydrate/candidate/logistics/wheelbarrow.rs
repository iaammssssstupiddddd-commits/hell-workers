use super::super::*;

pub(super) fn validate(
    candidate: &World,
    navigation: &DurableNavigationView<'_>,
    inventory_owners: &HashSet<Entity>,
    loaded_sources: &HashMap<Entity, HashSet<Entity>>,
) -> Result<(), String> {
    let mut home_counts: HashMap<Entity, usize> = HashMap::new();
    let mut parked_sources: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    for wheelbarrow in candidate.iter_entities() {
        if !wheelbarrow.contains::<Wheelbarrow>() {
            if wheelbarrow.contains::<LoadedItems>() {
                return Err(format!(
                    "LoadedItems target {:?} is not a Wheelbarrow",
                    wheelbarrow.id()
                ));
            }
            continue;
        }
        if wheelbarrow
            .get::<ResourceItem>()
            .is_none_or(|item| item.0 != hw_core::logistics::ResourceType::Wheelbarrow)
        {
            return Err(format!(
                "Wheelbarrow {:?} is missing ResourceItem(Wheelbarrow)",
                wheelbarrow.id()
            ));
        }
        if !inventory_owners.contains(&wheelbarrow.id()) {
            let transform = wheelbarrow
                .get::<Transform>()
                .ok_or_else(|| format!("Wheelbarrow {:?} has no Transform", wheelbarrow.id()))?;
            if navigation
                .nearest_walkable_position(transform.translation.truncate())
                .is_none()
            {
                return Err(format!(
                    "Wheelbarrow {:?} has no walkable normalization cell",
                    wheelbarrow.id()
                ));
            }
        }
        let home = validate_wheelbarrow_home(candidate, wheelbarrow.id(), &wheelbarrow)?;
        *home_counts.entry(home).or_default() += 1;
        if let Some(parked_at) = wheelbarrow.get::<ParkedAt>() {
            validate_parking_target(candidate, parked_at.0)?;
            if parked_at.0 != home {
                return Err(format!(
                    "Wheelbarrow {:?} is parked at {:?}, but its durable home is {home:?}",
                    wheelbarrow.id(),
                    parked_at.0
                ));
            }
            parked_sources
                .entry(parked_at.0)
                .or_default()
                .insert(wheelbarrow.id());
        }

        let actual_loaded = wheelbarrow.get::<LoadedItems>();
        let actual_loaded_set: HashSet<_> = actual_loaded
            .into_iter()
            .flat_map(|items| items.iter())
            .collect();
        if actual_loaded.is_some_and(|items| items.len() != actual_loaded_set.len()) {
            return Err(format!(
                "LoadedItems for Wheelbarrow {:?} contains duplicate sources",
                wheelbarrow.id()
            ));
        }
        let expected_loaded = loaded_sources
            .get(&wheelbarrow.id())
            .cloned()
            .unwrap_or_default();
        if actual_loaded_set != expected_loaded {
            return Err(format!(
                "LoadedIn/LoadedItems are not symmetric for Wheelbarrow {:?}",
                wheelbarrow.id()
            ));
        }
    }
    for (parking, count) in home_counts {
        let capacity = candidate
            .get::<WheelbarrowParking>(parking)
            .expect("validated WheelbarrowParking target")
            .capacity;
        if count > capacity {
            return Err(format!(
                "WheelbarrowParking {parking:?} contains {count} wheelbarrow(s), exceeding capacity {capacity}"
            ));
        }
    }
    for parking in candidate.iter_entities() {
        if !parking.contains::<WheelbarrowParking>() {
            if parking.contains::<ParkedWheelbarrows>() {
                return Err(format!(
                    "ParkedWheelbarrows target {:?} is not WheelbarrowParking",
                    parking.id()
                ));
            }
            continue;
        }
        let actual: HashSet<_> = parking
            .get::<ParkedWheelbarrows>()
            .into_iter()
            .flat_map(|wheelbarrows| wheelbarrows.iter())
            .collect();
        let raw_len = parking
            .get::<ParkedWheelbarrows>()
            .map_or(0, |wheelbarrows| wheelbarrows.iter().count());
        if actual.len() != raw_len {
            return Err(format!(
                "ParkedWheelbarrows for parking {:?} contains duplicate sources",
                parking.id()
            ));
        }
        let expected = parked_sources
            .get(&parking.id())
            .cloned()
            .unwrap_or_default();
        if actual != expected {
            return Err(format!(
                "ParkedAt/ParkedWheelbarrows are not symmetric for parking {:?}",
                parking.id()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_wheelbarrow_home(
    candidate: &World,
    wheelbarrow: Entity,
    wheelbarrow_ref: &EntityRef<'_>,
) -> Result<Entity, String> {
    let belongs_to = wheelbarrow_ref
        .get::<BelongsTo>()
        .ok_or_else(|| format!("Wheelbarrow {wheelbarrow:?} has no durable BelongsTo home"))?;
    validate_parking_target(candidate, belongs_to.0)?;
    Ok(belongs_to.0)
}

fn validate_parking_target(candidate: &World, parking: Entity) -> Result<(), String> {
    let parking_ref = candidate
        .get_entity(parking)
        .map_err(|_| format!("wheelbarrow parking reference {parking:?} is missing"))?;
    if parking_ref.contains::<WheelbarrowParking>() {
        Ok(())
    } else {
        Err(format!(
            "wheelbarrow parking reference {parking:?} is not a WheelbarrowParking"
        ))
    }
}
