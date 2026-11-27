//! Contract card, field marker and receiving-panel controls share the same installation identity.
use super::waterworks::{ContractDistrictGizmos, ContractPage, date, show};
use crate::systems::reception::{OfferSite, PublishedOffer};
use bevy::{ecs::system::SystemParam, prelude::*};
use hw_core::{
    GameTime, WorldEpoch, constants::TILE_SIZE, logistics::ResourceType, soul::DreamPool,
};
use hw_jobs::{
    Blueprint, Building,
    reception::{ReceivingPanel, ReceptionContract},
};
use hw_ui::{
    UiIntent,
    components::{LeftPanelMode, MenuButton},
    waterworks::{
        ContractControl, ContractDescription, ContractTitle, ContractTodoPanel, PanelDetailReport,
    },
};
use hw_world::WorldMap;

type Panels<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ReceivingPanel,
        Option<&'static Blueprint>,
        Option<&'static Building>,
    ),
>;
#[derive(SystemParam)]
pub(crate) struct SiteView<'w, 's> {
    offers: OfferSite<'w, 's>,
    clock: Res<'w, GameTime>,
    epoch: Res<'w, WorldEpoch>,
    pool: Res<'w, DreamPool>,
    contracts: Query<'w, 's, &'static ReceptionContract>,
    panels: Panels<'w, 's>,
    shell: Res<'w, hw_ui::shell::UiShellState>,
    mode: Res<'w, LeftPanelMode>,
    task: Res<'w, crate::app_contexts::TaskContext>,
}
#[derive(SystemParam)]
pub(crate) struct Widgets<'w, 's> {
    title: Query<'w, 's, (&'static mut Text, &'static ContractTitle), Without<ContractDescription>>,
    text: Query<'w, 's, (&'static mut Text, &'static ContractDescription), Without<ContractTitle>>,
    buttons: Query<'w, 's, (&'static mut MenuButton, &'static mut Node), With<ContractControl>>,
    hud: Query<'w, 's, &'static mut Node, (With<ContractTodoPanel>, Without<ContractControl>)>,
}

pub(crate) fn present(
    page: Res<ContractPage>,
    site: SiteView,
    mut widgets: Widgets,
    mut published: ResMut<PublishedOffer>,
    real: Res<Time<Real>>,
    mut next_survey: Local<f64>,
) {
    if page.is_changed() {
        let title = if page.reception {
            "川辺の夜普請"
        } else {
            "第一工区の給水所"
        };
        for (mut text, id) in &mut widgets.title {
            text.0 = if id.0 {
                format!("契約ToDo　{title}")
            } else {
                title.into()
            };
        }
        *next_survey = 0.;
    }
    if !page.reception {
        return;
    }
    let record = site.contracts.single().ok();
    let card = site.shell.management_open() && *site.mode == LeftPanelMode::Contracts;
    let hud = record.is_some()
        && site.shell.page == hw_ui::shell::WorkspacePage::World
        && !matches!(site.task.0, hw_core::game_state::TaskMode::AreaSelection(_));
    for mut n in &mut widgets.hud {
        show(&mut n, hud);
    }
    if !card && !hud {
        return;
    }
    if record.is_none() && real.elapsed_secs_f64() >= *next_survey {
        published.0 = Some(site.offers.terms());
        *next_survey = real.elapsed_secs_f64() + 0.5;
    }
    let terms = record
        .map(|c| c.terms)
        .or_else(|| published.0.and_then(Result::ok));
    let panel = site.panels.single().ok();
    let progress = match (record, panel) {
        (None, _) => published
            .0
            .and_then(Result::err)
            .unwrap_or("契約図を確認して受注してください。")
            .to_owned(),
        (Some(_), None) => "貸与盤1面　A点に指定盤を施工してください。台木は木材2本。".into(),
        (_, Some((_, _, Some(bp), _))) => format!(
            "施工中　台木 {}/2　据付 {:.0}%",
            bp.delivered_materials
                .get(&ResourceType::Wood)
                .unwrap_or(&0),
            bp.progress * 100.
        ),
        (_, Some((_, p, _, _))) if p.live => "外部受電中　使用 0 W".into(),
        (_, Some((_, p, _, _))) if p.reported => "構内据付済み・連絡済み　外部供給待ち".into(),
        _ => "指定点・盤仕様・据付 照合済み　準備完了の連絡待ち".into(),
    };
    let timing = record
        .and_then(|r| {
            r.first_received_at.map(|at| {
                format!(
                    "初回受電 {}（{}）",
                    date(at),
                    if at <= r.terms.deadline {
                        "期限内"
                    } else {
                        "遅延"
                    }
                )
            })
        })
        .unwrap_or_else(|| "初回受電　未記録".into());
    let supplier = terms.map_or("", |t| {
        if site.clock.seconds >= t.scheduled_at {
            "引込準備済み"
        } else {
            "引込準備中"
        }
    });
    let detail = match terms {
        None => format!("河岸の荷受け場の仮設受電工事\n{progress}"),
        Some(t) => {
            let progress = if record.is_none() && site.pool.points < t.setup_fee {
                "共有Dreamが不足しています"
            } else {
                &progress
            };
            format!(
                "工事請負契約　A点 ({}, {})／仮設受電盤1面／{:.1} W\n王側の引込予定 {}（{supplier}）\n受電期限 {}　引渡期限 {}\n貸与盤の現場渡し・木材2　受注時の定額費 {} Dream\n{progress}\n{timing}",
                t.point.x,
                t.point.y,
                t.capacity_w,
                date(t.scheduled_at),
                date(t.deadline),
                date(t.handover_deadline),
                t.setup_fee
            )
        }
    };
    let todo = format!("{progress}\n{timing}");
    for (mut text, id) in &mut widgets.text {
        let label = match id {
            ContractDescription::Card => &detail,
            ContractDescription::Todo => &todo,
        };
        if text.0 != *label {
            text.0.clone_from(label)
        }
    }
    for (mut control, mut node) in &mut widgets.buttons {
        let visible = match control.0 {
            UiIntent::SelectReceptionContract(reception) => !reception,
            UiIntent::LocateReceivingPanel => terms.is_some(),
            UiIntent::AcceptReception => {
                record.is_none() && terms.is_some_and(|t| site.pool.points >= t.setup_fee)
            }
            UiIntent::PlaceReceivingPanel => record.is_some() && panel.is_none(),
            UiIntent::ReportReceivingPanel { .. } => {
                if let Some((target, p, None, Some(_))) = panel {
                    control.set_if_neq(MenuButton(UiIntent::ReportReceivingPanel {
                        target,
                        designation: p.designation,
                        epoch: site.epoch.get(),
                    }));
                    !p.reported
                } else {
                    false
                }
            }
            _ => false,
        };
        show(&mut node, visible);
    }
}

