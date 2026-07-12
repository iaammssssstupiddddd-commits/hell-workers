use bevy::prelude::*;

/// テキスト入力確定イベント（`String` を含むため non-`Copy`）。
///
/// 既存の `UiIntent` / `MenuAction` は `Copy` 前提のため、リネーム等はこちらを使う。
#[derive(Message, Clone, Debug)]
pub enum TextInputIntent {
    RenameSoul { entity: Entity, name: String },
}
