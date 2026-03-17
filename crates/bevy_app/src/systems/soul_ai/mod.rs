//! 魂（ワーカー）AI モジュール
//!
//! 魂のバイタル管理、タスク実行、仕事管理、アイドル行動を一括して管理します。

use bevy::prelude::*;

pub mod adapters;
pub mod decide;
pub mod execute;
pub mod helpers;
pub mod perceive;
pub mod scheduling {
    pub use hw_core::system_sets::{FamiliarAiSystemSet, SoulAiSystemSet};
}
pub mod update;

use crate::systems::GameSystemSet;
use scheduling::{FamiliarAiSystemSet, SoulAiSystemSet};

pub struct SoulAiPlugin;

impl Plugin for SoulAiPlugin {
    fn build(&self, app: &mut App) {
        // hw_ai の SoulAiCorePlugin でコアシステムを登録
        app.add_plugins(hw_soul_ai::SoulAiCorePlugin);

        // drifting 書き込み adapter (hw_ai → root PopulationManager ブリッジ)
        app.add_observer(adapters::on_drifting_escape_started)
            .add_observer(adapters::on_soul_escaped);

        // Soul AI は Familiar AI の後に実行される
        // FamiliarAiSystemSet::Execute → ApplyDeferred → SoulAiSystemSet::Perceive
        app.configure_sets(
            Update,
            (
                SoulAiSystemSet::Perceive,
                SoulAiSystemSet::Update,
                SoulAiSystemSet::Decide,
                SoulAiSystemSet::Execute,
            )
                .chain()
                .after(FamiliarAiSystemSet::Execute)
                .in_set(GameSystemSet::Logic),
        )
        .add_systems(
            Update,
            (
                // === Familiar → Soul 間の同期 ===
                bevy::ecs::schedule::ApplyDeferred
                    .after(FamiliarAiSystemSet::Execute)
                    .before(SoulAiSystemSet::Perceive),
                // Perceive → Update 間の同期
                bevy::ecs::schedule::ApplyDeferred
                    .after(SoulAiSystemSet::Perceive)
                    .before(SoulAiSystemSet::Update),
                // Update → Decide 間の同期
                bevy::ecs::schedule::ApplyDeferred
                    .after(SoulAiSystemSet::Update)
                    .before(SoulAiSystemSet::Decide),
                // Decide → Execute 間の同期
                bevy::ecs::schedule::ApplyDeferred
                    .after(SoulAiSystemSet::Decide)
                    .before(SoulAiSystemSet::Execute),
                // === Execute Phase ===
                // エンティティ生成（GameAssets 依存のため bevy_app に残留）
                execute::gathering_spawn::gathering_spawn_system
                    .after(
                        hw_soul_ai::soul_ai::execute::gathering_spawn::gathering_spawn_logic_system,
                    )
                    .in_set(SoulAiSystemSet::Execute),
            ),
        );
    }
}
