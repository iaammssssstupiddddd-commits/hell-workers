//! Optional guide observes committed gameplay; it never issues gameplay commands.
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};
use hw_core::{
    WorldEpoch,
    area::TaskArea,
    familiar::Familiar,
    relationships::{CommandedBy, ManagedBy},
};
use hw_jobs::{
    AssignedTask, Designation, GatherPhase, PlayerIssuedDesignation, Rock, Tree, WorkType,
};
use hw_ui::UiIntent;
use hw_ui::components::{MenuButton, UiInputBlocker, UiRoot};

#[derive(Resource, Default, Debug)]
pub(crate) struct WorkGuide {
    active: bool,
    epoch: WorldEpoch,
    familiar: Option<Entity>,
    target: Option<Entity>,
    started: bool,
    completed: bool,
    text: String,
}

impl WorkGuide {
    pub(crate) fn start(&mut self, epoch: WorldEpoch) {
        *self = Self {
            active: true,
            epoch,
            ..default()
        };
    }
    pub(crate) fn end(&mut self) {
        *self = default();
    }
    fn forget_target(&mut self) {
        self.target = None;
        self.started = false;
        self.completed = false;
    }
}

pub(crate) fn reset(world: &mut World) {
    if let Some(mut guide) = world.get_resource_mut::<WorkGuide>() {
        guide.end();
    }
    let mut panels = world.query_filtered::<Entity, With<GuidePanel>>();
    let panels: Vec<_> = panels.iter(world).collect();
    for panel in panels {
        world.despawn(panel);
    }
}

type GatherTargets<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        &'static ManagedBy,
        Has<Tree>,
        Has<Rock>,
    ),
    With<PlayerIssuedDesignation>,
>;

pub(crate) fn update(
    mut guide: ResMut<WorkGuide>,
    epoch: Res<WorldEpoch>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    familiars: Query<(&Familiar, Option<&TaskArea>)>,
    targets: GatherTargets,
    tasks: Query<(&AssignedTask, &CommandedBy)>,
    time: Res<Time<Virtual>>,
) {
    if guide.epoch != *epoch {
        guide.end();
    }
    if !guide.active {
        return;
    }
    if guide
        .familiar
        .is_some_and(|entity| !familiars.contains(entity))
    {
        guide.familiar = None;
        guide.forget_target();
    }
    if guide.familiar.is_none() {
        guide.familiar = selected.0.filter(|entity| familiars.contains(*entity));
    }
    let Some(familiar_entity) = guide.familiar else {
        guide.text = "1/5 使い魔を選択\n左のEntities一覧、またはワールド上の使い魔を選んでください。使い魔がいなければ、用意できるまでガイドを閉じて構いません。".into();
        return;
    };
    let Ok((familiar, area)) = familiars.get(familiar_entity) else {
        return;
    };
    let prefix = format!("操作ガイド — {}\n", familiar.name);
    let Some(area) = area else {
        guide.forget_target();
        guide.text = format!(
            "{prefix}2/5 担当範囲を確定\nこの使い魔を選び、OrdersのAreaから範囲をドラッグして離してください。木・岩と働けるSoulが入る範囲を選びます。"
        );
        return;
    };
    // Done must be observed before liveness: mining despawns its target in
    // the same successful execution that publishes GatherPhase::Done.
    if let Some(target) = guide.target {
        for (task, owner) in &tasks {
            if owner.0 != familiar_entity {
                continue;
            }
            if let AssignedTask::Gather(data) = task
                && data.target == target
            {
                match data.phase {
                    GatherPhase::Done => {
                        guide.started = true;
                        guide.completed = true;
                    }
                    GatherPhase::Collecting { .. } => guide.started = true,
                    GatherPhase::GoingToResource => {}
                }
            }
        }
        if !guide.completed
            && !targets
                .get(target)
                .is_ok_and(|(_, transform, designation, owner, tree, rock)| {
                    owner.0 == familiar_entity
                        && area.contains(transform.translation.truncate())
                        && ((tree && designation.work_type == WorkType::Chop)
                            || (rock && designation.work_type == WorkType::Mine))
                })
        {
            guide.forget_target();
        }
    }
    if guide.target.is_none() {
        guide.target = targets
            .iter()
            .filter(|(_, transform, designation, owner, tree, rock)| {
                owner.0 == familiar_entity
                    && area.contains(transform.translation.truncate())
                    && ((*tree && designation.work_type == WorkType::Chop)
                        || (*rock && designation.work_type == WorkType::Mine))
            })
            .map(|(entity, ..)| entity)
            .min_by_key(|entity| entity.to_bits());
    }
    let instruction = if guide.completed {
        "5/5 作業完了\n資源の採取が完了しました。担当範囲と作業指定を整えると、使い魔とSoulが仕事を進めます。ガイドを閉じて続けてください。"
    } else if guide.started {
        "4/5 作業開始を確認\nSoulが採取を開始しました。完了まで待ちます。中断した場合は、Tasksの停止理由とSoulの状態を確認してください。"
    } else if guide.target.is_some() {
        "4/5 Soulの作業開始を待機\nTasksで担当と停止理由を確認できます。働けるSoul、使い魔の仕事設定、対象への通路を確認してください。"
    } else {
        "3/5 伐採または採掘を指定\nこの使い魔を選び、OrdersのChopまたはMineから担当範囲内の未指定の木・岩を囲んで離してください。"
    };
    guide.text = format!(
        "{prefix}{instruction}{}",
        if time.is_paused() && guide.target.is_some() && !guide.completed {
            "\n時間停止中です。作業を進めるにはSpaceで再開します。"
        } else {
            ""
        }
    );
}

