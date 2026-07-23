use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_core::relationships::{LoadedIn, StoredIn};
use hw_logistics::{BelongsTo, stockpile_owner_accepts_item};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StockpileOwnerDisposition {
    Preserve,
    Claim(Entity),
    Reject,
}

pub(super) fn stockpile_owner_disposition(
    item_owner: Option<Entity>,
    stockpile_owner: Option<Entity>,
    is_bucket_storage: bool,
) -> StockpileOwnerDisposition {
    if is_bucket_storage {
        return if item_owner.is_some() && item_owner == stockpile_owner {
            StockpileOwnerDisposition::Preserve
        } else {
            StockpileOwnerDisposition::Reject
        };
    }
    if !stockpile_owner_accepts_item(item_owner, stockpile_owner) {
        return StockpileOwnerDisposition::Reject;
    }
    match (item_owner, stockpile_owner) {
        (None, Some(owner)) => StockpileOwnerDisposition::Claim(owner),
        _ => StockpileOwnerDisposition::Preserve,
    }
}

pub(super) fn try_drop_item(
    commands: &mut Commands,
    item_entity: Entity,
    drop_pos: Vec2,
    store_in: Option<Entity>,
    claim_owner: Option<Entity>,
) -> bool {
    let Ok(mut item_commands) = commands.get_entity(item_entity) else {
        return false;
    };

    if let Some(stockpile) = store_in {
        item_commands.try_insert((
            Visibility::Visible,
            Transform::from_xyz(drop_pos.x, drop_pos.y, Z_ITEM_PICKUP),
            StoredIn(stockpile),
        ));
        if let Some(owner) = claim_owner {
            item_commands.try_insert(BelongsTo(owner));
        }
    } else {
        item_commands.try_insert((
            Visibility::Visible,
            Transform::from_xyz(drop_pos.x, drop_pos.y, Z_ITEM_PICKUP),
        ));
        item_commands.try_remove::<StoredIn>();
    }
    item_commands.try_remove::<LoadedIn>();
    item_commands.try_remove::<hw_core::relationships::DeliveringTo>();
    item_commands.try_remove::<hw_jobs::IssuedBy>();
    true
}

pub(super) fn try_despawn_item(commands: &mut Commands, item_entity: Entity) -> bool {
    let Ok(mut item_commands) = commands.get_entity(item_entity) else {
        return false;
    };
    item_commands.try_despawn();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource)]
    struct DropSpec {
        item: Entity,
        stockpile: Entity,
        owner: Entity,
    }

    fn drop_and_claim(mut commands: Commands, spec: Res<DropSpec>) {
        assert!(try_drop_item(
            &mut commands,
            spec.item,
            Vec2::new(3.0, 4.0),
            Some(spec.stockpile),
            Some(spec.owner),
        ));
    }

    #[test]
    fn ordinary_stockpile_claims_an_unowned_item_during_unload() {
        let mut app = App::new();
        let owner = app.world_mut().spawn_empty().id();
        let stockpile = app.world_mut().spawn_empty().id();
        let wheelbarrow = app.world_mut().spawn_empty().id();
        let item = app.world_mut().spawn(LoadedIn(wheelbarrow)).id();
        app.insert_resource(DropSpec {
            item,
            stockpile,
            owner,
        });
        app.add_systems(Update, drop_and_claim);

        app.update();

        assert_eq!(app.world().get::<BelongsTo>(item), Some(&BelongsTo(owner)));
        assert_eq!(
            app.world().get::<StoredIn>(item).map(|stored| stored.0),
            Some(stockpile)
        );
        assert!(app.world().get::<LoadedIn>(item).is_none());
    }

    #[test]
    fn owner_disposition_rejects_cross_owner_and_unowned_bucket_transfers() {
        let owner = Entity::from_raw_u32(1).expect("valid entity");
        let other = Entity::from_raw_u32(2).expect("valid entity");

        assert_eq!(
            stockpile_owner_disposition(Some(owner), Some(other), false),
            StockpileOwnerDisposition::Reject
        );
        assert_eq!(
            stockpile_owner_disposition(None, Some(owner), true),
            StockpileOwnerDisposition::Reject
        );
        assert_eq!(
            stockpile_owner_disposition(None, Some(owner), false),
            StockpileOwnerDisposition::Claim(owner)
        );
    }
}
