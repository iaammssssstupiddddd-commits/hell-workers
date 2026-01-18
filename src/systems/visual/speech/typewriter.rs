use super::components::*;
use bevy::prelude::*;

/// タイプライター効果の更新システム
pub fn update_typewriter(
    time: Res<Time>,
    mut q_bubbles: Query<(&mut TypewriterEffect, &mut Text2d)>,
) {
    let dt = time.delta_secs();

    for (mut tw, mut text) in q_bubbles.iter_mut() {
        tw.elapsed += dt;

        if tw.elapsed >= tw.char_interval && tw.current_len < tw.full_text.chars().count() {
            tw.elapsed = 0.0;
            tw.current_len += 1;

            // マルチバイト文字（絵文字や日本語）を考慮して char ベースで切り出す
            let display_text: String = tw.full_text.chars().take(tw.current_len).collect();
            text.0 = display_text;
        }
    }
}
