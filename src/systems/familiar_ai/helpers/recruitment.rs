//! 使い魔のリクルート管理モジュール
//!
//! リクルートの検索・実行ロジックを提供します。

use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path, StressBreakdown,
};
use crate::relationships::CommandedBy;
// use crate::events::OnSoulRecruited;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::ParticipatingIn;
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};
use bevy::prelude::*;

/// リクルート管理ユーティリティ
pub struct RecruitmentManager;

impl RecruitmentManager {
    /// 条件に合う魂を検索する (リクルート用)
    pub fn find_best_recruit(
        fam_pos: Vec2,
        fatigue_threshold: f32,
        _min_fatigue: f32,
        spatial_grid: &SpatialGrid,
        q_souls: &mut FamiliarSoulQuery,
        q_breakdown: &Query<&StressBreakdown>,
        radius_opt: Option<f32>,
    ) -> Option<Entity> {
        // 候補をフィルタリングするヘルパークロージャ
        let filter_candidate = |e: Entity| -> Option<(Entity, Vec2)> {
            let (entity, transform, soul, task, _, _, idle, _, uc, _): (
                Entity,
                &Transform,
                &DamnedSoul,
                &AssignedTask,
                &Destination,
                &Path,
                &IdleState,
                Option<&crate::systems::logistics::Inventory>,
                Option<&CommandedBy>,
                Option<&ParticipatingIn>,
            ) = q_souls.get(e).ok()?;
            let recruit_threshold = fatigue_threshold - 0.2;
            let fatigue_ok = soul.fatigue < recruit_threshold;
            let stress_ok = q_breakdown.get(entity).is_err();

            if uc.is_none()
                && matches!(*task, AssignedTask::None)
                && fatigue_ok
                && stress_ok
                && idle.behavior != IdleBehavior::ExhaustedGathering
            {
                Some((entity, transform.translation.truncate()))
            } else {
                None
            }
        };

        // 候補リストから最も近いエンティティを選択するヘルパー
        let find_nearest = |candidates: Vec<(Entity, Vec2)>| -> Option<Entity> {
            candidates
                .into_iter()
                .min_by(|(_, p1), (_, p2)| {
                    p1.distance_squared(fam_pos)
                        .partial_cmp(&p2.distance_squared(fam_pos))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _)| e)
        };

        if let Some(radius) = radius_opt {
            let nearby = spatial_grid.get_nearby_in_radius(fam_pos, radius);
            let candidates: Vec<_> = nearby.iter().filter_map(|&e| filter_candidate(e)).collect();
            return find_nearest(candidates);
        }

        // radius_opt = None の場合: 段階的に検索半径を拡大
        let search_tiers = [
            TILE_SIZE * 20.0,  // 640px - 近傍
            TILE_SIZE * 40.0,  // 1280px - 中距離
            TILE_SIZE * 80.0,  // 2560px - 遠方
            TILE_SIZE * 160.0, // 5120px - 超遠方（マップ端対応）
        ];

        for &radius in &search_tiers {
            let radius: f32 = radius;
            let nearby = spatial_grid.get_nearby_in_radius(fam_pos, radius);
            let candidates: Vec<_> = nearby.iter().filter_map(|&e| filter_candidate(e)).collect();

            if let Some(best) = find_nearest(candidates) {
                return Some(best);
            }
        }

        None
    }

    /// 即座にリクルートを試みる（近場の候補）
    ///
    /// # 戻り値
    /// リクルートされたエンティティ、または None
    pub fn try_immediate_recruit(
        fam_entity: Entity,
        fam_pos: Vec2,
        command_radius: f32,
        fatigue_threshold: f32,
        spatial_grid: &SpatialGrid,
        q_souls: &mut FamiliarSoulQuery,
        q_breakdown: &Query<&StressBreakdown>,
        request_writer: &mut MessageWriter<crate::events::SquadManagementRequest>,
    ) -> Option<Entity> {
        let recruit_entity = Self::find_best_recruit(
            fam_pos,
            fatigue_threshold,
            0.0,
            spatial_grid,
            q_souls,
            q_breakdown,
            Some(command_radius),
        )?;

        // リクルート実行要求
        request_writer.write(crate::events::SquadManagementRequest {
            familiar_entity: fam_entity,
            operation: crate::events::SquadManagementOperation::AddMember {
                soul_entity: recruit_entity,
            },
        });

        Some(recruit_entity)
    }

    /// スカウトを開始する（遠方の候補を検索）
    ///
    /// # 戻り値
    /// スカウト対象のエンティティ、または None
    pub fn start_scouting(
        fam_pos: Vec2,
        fatigue_threshold: f32,
        spatial_grid: &SpatialGrid,
        q_souls: &mut FamiliarSoulQuery,
        q_breakdown: &Query<&StressBreakdown>,
    ) -> Option<Entity> {
        Self::find_best_recruit(
            fam_pos,
            fatigue_threshold,
            0.0,
            spatial_grid,
            q_souls,
            q_breakdown,
            None, // 半径制限なし（段階的検索）
        )
    }
}
