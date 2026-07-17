use super::*;

/// Soul モード用セクション（SoulSectionNode でモード切替時に show/hide）。
pub(super) fn spawn_soul_section(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Percent(100.0),
            ..default()
        },
        SoulSectionNode,
    ))
    .with_children(|s| {
        sec_label(s, "─ Soul ─", font);
        spawn_btn(
            s,
            VisualTestAction::AddSoul,
            "+Soul [=]",
            Val::Percent(49.0),
            font,
        );
        spawn_btn(
            s,
            VisualTestAction::RemoveSoul,
            "-Soul [-]",
            Val::Percent(49.0),
            font,
        );
        spawn_btn(
            s,
            VisualTestAction::SelectNextSoul,
            "Select [Tab]",
            Val::Percent(49.0),
            font,
        );
        spawn_btn(
            s,
            VisualTestAction::ResetSoulPos,
            "Reset [R]",
            Val::Percent(49.0),
            font,
        );

        sec_label(s, "─ Shadow Caster ─", font);
        spawn_btn(
            s,
            VisualTestAction::SetSoulLayout(SoulLayout::Default),
            "Default",
            Val::Percent(49.0),
            font,
        );
        spawn_btn(
            s,
            VisualTestAction::SetSoulLayout(SoulLayout::ShadowCompare),
            "Shadow A/B",
            Val::Percent(49.0),
            font,
        );
        s.spawn((
            Text::new("Default  [Y]"),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(SFONT),
                ..default()
            },
            TextColor(VAL_COL),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(BTN_GAP)),
                ..default()
            },
            DynamicTextKind::ShadowLayout,
        ));

        sec_label(s, "─ 表情 ─", font);
        for expr in FaceExpression::ALL {
            spawn_btn(
                s,
                VisualTestAction::SetFace(expr),
                expr.label(),
                Val::Percent(49.0),
                font,
            );
        }
        spawn_btn(
            s,
            VisualTestAction::SetFaceAll,
            "全表情 [G]",
            Val::Percent(100.0),
            font,
        );

        sec_label(s, "─ アニメーション ─", font);
        for (i, &name) in ANIM_CLIP_NAMES.iter().enumerate() {
            spawn_btn(
                s,
                VisualTestAction::SetAnimation(i),
                name,
                Val::Percent(49.0),
                font,
            );
        }

        sec_label(s, "─ モーション ─", font);
        for mode in MotionMode::ALL {
            spawn_btn(
                s,
                VisualTestAction::SetMotion(mode),
                mode.label(),
                Val::Percent(49.0),
                font,
            );
        }

        sec_label(s, "─ シェーダー ─", font);
        param_row(
            s,
            "Ghost:",
            "1.00",
            DynamicTextKind::Ghost,
            font,
            VisualTestAction::GhostDown,
            VisualTestAction::GhostUp,
        );
        param_row(
            s,
            "Rim:  ",
            "0.28",
            DynamicTextKind::Rim,
            font,
            VisualTestAction::RimDown,
            VisualTestAction::RimUp,
        );
        param_row(
            s,
            "Post: ",
            "4.0",
            DynamicTextKind::Posterize,
            font,
            VisualTestAction::PosterizeDown,
            VisualTestAction::PosterizeUp,
        );
        spawn_btn(
            s,
            VisualTestAction::ResetShader,
            "Reset Shader [P]",
            Val::Percent(100.0),
            font,
        );
    });
}
