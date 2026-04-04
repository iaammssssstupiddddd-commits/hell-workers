use bevy::prelude::*;
use hw_energy::{
    GridConsumers, GridGenerators, PowerConsumer, PowerGenerator, PowerGrid, Unpowered,
};

/// PowerGrid の generation/consumption を集計し、停電状態を更新する。
/// soul_spa_power_output_system の後に実行することで PowerGenerator の変化を即時反映する。
pub fn grid_recalc_system(
    mut q_grids: Query<(
        &mut PowerGrid,
        Option<&GridGenerators>,
        Option<&GridConsumers>,
    )>,
    q_generators: Query<&PowerGenerator>,
    q_consumers: Query<&PowerConsumer>,
    mut commands: Commands,
) {
    for (mut grid, generators_opt, consumers_opt) in q_grids.iter_mut() {
        let new_gen: f32 = generators_opt
            .map(|generators| {
                generators
                    .iter()
                    .filter_map(|e| q_generators.get(*e).ok())
                    .map(|g| g.current_output)
                    .sum()
            })
            .unwrap_or(0.0);
        let new_cons: f32 = consumers_opt
            .map(|consumers| {
                consumers
                    .iter()
                    .filter_map(|e| q_consumers.get(*e).ok())
                    .map(|c| c.demand)
                    .sum()
            })
            .unwrap_or(0.0);
        // consumers == 0 は停電なし（PowerGrid::default() の仕様に合わせる）
        let new_powered = new_cons == 0.0 || new_gen >= new_cons;

        let gen_changed = (grid.generation - new_gen).abs() > f32::EPSILON;
        let cons_changed = (grid.consumption - new_cons).abs() > f32::EPSILON;
        let powered_changed = grid.powered != new_powered;

        if gen_changed {
            grid.generation = new_gen;
        }
        if cons_changed {
            grid.consumption = new_cons;
        }

        if powered_changed {
            grid.powered = new_powered;
            info!(
                "[Energy] Grid {} (gen={:.2}W, cons={:.2}W)",
                if new_powered { "POWERED" } else { "BLACKOUT" },
                new_gen,
                new_cons
            );
        }

        // powered_changed に加え、グリッドが通電中にコンシューマーが追加された場合も
        // Unpowered マーカーを同期する（新規追加コンシューマーは #[require(Unpowered)] で
        // デフォルト Unpowered になるが、powered_changed は発生しないため）。
        let sync_consumers = powered_changed || (new_powered && cons_changed);
        if sync_consumers && let Some(consumers) = consumers_opt {
            for &consumer in consumers.iter() {
                if new_powered {
                    commands.entity(consumer).remove::<Unpowered>();
                } else {
                    commands.entity(consumer).try_insert(Unpowered);
                }
            }
        }
    }
}
