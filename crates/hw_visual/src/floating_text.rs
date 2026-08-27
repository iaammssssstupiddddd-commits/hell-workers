//! フローティングテキスト実装
//!
//! ポップアップやフローティングテキストの汎用実装

use bevy::prelude::*;

use crate::blueprint::{CompletionText, DeliveryPopup};

type StandaloneFloatingTextQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut FloatingText,
        &'static mut Transform,
        &'static mut TextColor,
    ),
    (Without<DeliveryPopup>, Without<CompletionText>),
>;

/// フローティングテキストの設定
#[derive(Debug, Clone)]
pub struct FloatingTextConfig {
    /// 表示時間（秒）
    pub lifetime: f32,
    /// 上昇速度（ピクセル/秒）
    pub velocity: Vec2,
    /// 初期色
    pub initial_color: Color,
    /// フェードアウトするか
    pub fade_out: bool,
}

impl Default for FloatingTextConfig {
    fn default() -> Self {
        Self {
            lifetime: 1.0,
            velocity: Vec2::new(0.0, 20.0),
            initial_color: Color::WHITE,
            fade_out: true,
        }
    }
}

/// フローティングテキストコンポーネント
#[derive(Component, Clone)]
pub struct FloatingText {
    /// 残り表示時間
    pub lifetime: f32,
    /// 設定
    pub config: FloatingTextConfig,
}

/// フローティングテキストを生成する
pub fn spawn_floating_text(
    commands: &mut Commands,
    text: impl Into<String>,
    position: Vec3,
    config: FloatingTextConfig,
    font_size: Option<f32>,
    font: Handle<Font>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        FloatingText {
            lifetime: config.lifetime,
            config: config.clone(),
        },
        Text2d::new(text),
        TextFont {
            font: font.into(),
            font_size: FontSize::Px(font_size.unwrap_or(12.0)),
            ..default()
        },
        TextColor(config.initial_color),
        Transform::from_translation(position),
    ));

    entity_commands.insert(Name::new("FloatingText"));

    entity_commands.id()
}

/// フローティングテキストを更新する
/// 返り値: (削除すべきか, 新しい位置, 新しい透明度)
pub fn update_floating_text(
    time: &Time,
    text: &mut FloatingText,
    current_position: Vec3,
) -> (bool, Vec3, f32) {
    text.lifetime -= time.delta_secs();

    if text.lifetime <= 0.0 {
        return (true, current_position, 0.0);
    }

    // 上昇
    let new_position = current_position
        + Vec3::new(
            text.config.velocity.x * time.delta_secs(),
            text.config.velocity.y * time.delta_secs(),
            0.0,
        );

    // フェードアウト
    let alpha = if text.config.fade_out {
        (text.lifetime / text.config.lifetime).min(1.0)
    } else {
        1.0
    };

    (false, new_position, alpha)
}

/// 単体で動作するFloatingText用のシステム
pub fn update_all_floating_texts_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_texts: StandaloneFloatingTextQuery,
) {
    for (entity, mut text, mut transform, mut text_color) in q_texts.iter_mut() {
        let current_pos = transform.translation;
        let (despawn, new_pos, alpha) = update_floating_text(&time, &mut text, current_pos);

        if despawn {
            commands.entity(entity).try_despawn();
            continue;
        }

        transform.translation = new_pos;
        let mut color = text_color.0.to_srgba();
        color.alpha = alpha;
        text_color.0 = color.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::update_delivery_popup_system;

    #[test]
    fn blueprint_popup_has_only_its_specialized_lifetime_owner() {
        let mut app = App::new();
        app.set_error_handler(bevy::ecs::error::panic);
        app.init_resource::<Time>();
        app.add_systems(
            Update,
            (
                update_all_floating_texts_system,
                update_delivery_popup_system,
            )
                .chain_ignore_deferred(),
        );

        let config = FloatingTextConfig {
            lifetime: 0.0,
            ..default()
        };
        let popup = app
            .world_mut()
            .spawn((
                FloatingText {
                    lifetime: 0.0,
                    config: config.clone(),
                },
                DeliveryPopup {
                    floating_text: FloatingText {
                        lifetime: 0.0,
                        config,
                    },
                },
                Transform::default(),
                TextColor::default(),
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(popup).is_err());
    }
}
