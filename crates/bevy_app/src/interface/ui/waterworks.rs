use crate::{
    app_contexts::BuildContext,
    systems::waterworks::*,
    world::map::{GeneratedWorldLayoutResource, WorldMap},
};
use bevy::ecs::system::SystemParam;
use bevy::gizmos::config::GizmoConfigGroup;
use bevy::prelude::*;
use hw_core::{GameTime, constants::TILE_SIZE};
use hw_jobs::BuildingType;
use hw_ui::{
    UiIntent,
    components::{LeftPanelMode, MenuButton},
    waterworks::{ContractControl, ContractDescription, ContractTodoPanel},
};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub(crate) struct ContractDistrictGizmos;

#[derive(Resource, Default)]
pub(crate) struct ContractOverlay(pub bool);

#[derive(Resource, Default)]
pub(crate) struct ContractPage {
    pub reception: bool,
    pub show_point: bool,
}

pub fn reset(world: &mut World) {
    world.insert_resource(ContractOverlay::default());
    world.insert_resource(ContractPage::default());
    world.insert_resource(crate::systems::reception::PublishedOffer::default());
}

pub fn draw(
    layer: Res<ContractOverlay>,
    tool: Res<BuildContext>,
    page: Res<ContractPage>,
    layout: Res<GeneratedWorldLayoutResource>,
    contract: Query<&WaterworksContract>,
    camera: Query<&Projection, With<hw_ui::camera::MainCamera>>,
    mut gizmos: Gizmos<ContractDistrictGizmos>,
) {
    let placing_tank = tool.kind == Some(BuildingType::Tank)
        && contract
            .single()
            .is_ok_and(|c| c.outcome == ContractOutcome::Active);
    if !placing_tank && (page.reception || !layer.0) {
        return;
    }
    let district = contract
        .single()
        .map_or_else(|_| layout.layout.anchors.yard.into(), |c| c.district);
    let min =
        WorldMap::grid_to_world(district.min.x, district.min.y) - Vec2::splat(TILE_SIZE * 0.5);
    let max =
        WorldMap::grid_to_world(district.max.x, district.max.y) + Vec2::splat(TILE_SIZE * 0.5);
    let scale = match camera.single() {
        Ok(Projection::Orthographic(projection)) => projection.scale,
        _ => 1.0,
    };
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    let gold = Color::srgb(1.0, 0.78, 0.16);
    let ink = Color::srgb(0.15, 0.085, 0.035);
    for (a, b) in corners.into_iter().zip(corners.into_iter().cycle().skip(1)) {
        let along = (b - a).normalize();
        let inward = along.perp();
        let edge = inward * (3.0 * scale);
        gizmos.line_2d(a + edge, b + edge, ink);
        gizmos.line_2d(a, b, gold);
        // Inward ticks identify the buildable side even when the Yard shares this edge.
        let step = (TILE_SIZE * 2.0).max(22.0 * scale);
        let mut offset = step * 0.5;
        while offset < a.distance(b) {
            let p = a + along * offset;
            gizmos.line_2d(p + edge, p + inward * (9.0 * scale), gold);
            offset += step;
        }
    }
}

pub fn finish_placement(
    contracts: Query<&WaterworksContract>,
    mut tool: ResMut<BuildContext>,
    mut companion: ResMut<crate::app_contexts::CompanionPlacementState>,
    mut mode: ResMut<NextState<hw_core::game_state::PlayMode>>,
) {
    if tool.kind == Some(BuildingType::Tank)
        && tool.contract_district.is_some()
        && !contracts
            .single()
            .is_ok_and(|c| c.outcome == ContractOutcome::Active)
    {
        *tool = BuildContext::default();
        companion.0 = None;
        mode.set(hw_core::game_state::PlayMode::Normal);
    }
}

pub(super) fn date(seconds: f32) -> String {
    let minutes = (seconds / 60.0) as u32;
    format!(
        "{}日目 {:02}:{:02}",
        minutes / 1440 + 1,
        minutes / 60 % 24,
        minutes % 60
    )
}

fn checklist(contract: &WaterworksContract, progress: StationProgress) -> String {
    let inspected = !matches!(contract.outcome, ContractOutcome::Active);
    let placed = inspected || progress != StationProgress::Site;
    let built = inspected || progress == StationProgress::Water;
    let mark = |done| if done { "済" } else { "□" };
    let build = match progress {
        StationProgress::Building { wood, .. } if wood < 2 => format!("木材を搬入　{wood}/2"),
        StationProgress::Building { percent, .. } => format!("タンクを竣工　{percent}%"),
        _ => "タンクを竣工".into(),
    };
    let (paid, claim) = match contract.outcome {
        ContractOutcome::Inspected { reward, .. } => {
            (false, format!("管理 → 契約で {reward} Dream を受領"))
        }
        ContractOutcome::Paid { reward, .. } => (true, format!("{reward} Dream 受領済み")),
        ContractOutcome::Active => (false, "報酬を受領".into()),
    };
    format!(
        "{}　工区内にタンクと置場を配置\n{}　{build}\n{}　水1杯（5単位）を届ける\n{}　{claim}",
        mark(placed),
        mark(built),
        mark(inspected),
        mark(paid)
    )
}

