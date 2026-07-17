use super::*;

/// Build モード用セクション（BuildSectionNode でモード切替時に show/hide）。
pub(super) fn spawn_build_section(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Percent(100.0),
            display: Display::None, // デフォルトは Soul モード
            ..default()
        },
        BuildSectionNode,
    ))
    .with_children(|s| {
        sec_label(s, "─ 建築種別 ─", font);
        for kind in TestBuildingKind::ALL {
            spawn_btn(
                s,
                VisualTestAction::SetBuildingKind(kind),
                kind.label(),
                Val::Percent(49.0),
                font,
            );
        }

        sec_label(s, "─ 配置位置 ─", font);
        s.spawn((
            Text::new("(50, 50)"),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(SFONT),
                ..default()
            },
            TextColor(VAL_COL),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            DynamicTextKind::CursorPos,
        ));
        spawn_btn(
            s,
            VisualTestAction::PlaceOrRemove,
            "配置/削除 [Enter]",
            Val::Percent(100.0),
            font,
        );
        spawn_btn(
            s,
            VisualTestAction::RemoveAllBuildings,
            "全削除 [Del]",
            Val::Percent(100.0),
            font,
        );
    });
}