#[derive(Component)]
pub(crate) struct GuidePanel;
#[derive(Component)]
pub(crate) struct GuideText;
#[derive(Component)]
struct GuideScroll;

#[derive(SystemParam)]
pub(crate) struct GuideContentQueries<'w, 's> {
    text: Query<'w, 's, &'static mut Text, With<GuideText>>,
    scroll: Query<'w, 's, &'static mut ScrollPosition, With<GuideScroll>>,
}

pub(crate) fn present(
    mut commands: Commands,
    guide: Res<WorkGuide>,
    roots: Query<Entity, With<UiRoot>>,
    panels: Query<Entity, With<GuidePanel>>,
    mut content: GuideContentQueries,
    assets: Option<Res<crate::assets::GameAssets>>,
    theme: Res<hw_ui::theme::UiTheme>,
) {
    if !guide.active {
        for panel in &panels {
            commands.entity(panel).despawn();
        }
        return;
    }
    let updated = content.text.single_mut().ok().map(|mut text| {
        let changed = text.0 != guide.text;
        if changed {
            text.0.clone_from(&guide.text);
        }
        changed
    });
    if let Some(updated) = updated {
        if updated {
            for mut scroll in &mut content.scroll {
                scroll.0 = Vec2::ZERO;
            }
        }
        return;
    }
    let (Ok(root), Some(assets)) = (roots.single(), assets) else {
        return;
    };
    let panel = commands
        .spawn((
            GuidePanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(94.0),
                left: Val::Percent(35.0),
                width: Val::Px(350.0),
                max_width: Val::Percent(40.0),
                max_height: Val::Percent(60.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            GlobalZIndex(155),
            UiInputBlocker,
            RelativeCursorPosition::default(),
            Interaction::default(),
        ))
        .id();
    commands.entity(root).add_child(panel);
    commands.entity(panel).with_children(|parent| {
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|row| {
                let scroll = row
                    .spawn((
                        ScrollArea,
                        GuideScroll,
                        UiInputBlocker,
                        RelativeCursorPosition::default(),
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            min_height: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::scroll_y(),
                            padding: UiRect::right(Val::Px(8.0)),
                            ..default()
                        },
                        Name::new("Work Guide Scroll Area"),
                    ))
                    .with_children(|body| {
                        body.spawn((
                            GuideText,
                            Text::new(&guide.text),
                            TextFont {
                                font: assets.font_ui.clone().into(),
                                font_size: FontSize::Px(theme.typography.font_size_base),
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                            TextLayout::linebreak(LineBreak::WordOrCharacter),
                            Node {
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                    })
                    .id();
                row.spawn((
                    Node {
                        width: Val::Px(6.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Scrollbar::new(scroll, ControlOrientation::Vertical, 20.0),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        ScrollbarThumb {
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            border: UiRect::ZERO,
                        },
                        BackgroundColor(theme.colors.text_muted),
                    ));
                });
            });
        for label in ["ガイドを閉じる", "スキップして終了"] {
            parent
                .spawn((
                    Button,
                    MenuButton(UiIntent::EndWorkGuide),
                    Node {
                        min_height: Val::Px(32.0),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label),
                        TextFont {
                            font: assets.font_ui.clone().into(),
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(theme.colors.text_primary_semantic),
                    ));
                });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::GatherData;

    #[test]
    fn guide_preserves_reading_position_until_instruction_changes() {
        let mut app = App::new();
        app.insert_resource(WorkGuide {
            active: true,
            text: "現在の案内".into(),
            ..default()
        })
        .init_resource::<hw_ui::theme::UiTheme>()
        .add_systems(Update, present);
        app.world_mut().spawn((GuideText, Text::new("現在の案内")));
        let scroll = app
            .world_mut()
            .spawn((GuideScroll, ScrollPosition(Vec2::new(0.0, 80.0))))
            .id();
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(scroll).unwrap().0.y, 80.0);
        app.world_mut().resource_mut::<WorkGuide>().text = "次の案内".into();
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(scroll).unwrap().0,
            Vec2::ZERO
        );
    }

    fn app() -> App {
        let mut app = crate::test_support::minimal_app();
        app.init_resource::<WorkGuide>()
            .init_resource::<WorldEpoch>()
            .init_resource::<crate::interface::selection::SelectedEntity>()
            .init_resource::<Time<Virtual>>()
            .add_systems(Update, update);
        app.world_mut()
            .resource_mut::<WorkGuide>()
            .start(WorldEpoch::default());
        app
    }

    #[test]
    fn guide_observes_owner_area_and_actual_work_and_handles_successful_despawn() {
        let mut app = app();
        app.update();
        assert!(app.world().resource::<WorkGuide>().text.starts_with("1/5"));
        let familiar = app.world_mut().spawn(Familiar::default()).id();
        let other = app.world_mut().spawn(Familiar::default()).id();
        app.world_mut()
            .resource_mut::<crate::interface::selection::SelectedEntity>()
            .0 = Some(familiar);
        app.update();
        assert!(app.world().resource::<WorkGuide>().text.contains("2/5"));
        app.world_mut()
            .entity_mut(familiar)
            .insert(TaskArea::from_points(Vec2::ZERO, Vec2::splat(100.0)));
        let target = app
            .world_mut()
            .spawn((
                Rock,
                Transform::from_xyz(50.0, 50.0, 0.0),
                Designation {
                    work_type: WorkType::Mine,
                },
                PlayerIssuedDesignation,
                ManagedBy(other),
            ))
            .id();
        app.update();
        assert!(app.world().resource::<WorkGuide>().target.is_none());
        app.world_mut()
            .entity_mut(target)
            .insert(ManagedBy(familiar));
        app.update();
        assert_eq!(app.world().resource::<WorkGuide>().target, Some(target));
        let worker = app
            .world_mut()
            .spawn((
                CommandedBy(other),
                AssignedTask::Gather(GatherData {
                    target,
                    work_type: WorkType::Mine,
                    phase: GatherPhase::Collecting { progress: 0.5 },
                }),
            ))
            .id();
        app.update();
        assert!(!app.world().resource::<WorkGuide>().started);
        app.world_mut()
            .entity_mut(worker)
            .insert(CommandedBy(familiar));
        app.update();
        assert!(app.world().resource::<WorkGuide>().started);
        assert!(!app.world().resource::<WorkGuide>().completed);
        app.world_mut().despawn(target);
        app.world_mut()
            .entity_mut(worker)
            .insert(AssignedTask::Gather(GatherData {
                target,
                work_type: WorkType::Mine,
                phase: GatherPhase::Done,
            }));
        app.update();
        assert!(app.world().resource::<WorkGuide>().completed);
        app.world_mut()
            .entity_mut(worker)
            .insert(AssignedTask::None);
        app.update();
        assert!(app.world().resource::<WorkGuide>().completed);
    }

    #[test]
    fn guide_returns_to_lost_step_and_ends_on_world_replace_or_close() {
        let mut app = app();
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                TaskArea::from_points(Vec2::ZERO, Vec2::splat(100.0)),
            ))
            .id();
        app.world_mut()
            .resource_mut::<crate::interface::selection::SelectedEntity>()
            .0 = Some(familiar);
        let target = app
            .world_mut()
            .spawn((
                Tree,
                Transform::from_xyz(50.0, 50.0, 0.0),
                Designation {
                    work_type: WorkType::Chop,
                },
                PlayerIssuedDesignation,
                ManagedBy(familiar),
            ))
            .id();
        app.update();
        app.world_mut().despawn(target);
        app.update();
        assert!(app.world().resource::<WorkGuide>().text.contains("3/5"));
        app.world_mut().entity_mut(familiar).remove::<TaskArea>();
        app.update();
        assert!(app.world().resource::<WorkGuide>().text.contains("2/5"));
        app.world_mut().despawn(familiar);
        app.update();
        assert!(app.world().resource::<WorkGuide>().familiar.is_none());
        app.world_mut().resource_mut::<WorldEpoch>().advance();
        app.update();
        assert!(!app.world().resource::<WorkGuide>().active);
        let epoch = *app.world().resource::<WorldEpoch>();
        app.world_mut().resource_mut::<WorkGuide>().start(epoch);
        reset(app.world_mut());
        assert!(!app.world().resource::<WorkGuide>().active);
    }
}