type DetailStatusFilter = (
    With<hw_ui::waterworks::PanelDetailStatus>,
    Without<PanelDetailReport>,
);

#[derive(SystemParam)]
pub(crate) struct PanelDetailWidgets<'w, 's> {
    status: Query<'w, 's, (&'static mut Text, &'static mut Node), DetailStatusFilter>,
    controls: Query<'w, 's, (&'static mut MenuButton, &'static mut Node), With<PanelDetailReport>>,
}

pub(crate) fn detail(
    shell: Res<hw_ui::shell::UiShellState>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    pin: Res<hw_ui::panels::info_panel::InfoPanelPinState>,
    panels: Query<&ReceivingPanel, With<Building>>,
    epoch: Res<WorldEpoch>,
    contracts: Query<&ReceptionContract>,
    mut widgets: PanelDetailWidgets,
) {
    let target = shell
        .inspected_entity(selected.0, pin.entity)
        .filter(|_| shell.page == hw_ui::shell::WorkspacePage::Inspector)
        .and_then(|e| panels.get(e).ok().map(|p| (e, p)));
    for (mut text, mut node) in &mut widgets.status {
        show(&mut node, target.is_some());
        if let Some((_, panel)) = target
            && let Ok(record) = contracts.single()
        {
            let state = if panel.live {
                "外部受電中　使用 0 W"
            } else if panel.reported {
                "連絡済み　外部供給待ち"
            } else {
                "据付済み　準備完了の連絡待ち"
            };
            let receipt = record
                .first_received_at
                .map(date)
                .unwrap_or_else(|| "未記録".into());
            let label = format!("契約指定盤 A　{state}\n初回受電　{receipt}");
            if text.0 != label {
                text.0 = label;
            }
        }
    }
    for (mut button, mut node) in &mut widgets.controls {
        if let Some((target, p)) = target {
            button.set_if_neq(MenuButton(UiIntent::ReportReceivingPanel {
                target,
                designation: p.designation,
                epoch: epoch.get(),
            }));
        }
        show(&mut node, target.is_some_and(|(_, p)| !p.reported));
    }
}

pub(crate) fn draw(
    page: Res<ContractPage>,
    offer: Res<PublishedOffer>,
    contracts: Query<&ReceptionContract>,
    panels: Query<&ReceivingPanel, With<Building>>,
    mut gizmos: Gizmos<ContractDistrictGizmos>,
) {
    let Some(t) = contracts
        .single()
        .ok()
        .map(|c| c.terms)
        .or_else(|| offer.0.and_then(Result::ok))
    else {
        return;
    };
    let pos = WorldMap::grid_to_world(t.point.x, t.point.y);
    let live = panels.single().is_ok_and(|p| p.live);
    if live {
        gizmos.circle_2d(
            pos + Vec2::new(0.15, 0.20) * TILE_SIZE,
            TILE_SIZE * 0.09,
            Color::srgb(0.50, 1., 0.58),
        );
    }
    if !page.reception || !page.show_point {
        return;
    }
    let gold = Color::srgb(1., 0.83, 0.30);
    gizmos.rect_2d(pos, Vec2::splat(TILE_SIZE), gold);
    let a = pos + Vec2::Y * TILE_SIZE * 0.82;
    for (u, v) in [
        ((-0.19, -0.21), (0., 0.21)),
        ((0., 0.21), (0.19, -0.21)),
        ((-0.1, -0.03), (0.1, -0.03)),
    ] {
        gizmos.line_2d(
            a + Vec2::from(u) * TILE_SIZE,
            a + Vec2::from(v) * TILE_SIZE,
            gold,
        );
    }
    let begin = pos - Vec2::X * (TILE_SIZE * 2.);
    if live {
        gizmos.line_2d(begin, pos, gold);
    } else {
        for i in 0..6 {
            gizmos.line_2d(
                begin.lerp(pos, i as f32 / 6.),
                begin.lerp(pos, (i as f32 + 0.55) / 6.),
                gold,
            );
        }
    }
}
