//! 使い魔のアニメーション

use bevy::prelude::*;

use hw_core::constants::*;

use super::components::Familiar;

/// 使い魔のアニメーション更新システム
pub fn familiar_animation_system(
    time: Res<Time>,
    game_assets: Res<crate::assets::GameAssets>,
    mut q_animations: Query<&mut super::components::FamiliarAnimation, With<Familiar>>,
    mut q_visuals: Query<
        (
            &hw_visual::FamiliarVisualOwner,
            &mut Sprite,
            &mut Transform,
            &mut hw_visual::FamiliarVisualOffset,
        ),
        Without<Familiar>,
    >,
) {
    for (owner, mut sprite, mut transform, mut offset) in q_visuals.iter_mut() {
        let Ok(mut anim) = q_animations.get_mut(owner.owner) else {
            continue;
        };

        let desired_flip_x = !anim.facing_right;
        if sprite.flip_x != desired_flip_x {
            sprite.flip_x = desired_flip_x;
        }

        if anim.is_moving {
            anim.timer += time.delta_secs();
            anim.frame = ((anim.timer * FAMILIAR_MOVE_ANIMATION_FPS) as usize)
                % FAMILIAR_MOVE_ANIMATION_FRAMES;
        } else {
            anim.timer = 0.0;
            anim.frame = 0;
        }

        let desired_image = match anim.frame {
            0 => game_assets.familiar.clone(),
            1 => game_assets.familiar_anim_2.clone(),
            2 => game_assets.familiar_anim_3.clone(),
            _ => game_assets.familiar_anim_4.clone(),
        };
        if sprite.texture_atlas.is_some() {
            sprite.texture_atlas = None;
        }
        if sprite.image != desired_image {
            sprite.image = desired_image;
        }

        anim.hover_timer += time.delta_secs() * FAMILIAR_HOVER_SPEED;
        let hover_amplitude = if anim.is_moving {
            FAMILIAR_HOVER_AMPLITUDE_MOVE
        } else {
            FAMILIAR_HOVER_AMPLITUDE_IDLE
        };
        let hover_offset = anim.hover_timer.sin() * hover_amplitude;

        let dir_tilt = if anim.is_moving {
            if anim.facing_right { -0.04 } else { 0.04 }
        } else {
            0.0
        };
        let wobble_tilt = (anim.hover_timer * 0.8).sin() * FAMILIAR_HOVER_TILT_AMPLITUDE;
        let tilt_radians = dir_tilt + wobble_tilt;
        let desired_translation = Vec3::Y * hover_offset;
        let desired_rotation = Quat::from_rotation_z(tilt_radians);
        if transform.translation != desired_translation {
            transform.translation = desired_translation;
        }
        if transform.rotation != desired_rotation {
            transform.rotation = desired_rotation;
        }
        offset.hover_offset = hover_offset;
        offset.tilt_radians = tilt_radians;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::prelude::*;
    use bevy::time::TimeUpdateStrategy;
    use hw_core::soul::Path;

    use super::*;
    use crate::plugins::startup::create_game_assets;

    #[test]
    fn visual_hover_does_not_move_the_familiar_logical_root() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<Gltf>()
            .init_asset::<WorldAsset>();

        let asset_server = app.world().resource::<AssetServer>().clone();
        let game_assets = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            create_game_assets(&asset_server, &mut images)
        };
        app.insert_resource(game_assets)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(
                Update,
                (familiar_animation_system, hw_familiar_ai::familiar_movement).chain(),
            );

        let logical_position = Vec3::new(12.0, 34.0, 5.0);
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                Transform::from_translation(logical_position),
                Path::default(),
                super::super::components::FamiliarAnimation::default(),
            ))
            .id();
        let visual = app
            .world_mut()
            .spawn((
                hw_visual::FamiliarVisualOwner { owner: familiar },
                hw_visual::FamiliarVisualOffset::default(),
                Sprite::default(),
                Transform::default(),
            ))
            .id();

        // The first TimePlugin update establishes the initial timestamp.
        app.update();
        app.update();

        assert_eq!(
            app.world().get::<Transform>(familiar).unwrap().translation,
            logical_position
        );
        assert_ne!(
            app.world().get::<Transform>(visual).unwrap().translation.y,
            0.0
        );
    }
}
