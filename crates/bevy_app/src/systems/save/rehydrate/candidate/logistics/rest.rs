use super::super::*;

pub(super) fn validate(candidate: &World) -> Result<(), String> {
    let mut expected_occupants: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut expected_reservations: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    for soul in candidate.iter_entities() {
        if soul.contains::<RestingIn>() && soul.contains::<RestAreaReservedFor>() {
            return Err(format!(
                "DamnedSoul {:?} is both resting and reserved",
                soul.id()
            ));
        }
        if let Some(resting_in) = soul.get::<RestingIn>() {
            validate_rest_source_target(candidate, &soul, resting_in.0, "RestingIn")?;
            if soul
                .get::<IdleState>()
                .is_none_or(|idle| idle.behavior != IdleBehavior::Resting)
            {
                return Err(format!(
                    "RestingIn Soul {:?} is not in IdleBehavior::Resting",
                    soul.id()
                ));
            }
            expected_occupants
                .entry(resting_in.0)
                .or_default()
                .insert(soul.id());
        }
        if let Some(reserved_for) = soul.get::<RestAreaReservedFor>() {
            validate_rest_source_target(candidate, &soul, reserved_for.0, "RestAreaReservedFor")?;
            if soul
                .get::<IdleState>()
                .is_none_or(|idle| idle.behavior != IdleBehavior::GoingToRest)
            {
                return Err(format!(
                    "reserved Soul {:?} is not in IdleBehavior::GoingToRest",
                    soul.id()
                ));
            }
            expected_reservations
                .entry(reserved_for.0)
                .or_default()
                .insert(soul.id());
        }
    }
    for rest_area in candidate.iter_entities() {
        let occupant_len = rest_area
            .get::<RestAreaOccupants>()
            .map_or(0, |occupants| occupants.iter().count());
        let reservation_len = rest_area
            .get::<RestAreaReservations>()
            .map_or(0, |reservations| reservations.iter().count());
        let actual_occupants: HashSet<_> = rest_area
            .get::<RestAreaOccupants>()
            .into_iter()
            .flat_map(|occupants| occupants.iter().copied())
            .collect();
        let actual_reservations: HashSet<_> = rest_area
            .get::<RestAreaReservations>()
            .into_iter()
            .flat_map(|reservations| reservations.iter().copied())
            .collect();
        if occupant_len != actual_occupants.len() || reservation_len != actual_reservations.len() {
            return Err(format!(
                "rest relationship target {:?} contains duplicate Souls",
                rest_area.id()
            ));
        }
        if (rest_area.contains::<RestAreaOccupants>()
            || rest_area.contains::<RestAreaReservations>())
            && !rest_area.contains::<RestArea>()
        {
            return Err(format!(
                "rest relationship target {:?} is not a RestArea",
                rest_area.id()
            ));
        }
        if actual_occupants
            != expected_occupants
                .get(&rest_area.id())
                .cloned()
                .unwrap_or_default()
        {
            return Err(format!(
                "RestingIn/RestAreaOccupants are not symmetric for {:?}",
                rest_area.id()
            ));
        }
        if actual_reservations
            != expected_reservations
                .get(&rest_area.id())
                .cloned()
                .unwrap_or_default()
        {
            return Err(format!(
                "RestAreaReservedFor/RestAreaReservations are not symmetric for {:?}",
                rest_area.id()
            ));
        }
        if let Some(area) = rest_area.get::<RestArea>()
            && actual_occupants.len() + actual_reservations.len() > area.capacity
        {
            return Err(format!(
                "RestArea {:?} exceeds occupant/reservation capacity",
                rest_area.id()
            ));
        }
    }
    Ok(())
}

fn validate_rest_source_target(
    candidate: &World,
    source: &EntityRef<'_>,
    target: Entity,
    relation: &str,
) -> Result<(), String> {
    if !source.contains::<DamnedSoul>() {
        return Err(format!(
            "{relation} source {:?} is not a DamnedSoul",
            source.id()
        ));
    }
    let target_ref = candidate
        .get_entity(target)
        .map_err(|_| format!("{relation} target {target:?} is missing"))?;
    if !target_ref.contains::<RestArea>() {
        return Err(format!("{relation} target {target:?} is not a RestArea"));
    }
    Ok(())
}
