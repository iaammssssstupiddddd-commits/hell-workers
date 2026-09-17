use bevy::prelude::*;

use crate::app_contexts::{BuildContext, CompanionPlacementState, TaskContext, ZoneContext};
use crate::systems::command::{TaskMode, to_task_mode_zone_type};
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use hw_core::game_state::PlayMode;
use hw_ui::components::MenuState;

pub(super) fn toggle_menu_and_reset_mode(
    menu_state: &mut MenuState,
    target: MenuState,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
    clear_task_context: bool,
) {
    let current = *menu_state;
    *menu_state = if std::mem::discriminant(&current) == std::mem::discriminant(&target) {
        MenuState::Hidden
    } else {
        target
    };

    build_context.0 = None;
    zone_context.0 = None;
    if clear_task_context {
        task_context.0 = TaskMode::None;
    }
    next_play_mode.set(PlayMode::Normal);
}

pub(super) fn set_build_mode(
    kind: BuildingType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    if kind == BuildingType::Wall {
        build_context.0 = None;
        zone_context.0 = None;
        task_context.0 = TaskMode::WallPlace(None);
        next_play_mode.set(PlayMode::FloorPlace);
        return;
    }

    zone_context.0 = None;
    task_context.0 = TaskMode::None;
    build_context.0 = Some(kind);
    next_play_mode.set(PlayMode::BuildingPlace);
}

pub(super) fn set_floor_place_mode(
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = None;
    task_context.0 = TaskMode::FloorPlace(None);
    next_play_mode.set(PlayMode::FloorPlace);
}

pub(super) fn set_zone_mode(
    kind: ZoneType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = Some(kind);
    task_context.0 = TaskMode::ZonePlacement(to_task_mode_zone_type(kind), None);
    next_play_mode.set(PlayMode::TaskDesignation);
}

pub(super) fn set_zone_removal_mode(
    kind: ZoneType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = Some(kind); // 削除モードでも一応セットしておく
    task_context.0 = TaskMode::ZoneRemoval(to_task_mode_zone_type(kind), None);
    next_play_mode.set(PlayMode::TaskDesignation);
}

pub(super) fn set_task_mode(
    mode: TaskMode,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = None;
    task_context.0 = mode;
    next_play_mode.set(PlayMode::TaskDesignation);
}

pub(super) fn set_area_task_mode(
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    let mode = TaskMode::AreaSelection(None);
    build_context.0 = None;
    zone_context.0 = None;
    task_context.0 = mode;
    next_play_mode.set(PlayMode::TaskDesignation);
}

pub(super) struct ModeCtxRefs<'a> {
    pub play_mode: &'a PlayMode,
    pub build_context: &'a BuildContext,
    pub companion_state: &'a CompanionPlacementState,
    pub task_context: &'a TaskContext,
}

pub(super) struct ModeDisplayInfo<'a> {
    pub selected_familiar_name: Option<&'a str>,
    pub selected_area_size_tiles: Option<UVec2>,
    pub area_edit_dragging: bool,
    pub area_edit_operation: Option<&'a str>,
    pub area_overlap: Option<(usize, f32)>,
    pub clipboard_has_area: bool,
    pub unassigned_tasks_in_area: Option<usize>,
}

pub(super) fn next_operation_guidance(play_mode: &PlayMode, task: &TaskMode) -> &'static str {
    match play_mode {
        PlayMode::Normal => "",
        PlayMode::BuildingPlace => "左クリックで配置 · Escで操作を終了",
        PlayMode::BuildingMove => "左クリックで移動先を決定 · Escで中止",
        PlayMode::FloorPlace => "ドラッグして離すと施工予定を作成 · Escで中止",
        PlayMode::TaskDesignation => match task {
            TaskMode::AreaSelection(_) => "ドラッグして離すと担当範囲を適用 · Escで操作を終了",
            TaskMode::DesignateDeconstruct(_) => {
                "建物を選び、返却資材を確認して離すと解体 · Escで中止"
            }
            TaskMode::SoulSpaPlace(_) => "2×2の配置先を選んで左クリック / Escで終了",
            TaskMode::ZonePlacement(_, _) => {
                "左ドラッグ → 採用・除外数を確認 → 離して確定 / Escで未確定の指定を中止"
            }
            TaskMode::ZoneRemoval(_, _) => {
                "左ドラッグで削除範囲を指定 → 離して確定 / Escで未確定の指定を中止"
            }
            TaskMode::StockpilePolicyEdit(_) => {
                "左ドラッグで対象範囲を指定 → 離して方針を適用 / Escで中止"
            }
            TaskMode::DreamPlanting(_) => {
                "左ドラッグで植樹範囲を指定 → 離して適用 / Escで未確定の指定を中止"
            }
            _ => {
                "左クリックまたはドラッグで対象指定 → 離して適用 / 結果は通知履歴で確認 / Escで終了"
            }
        },
    }
}

