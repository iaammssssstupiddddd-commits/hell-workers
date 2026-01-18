use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path, StressBreakdown,
};
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarVoice, UnderCommand,
};
use crate::events::FamiliarOperationMaxSoulChangedEvent;
use crate::relationships::Holding;
use crate::relationships::{Commanding, ManagedTasks, TaskWorkers};
use crate::systems::GameSystemSet;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, IssuedBy, TargetBlueprint, TaskSlots};
use crate::systems::jobs::{Designation, DesignationCreatedEvent};
use crate::systems::logistics::{ResourceItem, Stockpile};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::unassign_task;
use crate::systems::spatial::{
    DesignationSpatialGrid, SpatialGrid, update_designation_spatial_grid_system,
};
use crate::systems::visual::speech::components::{
    BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub mod following;
pub mod haul_cache;
pub mod helpers;
pub mod scouting;
pub mod searching;
pub mod supervising;

/// 使い魔のAI状態
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub enum FamiliarAiState {
    /// 待機中
    Idle,
    /// タスク探索中
    SearchingTask,
    /// スカウト中
    Scouting { target_soul: Entity },
    /// 監視中
    Supervising {
        /// 現在固定しているターゲット
        target: Option<Entity>,
        /// 切り替え禁止タイマー
        timer: f32,
    },
}

impl Default for FamiliarAiState {
    fn default() -> Self {
        Self::Idle
    }
}

pub struct FamiliarAiPlugin;

impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<FamiliarAiState>()
            .init_resource::<haul_cache::HaulReservationCache>()
            .init_resource::<DesignationSpatialGrid>()
            .add_systems(
                Update,
                (
                    update_designation_spatial_grid_system.in_set(GameSystemSet::Logic),
                    familiar_ai_system.in_set(GameSystemSet::Logic),
                    handle_max_soul_changed_system.in_set(GameSystemSet::Logic),
                    following::following_familiar_system.in_set(GameSystemSet::Logic),
                ),
            );
    }
}

#[derive(SystemParam)]
pub struct FamiliarAiParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub time: Res<'w, Time>,
    pub spatial_grid: Res<'w, SpatialGrid>,
    pub q_familiars: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Familiar,
            &'static FamiliarOperation,
            &'static ActiveCommand,
            &'static mut FamiliarAiState,
            &'static mut Destination,
            &'static mut Path,
            Option<&'static TaskArea>,
            Option<&'static Commanding>,
            Option<&'static ManagedTasks>,
            Option<&'static FamiliarVoice>,
        ),
    >,
    pub q_souls: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static DamnedSoul,
            &'static mut AssignedTask,
            &'static mut Destination,
            &'static mut Path,
            &'static IdleState,
            Option<&'static crate::relationships::Holding>,
            Option<&'static crate::entities::familiar::UnderCommand>,
        ),
        Without<Familiar>,
    >,
    pub q_designations: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static crate::systems::jobs::Designation,
            Option<&'static IssuedBy>,
            Option<&'static TaskSlots>,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub q_stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub _q_souls_lite: Query<'w, 's, (Entity, &'static UnderCommand), With<DamnedSoul>>,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    pub q_resources: Query<'w, 's, &'static ResourceItem>,
    pub q_target_blueprints: Query<'w, 's, &'static TargetBlueprint>,
    pub q_blueprints: Query<'w, 's, &'static Blueprint>,
    pub haul_cache: ResMut<'w, haul_cache::HaulReservationCache>,
    pub designation_grid: Res<'w, DesignationSpatialGrid>,
    pub game_assets: Res<'w, crate::assets::GameAssets>,
    pub q_bubbles: Query<'w, 's, (Entity, &'static SpeechBubble), With<FamiliarBubble>>,
    pub cooldowns: ResMut<'w, crate::systems::visual::speech::cooldown::BubbleCooldowns>,
    pub ev_created: MessageWriter<'w, crate::systems::jobs::DesignationCreatedEvent>,
}

