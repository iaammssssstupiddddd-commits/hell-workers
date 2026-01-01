use bevy::prelude::*;
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::interface::selection::SelectedEntity;

/// キーボードで使い魔に指示を与えるシステム
/// G キー = GatherResources (リソース収集)
/// P キー = Patrol (パトロール)
/// Escape = Idle (待機に戻る)
pub fn familiar_command_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedEntity>,
    mut q_familiars: Query<&mut ActiveCommand, With<Familiar>>,
) {
    // 選択されたエンティティが使い魔かチェック
    let Some(entity) = selected.0 else { return };
    let Ok(mut command) = q_familiars.get_mut(entity) else { return };

    if keyboard.just_pressed(KeyCode::KeyG) {
        command.command = FamiliarCommand::GatherResources;
        info!("FAMILIAR_COMMAND: GatherResources - 人間を働かせます！");
    } else if keyboard.just_pressed(KeyCode::KeyP) {
        command.command = FamiliarCommand::Patrol;
        info!("FAMILIAR_COMMAND: Patrol - パトロール開始");
    } else if keyboard.just_pressed(KeyCode::Escape) {
        command.command = FamiliarCommand::Idle;
        info!("FAMILIAR_COMMAND: Idle - 待機に戻る");
    }
}

/// 使い魔コマンドのビジュアルフィードバック
/// アクティブなコマンドがある場合、使い魔が光る
pub fn familiar_command_visual_system(
    mut q_familiars: Query<(&ActiveCommand, &mut Sprite), With<Familiar>>,
) {
    for (command, mut sprite) in q_familiars.iter_mut() {
        match command.command {
            FamiliarCommand::Idle => {
                // 待機中は暗めの赤
                sprite.color = Color::srgb(0.6, 0.2, 0.2);
            }
            FamiliarCommand::GatherResources => {
                // リソース収集中は明るいオレンジ
                sprite.color = Color::srgb(1.0, 0.6, 0.2);
            }
            FamiliarCommand::Patrol => {
                // パトロール中は明るい赤
                sprite.color = Color::srgb(1.0, 0.3, 0.3);
            }
            FamiliarCommand::Construct(_) => {
                // 建築中は黄色
                sprite.color = Color::srgb(1.0, 1.0, 0.3);
            }
        }
    }
}
