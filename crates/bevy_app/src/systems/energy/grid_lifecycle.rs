use bevy::prelude::*;
use hw_energy::{ConsumesFrom, PowerConsumer, PowerGrid, YardPowerGrid};
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

/// PowerConsumer が追加されたとき、包含する Yard の PowerGrid に ConsumesFrom を付与する。
/// setup_outdoor_lamp が PowerConsumer を insert したとき自動発火。
/// soul_spa_place/input.rs と同じ Yard lookup パターン（yard.contains(pos)）。
pub fn on_power_consumer_added(
    on: On<Add, PowerConsumer>,
    mut commands: Commands,
    q_transform: Query<&Transform>,
    q_yards: Query<(Entity, &Yard)>,
    q_grids: Query<(Entity, &YardPowerGrid)>,
) {
    let entity = on.entity;
    let Ok(transform) = q_transform.get(entity) else {
        return;
    };
    let pos = transform.translation.xy();
    let Some(yard_entity) = q_yards
        .iter()
        .find(|(_, y)| y.contains(pos))
        .map(|(e, _)| e)
    else {
        // Yard 外のランプは ConsumesFrom なし → 常時 Unpowered
        return;
    };
    let Some(grid_entity) = q_grids
        .iter()
        .find(|(_, ypg)| ypg.0 == yard_entity)
        .map(|(e, _)| e)
    else {
        return;
    };
    commands.entity(entity).insert(ConsumesFrom(grid_entity));
}