#[derive(SystemParam)]
pub(crate) struct ContractWidgets<'w, 's> {
    page: Res<'w, ContractPage>,
    task: Res<'w, crate::app_contexts::TaskContext>,
    text: Query<'w, 's, (&'static mut Text, &'static ContractDescription)>,
    buttons: Query<'w, 's, (&'static MenuButton, &'static mut Node), With<ContractControl>>,
    hud: Query<'w, 's, &'static mut Node, (With<ContractTodoPanel>, Without<ContractControl>)>,
}

pub(super) fn show(node: &mut Mut<Node>, visible: bool) {
    let display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct CardState {
    contract: Option<WaterworksContract>,
    stage: StationProgress,
    remaining: u32,
    late: bool,
}

impl CardState {
    fn labels(&self) -> (String, String) {
        let Some(contract) = &self.contract else {
            let offer = "第一工区でタンクに水を1杯届けてください。通常の「建てる」や既設のタンクも使えます。本体とバケツ置場を金色の枠内に収めます。新設には木材2個が必要です。";
            let card = format!(
                "依頼主　地獄の王\n{offer}\n期限　受注から{HOURS}時間　報酬　{FULL_REWARD} Dream（遅延 {LATE_REWARD}）"
            );
            return (String::new(), card);
        };
        let todo = checklist(contract, self.stage);
        match contract.outcome {
            ContractOutcome::Active => {
                let timing = if self.late { "期限超過" } else { "締切" };
                let deadline = format!("{timing}　{}", date(contract.deadline));
                let hint = match self.stage {
                    StationProgress::Site => "「建てる」または契約書からタンクを配置できます。",
                    StationProgress::Building { .. } => {
                        "資材が足りないときは、使い魔に伐採を指示してください。"
                    }
                    StationProgress::Water => "使い魔の担当範囲と取水の許可を確認してください。",
                };
                let hud = format!("{todo}\n{deadline}");
                let card = format!(
                    "{todo}\n\n{deadline}　残り {}時間{}分\n報酬　{} Dream（遅延 {}）\n{hint}",
                    self.remaining / 60,
                    self.remaining % 60,
                    contract.full_reward,
                    contract.late_reward
                );
                (hud, card)
            }
            ContractOutcome::Inspected { at, .. } | ContractOutcome::Paid { at, .. } => {
                let timing = if at <= contract.deadline {
                    "期限内"
                } else {
                    "遅延"
                };
                let card = format!("{todo}\n\n検収　{}・{timing}", date(at));
                (todo, card)
            }
        }
    }
}

pub fn present(
    mode: Res<LeftPanelMode>,
    shell: Res<hw_ui::shell::UiShellState>,
    clock: Res<GameTime>,
    contracts: Query<&WaterworksContract>,
    stations: Stations,
    mut widgets: ContractWidgets,
    mut previous: Local<Option<CardState>>,
) {
    if widgets.page.reception {
        *previous = None;
        return;
    }
    let record = contracts.single().ok();
    let outcome = record.map(|c| c.outcome);
    let active = outcome == Some(ContractOutcome::Active);
    let claimable = matches!(outcome, Some(ContractOutcome::Inspected { .. }));
    let show_hud = (active || claimable)
        && shell.page == hw_ui::shell::WorkspacePage::World
        && !matches!(
            widgets.task.0,
            hw_core::game_state::TaskMode::AreaSelection(_)
        );
    for mut node in &mut widgets.hud {
        show(&mut node, show_hud);
    }
    let card = *mode == LeftPanelMode::Contracts && shell.management_open();
    if !show_hud && !card {
        return;
    }

    let stage = record
        .filter(|_| active)
        .map_or(StationProgress::Site, |c| stations.progress(c.district));
    let remaining = record.map_or(0, |c| {
        ((c.deadline - clock.seconds).max(0.) / 60.).ceil() as u32
    });
    let late = record.is_some_and(|c| clock.seconds > c.deadline);
    let state = CardState {
        contract: record.copied(),
        stage,
        remaining: if active { remaining } else { 0 },
        late: active && late,
    };
    if *previous == Some(state) {
        return;
    }
    *previous = Some(state);
    let (hud, description) = state.labels();

    for (mut label, surface) in &mut widgets.text {
        let value = match surface {
            ContractDescription::Todo => &hud,
            ContractDescription::Card => &description,
        };
        if label.0 != *value {
            label.0.clone_from(value);
        }
    }
    for (button, mut node) in &mut widgets.buttons {
        show(
            &mut node,
            match button.0 {
                UiIntent::SelectReceptionContract(show_reception) => show_reception,
                UiIntent::LocateReceivingPanel
                | UiIntent::AcceptReception
                | UiIntent::PlaceReceivingPanel
                | UiIntent::ReportReceivingPanel { .. } => false,
                UiIntent::AcceptWaterworks => record.is_none(),
                UiIntent::PlaceWaterworks => active && stage == StationProgress::Site,
                UiIntent::ClaimWaterworks => claimable,
                UiIntent::ViewWaterworksTasks => active && stage != StationProgress::Site,
                _ => true,
            },
        );
    }
}
