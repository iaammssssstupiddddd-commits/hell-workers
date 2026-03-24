//! 使い魔のリクルート管理モジュール（hw_ai）
//!
//! リクルートの検索・スコアリング・実行ロジックを提供します。
//! 空間グリッドへのアクセスは `SpatialGridOps` トレイト越しに行い、
//! concrete `SpatialGrid` resource への直接依存を持ちません。

use std::collections::HashSet;

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::TILE_SIZE;
use hw_core::familiar::{Familiar, FamiliarAiState, FamiliarOperation};
use hw_core::relationships::RestingIn;
use hw_core::soul::{Destination, IdleBehavior, Path, RestAreaCooldown, StressBreakdown};
use hw_jobs::AssignedTask;
use hw_world::SpatialGridOps;

use super::query_types::SoulRecruitmentQuery;

// ──────────────────────────────────────────────────────────────────────────────
// 定数
// ──────────────────────────────────────────────────────────────────────────────

const RECRUIT_MAX_SEARCH_RADIUS: f32 = TILE_SIZE * 160.0;
const RECRUIT_GOOD_ENOUGH_SCORE: f32 = 0.72;
const RECRUIT_WEIGHT_DISTANCE: f32 = 0.40;
const RECRUIT_WEIGHT_FATIGUE: f32 = 0.30;
const RECRUIT_WEIGHT_DIRECTION: f32 = 0.15;
const RECRUIT_WEIGHT_MOTIVATION: f32 = 0.15;

// ──────────────────────────────────────────────────────────────────────────────
// スコアリング（純粋ロジック）
// ──────────────────────────────────────────────────────────────────────────────

