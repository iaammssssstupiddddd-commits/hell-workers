use bevy::prelude::*;
use hw_energy::{PowerGrid, YardPowerGrid};
use hw_world::zones::Yard;

/// Yard が追加されたとき PowerGrid エンティティをスポーン。
pub fn on_yard_added(on: On<Add, Yard>, mut commands: Commands) {
    let yard_entity = on.entity;
    commands.spawn((
        Name::new("PowerGrid"),
        PowerGrid::default(),
        YardPowerGrid(yard_entity),
    ));
    info!("[Energy] PowerGrid spawned for Yard {:?}", yard_entity);
}

/// Yard が削除されたとき対応する PowerGrid をデスポーン。
/// PowerGrid despawn 時、Bevy が `GeneratesFor`/`ConsumesFrom` Source を
/// 参照元エンティティから自動削除する。明示的なクリーンアップは不要。
pub fn on_yard_removed(
    on: On<Remove, Yard>,
    q_grids: Query<(Entity, &YardPowerGrid)>,
    mut commands: Commands,
) {
    let yard_entity = on.entity;
    for (grid_entity, yard_ref) in &q_grids {
        if yard_ref.0 == yard_entity {
            commands.entity(grid_entity).despawn();
            break;
        }
    }
}