/// 使い魔AIの更新システム
pub fn familiar_ai_system(params: FamiliarAiParams) {
    let FamiliarAiParams {
        mut commands,
        time,
        spatial_grid,
        mut q_familiars,
        mut q_souls,
        q_designations,
        q_stockpiles,
        _q_souls_lite,
        q_breakdown,
        q_resources,
        q_target_blueprints,
        q_blueprints,
        mut haul_cache,
        designation_grid,
        game_assets,
        q_bubbles,
        mut cooldowns,
        mut ev_created,
    } = params;
    // 1. 搬送中のアイテム・ストックパイル予約状況を事前計算
    // フェーズ2: 全ソウルをイテレートする代わりにキャッシュ（HaulReservationCache）を使用
    // let mut in_flight_haulers = std::collections::HashMap::new();
    // for (_, _, _, task, _, _, _, _, _) in q_souls.iter() {
    //     if let AssignedTask::Haul { stockpile, .. } = *task {
    //         *in_flight_haulers.entry(stockpile).or_insert(0) += 1;
    //     }
    // }

    for (
        fam_entity,
        fam_transform,
        familiar,
        familiar_op,
        active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        managed_tasks_opt,
        voice_opt,
    ) in q_familiars.iter_mut()
    {
        let default_tasks = crate::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

        // 個別の使い魔の処理開始ログ
        debug!(
            "FAM_AI: {:?} Processing. Command: {:?}, State: {:?}, Area: {}",
            fam_entity,
            active_command.command,
            *ai_state,
            task_area_opt.is_some()
        );

        let old_state = ai_state.clone();
        // 1. 基本コマンドチェック
        if matches!(active_command.command, FamiliarCommand::Idle) {
            if *ai_state != FamiliarAiState::Idle {
                debug!(
                    "FAM_AI: {:?} Switching to Idle state because command is Idle",
                    fam_entity
                );
                *ai_state = FamiliarAiState::Idle;
                // 休息フレーズを表示
                if cooldowns.can_speak(fam_entity, BubblePriority::Normal, time.elapsed_secs()) {
                    crate::systems::visual::speech::spawn::spawn_familiar_bubble(
                        &mut commands,
                        fam_entity,
                        crate::systems::visual::speech::phrases::LatinPhrase::Requiesce,
                        fam_transform.translation,
                        &game_assets,
                        &q_bubbles,
                        BubbleEmotion::Neutral,
                        BubblePriority::Normal,
                        voice_opt,
                    );
                    cooldowns.record_speech(
                        fam_entity,
                        BubblePriority::Normal,
                        time.elapsed_secs(),
                    );
                }
            }
            fam_dest.0 = fam_transform.translation.truncate();
            fam_path.waypoints.clear();
            continue;
        }

        let fam_pos = fam_transform.translation.truncate();
        let command_radius = familiar.command_radius;
        let fatigue_threshold = familiar_op.fatigue_threshold;

        // Relationshipから現在の部下リストを取得 (Commandingがない場合は空)
        let mut squad_entities: Vec<Entity> = if let Some(c) = commanding {
            c.iter().copied().collect()
        } else {
            Vec::new()
        };

        // 2. 疲労解放
        let mut released_entities: Vec<Entity> = Vec::new();
        for &member_entity in &squad_entities {
            if let Ok((entity, transform, soul, mut task, _, mut path, idle, holding_opt, uc)) =
                q_souls.get_mut(member_entity)
            {
                // 整合性チェック: 相手が自分を主人だと思っていないならリストから外す ( Relationship更新遅延対策 )
                if let Some(uc_comp) = uc {
                    if uc_comp.0 != fam_entity {
                        info!(
                            "FAM_AI: {:?} squad member {:?} belongs to another master {:?}",
                            fam_entity, member_entity, uc_comp.0
                        );
                        released_entities.push(member_entity);
                        continue;
                    }
                } else {
                    // ここで即座にリリースせず、1フレーム待つか警告に留める
                    // Relationship の反映がコンポーネントより先に来る可能性があるため
                    debug!(
                        "FAM_AI: {:?} squad member {:?} has no UnderCommand comp yet (waiting sync)",
                        fam_entity, member_entity
                    );
                    //released_entities.push(member_entity); // 一時的にコメントアウト
                    //continue;
                }

                // 疲労・崩壊チェック
                let is_resting = idle.behavior == IdleBehavior::Gathering;
                if (!is_resting && soul.fatigue > fatigue_threshold)
                    || idle.behavior == IdleBehavior::ExhaustedGathering
                {
                    debug!(
                        "FAM_AI: {:?} releasing soul {:?} (Fatigue/Exhausted)",
                        fam_entity, member_entity
                    );
                    unassign_task(
                        &mut commands,
                        entity,
                        transform.translation.truncate(),
                        &mut task,
                        &mut path,
                        holding_opt,
                        &q_designations,
                        &mut *haul_cache,
                        Some(&mut ev_created),
                        false, // emit_abandoned_event: 疲労リリース時は個別のタスク中断セリフを出さない
                    );

                    // リリースフレーズを表示
                    if cooldowns.can_speak(fam_entity, BubblePriority::Normal, time.elapsed_secs())
                    {
                        crate::systems::visual::speech::spawn::spawn_familiar_bubble(
                            &mut commands,
                            fam_entity,
                            crate::systems::visual::speech::phrases::LatinPhrase::Abi,
                            fam_transform.translation,
                            &game_assets,
                            &q_bubbles,
                            BubbleEmotion::Neutral,
                            BubblePriority::Normal,
                            voice_opt,
                        );
                        cooldowns.record_speech(
                            fam_entity,
                            BubblePriority::Normal,
                            time.elapsed_secs(),
                        );
                    }

                    commands.entity(member_entity).remove::<UnderCommand>();
                    released_entities.push(member_entity);
                }
            } else {
                // エンティティが消失している
                released_entities.push(member_entity);
            }
        }
        if !released_entities.is_empty() {
            let was_scouting = matches!(*ai_state, FamiliarAiState::Scouting { .. });
            squad_entities.retain(|e| !released_entities.contains(e));

            // 分隊が空になった瞬間を検知
            if squad_entities.is_empty() {
                debug!(
                    "FAM_AI: {:?} squad became empty (was_scouting: {}, state: {:?})",
                    fam_entity, was_scouting, *ai_state
                );
            }
        }

        // 3. 状態に応じたロジック実行
        let max_workers = familiar_op.max_controlled_soul;
        let mut state_changed = false;

        // --- ステートに応じた主要ロジック ---
        match *ai_state {
            FamiliarAiState::Scouting { target_soul } => {
                // Scoutingロジックを実行 (分隊の空き状況に関わらず常にチェック)
                state_changed = scouting::scouting_logic(
                    fam_entity,
                    fam_pos,
                    target_soul,
                    fatigue_threshold,
                    max_workers,
                    &mut squad_entities,
                    &mut ai_state,
                    &mut fam_dest,
                    &mut fam_path,
                    &q_souls,
                    &q_breakdown,
                    &mut commands,
                );
            }
            _ => {
                // スカウト中以外で分隊に空きがあれば新規リクルートを試みる
                if squad_entities.len() < max_workers {
                    // 近場のリクルート検索 (即時勧誘)
                    if let Some(new_recruit) = helpers::find_best_recruit(
                        fam_pos,
                        fatigue_threshold,
                        0.0,
                        &*spatial_grid,
                        &q_souls,
                        &q_breakdown,
                        Some(command_radius),
                    ) {
                        debug!(
                            "FAM_AI: {:?} recruiting nearby soul {:?}",
                            fam_entity, new_recruit
                        );
                        commands
                            .entity(new_recruit)
                            .insert(UnderCommand(fam_entity));
                        commands.trigger(crate::events::OnSoulRecruited {
                            entity: new_recruit,
                            familiar_entity: fam_entity,
                        });
                        squad_entities.push(new_recruit);
                        state_changed = true;
                    }
                    // 遠方のリクルート検索 (Scouting開始)
                    else {
                        if let Some(distant_recruit) = helpers::find_best_recruit(
                            fam_pos,
                            fatigue_threshold,
                            0.0,
                            &*spatial_grid,
                            &q_souls,
                            &q_breakdown,
                            None,
                        ) {
                            debug!(
                                "FAM_AI: {:?} scouting distant soul {:?}",
                                fam_entity, distant_recruit
                            );
                            *ai_state = FamiliarAiState::Scouting {
                                target_soul: distant_recruit,
                            };
                            state_changed = true;

                            // 即座に移動開始
                            if let Ok((_, target_transform, _, _, _, _, _, _, _)) =
                                q_souls.get(distant_recruit)
                            {
                                let target_pos = target_transform.translation.truncate();
                                fam_dest.0 = target_pos;
                                fam_path.waypoints = vec![target_pos];
                                fam_path.current_index = 0;
                            }
                        } else {
                            // 何も見つからなければログに出す (デバッグ用)
                            debug!("FAM_AI: {:?} No recruitable souls found", fam_entity);
                        }
                    }
                } else {
                    debug!(
                        "FAM_AI: {:?} Squad full ({}/{})",
                        fam_entity,
                        squad_entities.len(),
                        max_workers
                    );
                }
            }
        }

        // --- ステートの最終確定 ---
        if squad_entities.is_empty() {
            if !matches!(
                *ai_state,
                FamiliarAiState::SearchingTask
                    | FamiliarAiState::Idle
                    | FamiliarAiState::Scouting { .. }
            ) {
                let prev_state = ai_state.clone();
                *ai_state = FamiliarAiState::SearchingTask;
                state_changed = true;
                info!(
                    "FAM_AI: {:?} squad is empty. Transitioning to SearchingTask from {:?}",
                    fam_entity, prev_state
                );
            }
        } else {
            // メンバーがいるなら、スカウト中でない限り監視モードを維持
            if !matches!(*ai_state, FamiliarAiState::Scouting { .. }) {
                if !matches!(*ai_state, FamiliarAiState::Supervising { .. }) {
                    *ai_state = FamiliarAiState::Supervising {
                        target: None,
                        timer: 0.0,
                    };
                    state_changed = true;
                    // 状態遷移は重要なため info を残すが、形式を整理
                    info!("FAM_AI: {:?} squad not empty. -> Supervising", fam_entity);
                }
            }
        }

        if state_changed {
            info!(
                "FAM_AI: {:?} state changed: {:?} -> {:?}",
                fam_entity, old_state, *ai_state
            );
        }

        // 4. タスク委譲
        // 重複排除: 検索結果を保持して再利用
        let available_task_opt = helpers::find_unassigned_task_in_area(
            fam_entity,
            fam_pos,
            task_area_opt,
            &q_designations,
            &designation_grid,
            managed_tasks,
            &q_blueprints,
            &q_target_blueprints,
        );

        let has_available_task = available_task_opt.is_some();

        if let Some(task_entity) = available_task_opt {
            // タスクがある場合のみ、委譲処理を実行
            let mut idle_member_opt = None;
            for &member_entity in &squad_entities {
                if let Ok((_, _, soul, task, _, _, idle, _, _)) = q_souls.get(member_entity) {
                    if matches!(*task, AssignedTask::None)
                        && idle.behavior != IdleBehavior::ExhaustedGathering
                        && soul.fatigue < fatigue_threshold
                    {
                        idle_member_opt = Some(member_entity);
                        break; // 一人ずつ割り当てる
                    }
                }
            }

            if let Some(best_idle_member) = idle_member_opt {
                debug!(
                    "FAM_AI: Found task {:?} for idle member {:?}",
                    task_entity, best_idle_member
                );
                helpers::assign_task_to_worker(
                    &mut commands,
                    fam_entity,
                    task_entity,
                    best_idle_member,
                    fatigue_threshold,
                    &q_designations,
                    &mut q_souls,
                    &q_stockpiles,
                    &q_resources,
                    &q_target_blueprints,
                    &q_blueprints,
                    task_area_opt,
                    &mut *haul_cache,
                );
                commands.entity(task_entity).insert(IssuedBy(fam_entity));
            }
        }

        // 5. 移動制御
        // state_changed があっても、Supervising/SearchingTask なら各ロジックを呼ぶ
        if !state_changed
            || matches!(
                *ai_state,
                FamiliarAiState::Supervising { .. } | FamiliarAiState::SearchingTask
            )
        {
            let active_members: Vec<Entity> = squad_entities
                .iter()
                .filter(|&&e| {
                    if let Ok((_, _, _, _, _, _, idle, _, _)) = q_souls.get(e) {
                        idle.behavior != IdleBehavior::ExhaustedGathering
                    } else {
                        false
                    }
                })
                .copied()
                .collect();

            debug!(
                "FAM_AI: Movement control - state: {:?}, active_members: {}, has_available_task: {}, state_changed: {}",
                *ai_state,
                active_members.len(),
                has_available_task,
                state_changed
            );

            match *ai_state {
                FamiliarAiState::Supervising { .. } => {
                    // 移動制御では、既にチェック済みのhas_available_taskを使用
                    supervising::supervising_logic(
                        fam_entity,
                        fam_pos,
                        &active_members,
                        task_area_opt,
                        &time,
                        &mut ai_state,
                        &mut fam_dest,
                        &mut fam_path,
                        &q_souls,
                        has_available_task,
                    );
                }
                FamiliarAiState::SearchingTask => {
                    debug!("FAM_AI: {:?} executing SearchingTask logic", fam_entity);
                    searching::searching_logic(
                        fam_entity,
                        fam_pos,
                        task_area_opt,
                        &mut fam_dest,
                        &mut fam_path,
                    );
                }
                _ => {}
            }
        }
    }
}

