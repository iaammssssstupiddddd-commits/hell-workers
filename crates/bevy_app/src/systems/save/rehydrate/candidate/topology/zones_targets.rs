use super::super::*;

pub(super) fn validate_zone_links(candidate: &World) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        if let Some(paired) = entity.get::<PairedYard>() {
            if !entity.contains::<Site>() {
                return Err(format!("PairedYard source {:?} is not a Site", entity.id()));
            }
            let yard = candidate
                .get_entity(paired.0)
                .map_err(|_| format!("PairedYard target {:?} is missing", paired.0))?;
            if !yard.contains::<Yard>()
                || yard
                    .get::<PairedSite>()
                    .is_none_or(|reverse| reverse.0 != entity.id())
            {
                return Err(format!(
                    "PairedYard/PairedSite are not symmetric for Site {:?}",
                    entity.id()
                ));
            }
        }
        if let Some(paired) = entity.get::<PairedSite>() {
            if !entity.contains::<Yard>() {
                return Err(format!("PairedSite source {:?} is not a Yard", entity.id()));
            }
            let site = candidate
                .get_entity(paired.0)
                .map_err(|_| format!("PairedSite target {:?} is missing", paired.0))?;
            if !site.contains::<Site>()
                || site
                    .get::<PairedYard>()
                    .is_none_or(|reverse| reverse.0 != entity.id())
            {
                return Err(format!(
                    "PairedSite/PairedYard are not symmetric for Yard {:?}",
                    entity.id()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_target_links(candidate: &World) -> Result<(), String> {
    for source in candidate.iter_entities() {
        validate_target::<Blueprint>(
            candidate,
            &source,
            source.get::<TargetBlueprint>().map(|target| target.0),
            "TargetBlueprint",
        )?;
        validate_target::<FloorConstructionSite>(
            candidate,
            &source,
            source
                .get::<TargetFloorConstructionSite>()
                .map(|target| target.0),
            "TargetFloorConstructionSite",
        )?;
        validate_target::<WallConstructionSite>(
            candidate,
            &source,
            source
                .get::<TargetWallConstructionSite>()
                .map(|target| target.0),
            "TargetWallConstructionSite",
        )?;
        validate_target::<MudMixerStorage>(
            candidate,
            &source,
            source.get::<TargetMixer>().map(|target| target.0),
            "TargetMixer",
        )?;
        validate_target::<SoulSpaSite>(
            candidate,
            &source,
            source.get::<TargetSoulSpaSite>().map(|target| target.0),
            "TargetSoulSpaSite",
        )?;
    }
    Ok(())
}

fn validate_target<TTarget: Component>(
    candidate: &World,
    source: &EntityRef<'_>,
    target: Option<Entity>,
    relation: &str,
) -> Result<(), String> {
    let Some(target) = target else {
        return Ok(());
    };
    let target_ref = candidate.get_entity(target).map_err(|_| {
        format!(
            "{relation} source {:?} references missing target {target:?}",
            source.id()
        )
    })?;
    if !target_ref.contains::<TTarget>() {
        return Err(format!(
            "{relation} source {:?} references a target with the wrong role",
            source.id()
        ));
    }
    Ok(())
}
