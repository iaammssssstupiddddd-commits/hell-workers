use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_core::relationships::{LoadedIn, StoredIn};

pub(super) fn try_drop_item(
    commands: &mut Commands,
    item_entity: Entity,
    drop_pos: Vec2,
    store_in: Option<Entity>,
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
    item_commands.try_remove::<hw_core::relationships::TaskWorkers>();
    true
}

pub(super) fn try_despawn_item(commands: &mut Commands, item_entity: Entity) -> bool {
    let Ok(mut item_commands) = commands.get_entity(item_entity) else {
        return false;
    };
    item_commands.try_despawn();
    true
}