fn score_recruit(
    soul_pos: Vec2,
    fam_pos: Vec2,
    task_area_center: Option<Vec2>,
    fatigue: f32,
    fatigue_threshold: f32,
    motivation: f32,
) -> f32 {
    let max_dist_sq = RECRUIT_MAX_SEARCH_RADIUS * RECRUIT_MAX_SEARCH_RADIUS;
    let dist_sq = soul_pos.distance_squared(fam_pos);
    let dist_score = 1.0 - (dist_sq / max_dist_sq).min(1.0);

    let fatigue_score = if fatigue_threshold > f32::EPSILON {
        ((fatigue_threshold - fatigue).max(0.0) / fatigue_threshold).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let direction_score = if let Some(area_center) = task_area_center {
        let fam_to_area = (area_center - fam_pos).normalize_or_zero();
        let fam_to_soul = (soul_pos - fam_pos).normalize_or_zero();
        ((fam_to_area.dot(fam_to_soul) + 1.0) * 0.5).clamp(0.0, 1.0)
    } else {
        0.5
    };

    let motivation_score = motivation.clamp(0.0, 1.0);

    dist_score * RECRUIT_WEIGHT_DISTANCE
        + fatigue_score * RECRUIT_WEIGHT_FATIGUE
        + direction_score * RECRUIT_WEIGHT_DIRECTION
        + motivation_score * RECRUIT_WEIGHT_MOTIVATION
}

// ──────────────────────────────────────────────────────────────────────────────
// RecruitmentManager
// ──────────────────────────────────────────────────────────────────────────────

/// リクルート管理ユーティリティ
pub struct RecruitmentManager;

impl RecruitmentManager {
    #[allow(clippy::too_many_arguments)]
    /// 条件に合う魂を検索する (リクルート用)
    pub fn find_best_recruit<G: SpatialGridOps>(
        fam_pos: Vec2,
        fatigue_threshold: f32,
        _min_fatigue: f32,
        task_area_center: Option<Vec2>,
        spatial_grid: &G,
        q_souls: &SoulRecruitmentQuery,
        q_breakdown: &Query<&StressBreakdown>,
        q_resting: &Query<(), With<RestingIn>>,
        q_cooldown: &Query<&RestAreaCooldown>,
        scratch: &mut Vec<Entity>,
        radius_opt: Option<f32>,
        excluded: &HashSet<Entity>,
    ) -> Option<Entity> {
        let filter_candidate = |e: Entity| -> Option<(Entity, Vec2, f32, f32)> {
            if excluded.contains(&e) {
                return None;
            }
            let (entity, transform, soul, task, idle, uc) = q_souls.get(e).ok()?;
            let fatigue_ok = soul.fatigue <= fatigue_threshold;
            let stress_ok = q_breakdown.get(entity).is_err();
            let resting_ok = q_resting.get(entity).is_err();
            let cooldown_ok = q_cooldown
                .get(entity)
                .map(|cooldown| cooldown.remaining_secs <= 0.0)
                .unwrap_or(true);

            if uc.is_none()
                && matches!(*task, AssignedTask::None)
                && fatigue_ok
                && stress_ok
                && resting_ok
                && cooldown_ok
                && idle.behavior != IdleBehavior::Resting
                && idle.behavior != IdleBehavior::GoingToRest
                && idle.behavior != IdleBehavior::ExhaustedGathering
            {
                Some((
                    entity,
                    transform.translation.truncate(),
                    soul.fatigue,
                    soul.motivation,
                ))
            } else {
                None
            }
        };

        let find_best_scored =
            |candidates: Vec<(Entity, Vec2, f32, f32)>| -> Option<(Entity, f32)> {
                candidates.into_iter().fold(
                    None,
                    |best, (entity, soul_pos, fatigue, motivation)| {
                        let score = score_recruit(
                            soul_pos,
                            fam_pos,
                            task_area_center,
                            fatigue,
                            fatigue_threshold,
                            motivation,
                        );
                        match best {
                            Some((best_entity, best_score)) if best_score >= score => {
                                Some((best_entity, best_score))
                            }
                            _ => Some((entity, score)),
                        }
                    },
                )
            };

        if let Some(radius) = radius_opt {
            spatial_grid.get_nearby_in_radius_into(fam_pos, radius, scratch);
            let candidates: Vec<_> = scratch
                .iter()
                .filter_map(|&e| filter_candidate(e))
                .collect();
            return find_best_scored(candidates).map(|(entity, _)| entity);
        }

        // radius_opt = None の場合: 段階的に検索半径を拡大
        let search_tiers = [
            TILE_SIZE * 20.0,
            TILE_SIZE * 40.0,
            TILE_SIZE * 80.0,
            TILE_SIZE * 160.0,
        ];

        let mut overall_best: Option<(Entity, f32)> = None;

        for &radius in &search_tiers {
            spatial_grid.get_nearby_in_radius_into(fam_pos, radius, scratch);
            let candidates: Vec<_> = scratch
                .iter()
                .filter_map(|&e| filter_candidate(e))
                .collect();

            if let Some((entity, score)) = find_best_scored(candidates) {
                let should_replace = match overall_best {
                    Some((_, best_score)) => score > best_score,
                    None => true,
                };
                if should_replace {
                    overall_best = Some((entity, score));
                }

                if score >= RECRUIT_GOOD_ENOUGH_SCORE {
                    return Some(entity);
                }
            }
        }

        overall_best.map(|(entity, _)| entity)
    }

    #[allow(clippy::too_many_arguments)]
    /// 即座にリクルートを試みる（近場の候補）
    ///
    /// メッセージ発行は行わない。呼び出し元が `Entity` を受け取って AddMember リクエストを発行する。
    pub fn try_immediate_recruit<G: SpatialGridOps>(
        fam_pos: Vec2,
        command_radius: f32,
        fatigue_threshold: f32,
        task_area_center: Option<Vec2>,
        spatial_grid: &G,
        q_souls: &SoulRecruitmentQuery,
        q_breakdown: &Query<&StressBreakdown>,
        q_resting: &Query<(), With<RestingIn>>,
        q_cooldown: &Query<&RestAreaCooldown>,
        scratch: &mut Vec<Entity>,
        excluded: &mut HashSet<Entity>,
    ) -> Option<Entity> {
        let recruit_entity = Self::find_best_recruit(
            fam_pos,
            fatigue_threshold,
            0.0,
            task_area_center,
            spatial_grid,
            q_souls,
            q_breakdown,
            q_resting,
            q_cooldown,
            scratch,
            Some(command_radius),
            excluded,
        )?;

        excluded.insert(recruit_entity);
        Some(recruit_entity)
    }

    #[allow(clippy::too_many_arguments)]
    /// スカウトを開始する（遠方の候補を検索）
    pub fn start_scouting<G: SpatialGridOps>(
        fam_pos: Vec2,
        fatigue_threshold: f32,
        task_area_center: Option<Vec2>,
        spatial_grid: &G,
        q_souls: &SoulRecruitmentQuery,
        q_breakdown: &Query<&StressBreakdown>,
        q_resting: &Query<(), With<RestingIn>>,
        q_cooldown: &Query<&RestAreaCooldown>,
        scratch: &mut Vec<Entity>,
        excluded: &mut HashSet<Entity>,
    ) -> Option<Entity> {
        let result = Self::find_best_recruit(
            fam_pos,
            fatigue_threshold,
            0.0,
            task_area_center,
            spatial_grid,
            q_souls,
            q_breakdown,
            q_resting,
            q_cooldown,
            scratch,
            None,
            excluded,
        )?;
        excluded.insert(result);
        Some(result)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// リクルート処理コンテキストと関数
// ──────────────────────────────────────────────────────────────────────────────

/// リクルート判定の結果
pub enum RecruitmentOutcome {
    /// 近傍ソウルを即時リクルート（AddMember メッセージを root 側で発行する）
    ImmediateRecruit(Entity),
    /// 遠方ソウルのスカウトを開始（state は context 内で更新済み）
    ScoutingStarted,
    /// リクルート候補なし
    NoRecruit,
}

/// リクルート判定に必要なコンテキスト
///
/// `request_writer` は含まない。
/// メッセージ発行は呼び出し元 (root adapter) が担当する。
pub struct FamiliarRecruitmentContext<'a, 'w, 's, G: SpatialGridOps> {
    pub fam_entity: Entity,
    pub fam_transform: &'a Transform,
    pub familiar: &'a Familiar,
    pub familiar_op: &'a FamiliarOperation,
    pub ai_state: &'a mut FamiliarAiState,
    pub fam_dest: &'a mut Destination,
    pub fam_path: &'a mut Path,
    pub squad_entities: &'a mut Vec<Entity>,
    pub max_workers: usize,
    pub task_area_opt: Option<&'a TaskArea>,
    pub spatial_grid: &'a G,
    pub q_souls: &'a SoulRecruitmentQuery<'w, 's>,
    pub q_breakdown: &'a Query<'w, 's, &'static StressBreakdown>,
    pub q_resting: &'a Query<'w, 's, (), With<RestingIn>>,
    pub q_cooldown: &'a Query<'w, 's, &'static RestAreaCooldown>,
    /// 同フレーム内でのリクルート予約セット（重複防止）
    pub recruitment_reservations: &'a mut HashSet<Entity>,
    /// 空間グリッド検索用の再利用可能バッファ
    pub scratch: &'a mut Vec<Entity>,
}

/// リクルート処理を実行
///
/// メッセージ発行は行わない。呼び出し元が `RecruitmentOutcome` に応じて処理する。
pub fn process_recruitment<G: SpatialGridOps>(
    ctx: &mut FamiliarRecruitmentContext<'_, '_, '_, G>,
) -> RecruitmentOutcome {
    let fam_pos = ctx.fam_transform.translation.truncate();
    let command_radius = ctx.familiar.command_radius;
    let fatigue_threshold = ctx.familiar_op.fatigue_threshold;
    let task_area_center = ctx.task_area_opt.map(TaskArea::center);

    if ctx.squad_entities.len() < ctx.max_workers {
        if let Some(new_recruit) = RecruitmentManager::try_immediate_recruit(
            fam_pos,
            command_radius,
            fatigue_threshold,
            task_area_center,
            ctx.spatial_grid,
            ctx.q_souls,
            ctx.q_breakdown,
            ctx.q_resting,
            ctx.q_cooldown,
            ctx.scratch,
            ctx.recruitment_reservations,
        ) {
            debug!(
                "FAM_AI: {:?} recruiting nearby soul {:?}",
                ctx.fam_entity, new_recruit
            );
            ctx.squad_entities.push(new_recruit);
            return RecruitmentOutcome::ImmediateRecruit(new_recruit);
        } else if let Some(distant_recruit) = RecruitmentManager::start_scouting(
            fam_pos,
            fatigue_threshold,
            task_area_center,
            ctx.spatial_grid,
            ctx.q_souls,
            ctx.q_breakdown,
            ctx.q_resting,
            ctx.q_cooldown,
            ctx.scratch,
            ctx.recruitment_reservations,
        ) {
            debug!(
                "FAM_AI: {:?} scouting distant soul {:?}",
                ctx.fam_entity, distant_recruit
            );
            *ctx.ai_state = FamiliarAiState::Scouting {
                target_soul: distant_recruit,
            };

            if let Ok((_, target_transform, _, _, _, _)) = ctx.q_souls.get(distant_recruit) {
                let target_pos = target_transform.translation.truncate();
                ctx.fam_dest.0 = target_pos;
                ctx.fam_path.waypoints = vec![target_pos];
                ctx.fam_path.current_index = 0;
            }
            return RecruitmentOutcome::ScoutingStarted;
        } else {
            debug!("FAM_AI: {:?} No recruitable souls found", ctx.fam_entity);
        }
    } else {
        debug!(
            "FAM_AI: {:?} Squad full ({}/{})",
            ctx.fam_entity,
            ctx.squad_entities.len(),
            ctx.max_workers
        );
    }
    RecruitmentOutcome::NoRecruit
}