pub(super) fn build_mode_text(ctx: ModeCtxRefs, info: ModeDisplayInfo) -> String {
    match ctx.play_mode {
        PlayMode::Normal => String::new(),
        PlayMode::BuildingPlace => {
            if ctx.companion_state.0.is_some() {
                return "バケツ置き場を配置 · 水タンクの付属設備".into();
            }
            let kind = ctx.build_context.0;
            kind.map(|kind| {
                let (name, role) = hw_ui::catalog::building_copy(kind);
                format!("{name}を配置 · {role}")
            })
            .unwrap_or_else(|| "建築物を配置".into())
        }
        PlayMode::BuildingMove => "建築物を移動".into(),
        PlayMode::FloorPlace => match ctx.task_context.0 {
            TaskMode::WallPlace(_) => "壁を配置 · 幅1タイルの直線".into(),
            _ => "床を配置 · 部屋の土台".into(),
        },
        PlayMode::TaskDesignation => match ctx.task_context.0 {
            TaskMode::DesignateChop(_) => "伐採を指示".into(),
            TaskMode::DesignateMine(_) => "採掘を指示".into(),
            TaskMode::DesignateHaul(_) => "運搬を指示".into(),
            TaskMode::CancelDesignation(_) => "作業指示を取消".into(),
            TaskMode::DesignateDeconstruct(_) => "完成した建物を解体".into(),
            TaskMode::AreaSelection(_) => {
                let target = info.selected_familiar_name.unwrap_or("使い魔を選択");
                let size = info
                    .selected_area_size_tiles
                    .map(|size| format!("{}×{}タイル", size.x, size.y))
                    .unwrap_or_else(|| "新規範囲".into());
                let state = if info.area_edit_dragging {
                    info.area_edit_operation.unwrap_or("ドラッグ中")
                } else {
                    "範囲編集"
                };
                let overlap = info
                    .area_overlap
                    .filter(|(count, _)| *count > 0)
                    .map(|(count, ratio)| format!(" · 重複{count}（最大{:.0}%）", ratio * 100.0))
                    .unwrap_or_default();
                let tasks = info
                    .unassigned_tasks_in_area
                    .map(|count| format!(" · 未担当{count}件"))
                    .unwrap_or_default();
                let clipboard = if info.clipboard_has_area {
                    " · コピーあり"
                } else {
                    ""
                };
                format!("{target} · {state} · {size}{overlap}{tasks}{clipboard}")
            }
            TaskMode::ZonePlacement(kind, _) => match kind {
                hw_core::game_state::TaskModeZoneType::Stockpile => "保管場所を設定".into(),
                hw_core::game_state::TaskModeZoneType::Yard => "Yardを拡張".into(),
            },
            TaskMode::ZoneRemoval(_, _) => "保管範囲を削除".into(),
            TaskMode::DreamPlanting(_) => "Dreamで植樹".into(),
            TaskMode::StockpilePolicyEdit(_) => "保管方針を範囲に適用".into(),
            TaskMode::SoulSpaPlace(_) => "Soul Spaを配置 · 2×2タイル".into(),
            TaskMode::AssignTask(_) | TaskMode::SelectBuildTarget => "担当する仕事を選択".into(),
            _ => "仕事を指定".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_placement_flow_does_not_require_zone_place_state() {
        let mut next_play_mode = NextState::<PlayMode>::Unchanged;
        let mut build_context = BuildContext::default();
        let mut zone_context = ZoneContext::default();
        let mut task_context = TaskContext::default();

        set_zone_mode(
            ZoneType::Stockpile,
            &mut next_play_mode,
            &mut build_context,
            &mut zone_context,
            &mut task_context,
        );

        assert!(matches!(
            next_play_mode,
            NextState::Pending(PlayMode::TaskDesignation)
                | NextState::PendingIfNeq(PlayMode::TaskDesignation)
        ));
        assert_eq!(zone_context.0, Some(ZoneType::Stockpile));
        assert_eq!(
            task_context.0,
            TaskMode::ZonePlacement(hw_core::game_state::TaskModeZoneType::Stockpile, None)
        );
        assert!(build_context.0.is_none());
    }
}
