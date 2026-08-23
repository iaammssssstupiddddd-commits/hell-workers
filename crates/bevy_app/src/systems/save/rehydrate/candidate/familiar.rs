use super::*;

pub(super) fn validate(candidate: &World) -> Result<(), String> {
    let mut expected_rosters: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    for soul in candidate.iter_entities() {
        let Some(commanded_by) = soul.get::<CommandedBy>() else {
            continue;
        };
        if !soul.contains::<DamnedSoul>() {
            return Err(format!(
                "CommandedBy source {:?} is not a DamnedSoul",
                soul.id()
            ));
        }
        let familiar = candidate
            .get_entity(commanded_by.0)
            .map_err(|_| format!("CommandedBy target {:?} is missing", commanded_by.0))?;
        if !familiar.contains::<Familiar>() {
            return Err(format!(
                "CommandedBy target {:?} is not a Familiar",
                commanded_by.0
            ));
        }
        expected_rosters
            .entry(commanded_by.0)
            .or_default()
            .insert(soul.id());
    }

    for entity in candidate.iter_entities() {
        let raw_roster_len = entity
            .get::<Commanding>()
            .map_or(0, |roster| roster.iter().count());
        let actual_roster: HashSet<_> = entity
            .get::<Commanding>()
            .into_iter()
            .flat_map(|roster| roster.iter().copied())
            .collect();
        if raw_roster_len != actual_roster.len() {
            return Err(format!(
                "Commanding for Familiar {:?} contains duplicate Souls",
                entity.id()
            ));
        }
        let expected_roster = expected_rosters
            .get(&entity.id())
            .cloned()
            .unwrap_or_default();
        if entity.contains::<Commanding>() && !entity.contains::<Familiar>() {
            return Err(format!(
                "Commanding target {:?} is not a Familiar",
                entity.id()
            ));
        }
        if actual_roster != expected_roster {
            return Err(format!(
                "CommandedBy/Commanding are not symmetric for Familiar {:?}",
                entity.id()
            ));
        }
        if !entity.contains::<Familiar>() {
            continue;
        }
        let Some(operation) = entity.get::<FamiliarOperation>() else {
            continue;
        };
        let roster_len = actual_roster.len();
        if operation.max_controlled_soul < roster_len {
            return Err(format!(
                "FamiliarOperation.max_controlled_soul is {} but the Commanding roster contains {roster_len} Soul(s)",
                operation.max_controlled_soul
            ));
        }
    }
    Ok(())
}
