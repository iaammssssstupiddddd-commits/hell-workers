use super::*;

pub(super) fn validate(candidate: &World) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        let spatial_root = if entity.contains::<DamnedSoul>() {
            Some("DamnedSoul")
        } else if entity.contains::<Familiar>() {
            Some("Familiar")
        } else if entity.contains::<Building>() {
            Some("Building")
        } else if entity.contains::<Blueprint>() {
            Some("Blueprint")
        } else if entity.contains::<FloorConstructionSite>() {
            Some("FloorConstructionSite")
        } else if entity.contains::<FloorTileBlueprint>() {
            Some("FloorTileBlueprint")
        } else if entity.contains::<WallConstructionSite>() {
            Some("WallConstructionSite")
        } else if entity.contains::<WallTileBlueprint>() {
            Some("WallTileBlueprint")
        } else if entity.contains::<Tree>() {
            Some("Tree")
        } else if entity.contains::<Rock>() {
            Some("Rock")
        } else if entity.contains::<ResourceItem>() {
            Some("ResourceItem")
        } else if entity.contains::<Stockpile>() {
            Some("Stockpile")
        } else {
            None
        };
        if let Some(label) = spatial_root
            && !entity.contains::<Transform>()
        {
            return Err(format!("{label} {:?} has no Transform", entity.id()));
        }
        if entity.contains::<DamnedSoul>()
            && (!entity.contains::<IdleState>()
                || !entity.contains::<DreamState>()
                || !entity.contains::<Inventory>())
        {
            return Err(format!(
                "DamnedSoul {:?} is missing IdleState, DreamState, or Inventory",
                entity.id()
            ));
        }
        if entity.contains::<Tree>() && !entity.contains::<TreeVariant>() {
            return Err(format!("Tree {:?} has no TreeVariant", entity.id()));
        }
    }
    Ok(())
}