/// 使役数上限変更イベントを処理するシステム
/// UIで使役数が減少した場合、超過分の魂をリリースする
pub fn handle_max_soul_changed_system(
    mut ev_max_soul_changed: MessageReader<FamiliarOperationMaxSoulChangedEvent>,
    q_familiars: Query<(&Transform, &FamiliarVoice, Option<&Familiar>), With<Familiar>>,
    q_commanding: Query<&Commanding, With<Familiar>>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut AssignedTask,
        &mut Path,
        Option<&Holding>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    mut haul_cache: ResMut<haul_cache::HaulReservationCache>,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
    game_assets: Res<crate::assets::GameAssets>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for event in ev_max_soul_changed.read() {
        // 使役数が減少した場合のみ処理
        if event.new_value < event.old_value {
            if let Ok(commanding) = q_commanding.get(event.familiar_entity) {
                let squad_entities: Vec<Entity> = commanding.iter().copied().collect();

                if squad_entities.len() > event.new_value {
                    let excess_count = squad_entities.len() - event.new_value;
                    info!(
                        "FAM_AI: {:?} max_soul decreased from {} to {}, releasing {} excess members",
                        event.familiar_entity, event.old_value, event.new_value, excess_count
                    );

                    // 超過分をリリース（後ろから順にリリース）
                    let mut released_count = 0;
                    for i in (0..squad_entities.len()).rev() {
                        if released_count >= excess_count {
                            break;
                        }
                        let member_entity = squad_entities[i];
                        if let Ok((entity, transform, mut task, mut path, holding_opt)) =
                            q_souls.get_mut(member_entity)
                        {
                            // タスクを解除
                            unassign_task(
                                &mut commands,
                                entity,
                                transform.translation.truncate(),
                                &mut task,
                                &mut path,
                                holding_opt,
                                &q_designations,
                                &mut *haul_cache,
                                Some(&mut ev_created),
                                false, // emit_abandoned_event: 上限超過リリース時は個別のタスク中断セリフを出さない
                            );
                        }

                        commands.entity(member_entity).remove::<UnderCommand>();
                        released_count += 1;

                        info!(
                            "FAM_AI: {:?} released excess member {:?} (limit: {} -> {})",
                            event.familiar_entity, member_entity, event.old_value, event.new_value
                        );
                    }

                    // リリースフレーズを表示（一度だけ）
                    if let Ok((fam_transform, voice_opt, _)) =
                        q_familiars.get(event.familiar_entity)
                    {
                        if cooldowns.can_speak(
                            event.familiar_entity,
                            BubblePriority::Normal,
                            time.elapsed_secs(),
                        ) {
                            crate::systems::visual::speech::spawn::spawn_familiar_bubble(
                                &mut commands,
                                event.familiar_entity,
                                crate::systems::visual::speech::phrases::LatinPhrase::Abi,
                                fam_transform.translation,
                                &game_assets,
                                &q_bubbles,
                                BubbleEmotion::Neutral,
                                BubblePriority::Normal,
                                Some(voice_opt),
                            );
                            cooldowns.record_speech(
                                event.familiar_entity,
                                BubblePriority::Normal,
                                time.elapsed_secs(),
                            );
                        }
                    }
                }
            }
        }
    }
}
