use super::common::*;
use super::context::{TaskExecutionContext, TaskHandlerControl};
use super::types::{AssignedTask, RefineData, RefinePhase};
use bevy::prelude::*;
use hw_core::constants::*;
use hw_jobs::StoredByMixer;
use hw_logistics::{ResourceItem, ResourceType};

pub fn handle_refine_task(
    ctx: &mut TaskExecutionContext,
    data: RefineData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let RefineData { mixer, phase } = data;
    let mixer_entity = mixer;
    let soul_pos = ctx.soul_pos();

    match phase {
        RefinePhase::GoingToMixer => {
            if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();

                match update_task_destination_to_adjacent(ctx, mixer_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        debug!(
                            "REFINE: Soul {:?} cannot reach mixer {:?}, canceling",
                            ctx.soul_entity, mixer_entity
                        );
                        commands
                            .entity(mixer_entity)
                            .remove::<hw_jobs::Designation>();
                        commands.entity(mixer_entity).remove::<hw_jobs::TaskSlots>();
                        return ctx.abort_retryable(commands, "refine mixer unreachable");
                    }
                }

                if is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0) {
                    *ctx.task = AssignedTask::Refine(RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Refining { progress: 0.0 },
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                commands
                    .entity(mixer_entity)
                    .remove::<hw_jobs::Designation>();
                commands.entity(mixer_entity).remove::<hw_jobs::TaskSlots>();
                return ctx.abort_closed(commands, "refine mixer gone");
            }
        }

        RefinePhase::Refining { mut progress } => {
            if let Ok(mixer_data) = ctx.queries.storage.mixers.get_mut(mixer_entity) {
                let (mixer_transform, mut storage, _) = mixer_data;
                let water_count = match ctx.queries.storage.stockpiles.get(mixer_entity) {
                    Ok((_, _, stockpile, Some(stored_items)))
                        if stockpile.resource_type == Some(ResourceType::Water) =>
                    {
                        stored_items.len() as u32
                    }
                    _ => 0,
                };

                if !storage.has_materials_for_refining(water_count)
                    || !storage.has_output_capacity_for_refining()
                {
                    debug!(
                        "TASK_EXEC: Soul {:?} canceled refining due to lack of materials or mud storage",
                        ctx.soul_entity
                    );
                    commands
                        .entity(mixer_entity)
                        .remove::<hw_jobs::Designation>();
                    commands.entity(mixer_entity).remove::<hw_jobs::TaskSlots>();
                    return ctx.abort_retryable(commands, "refine materials unavailable");
                }

                progress += ctx.env.time.delta_secs() * GATHER_SPEED_BASE;

                if progress >= 1.0 {
                    storage.consume_materials_for_refining(water_count);
                    if let Some(water_entity) = ctx.queries.resource_items.iter().find_map(
                        |(res_entity, _, _, res_item, stored_in, _)| {
                            if res_item.0 == ResourceType::Water
                                && stored_in.map(|s| s.0) == Some(mixer_entity)
                            {
                                Some(res_entity)
                            } else {
                                None
                            }
                        },
                    ) {
                        ctx.identity.detach_from_working_on();
                        commands
                            .entity(ctx.soul_entity)
                            .remove::<hw_core::relationships::WorkingOn>();
                        commands.entity(water_entity).despawn();
                    } else {
                        warn!(
                            "TASK_EXEC: Soul {:?} could not find water item in mixer {:?} during refine",
                            ctx.soul_entity, mixer_entity
                        );
                    }

                    let pos = mixer_transform.translation;
                    for i in 0..STASIS_MUD_OUTPUT {
                        let offset =
                            Vec3::new(((i % 3) as f32 - 1.0) * 8.0, ((i / 3) as f32) * 8.0, 0.0);
                        commands.spawn((
                            ResourceItem(ResourceType::StasisMud),
                            StoredByMixer(mixer_entity),
                            Sprite {
                                image: ctx.env.soul_handles.icon_stasis_mud_small.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                ..default()
                            },
                            Visibility::Visible,
                            Transform::from_translation(
                                pos.truncate().extend(Z_ITEM_PICKUP) + offset,
                            ),
                            Name::new("Item (StasisMud)"),
                            hw_logistics::item_lifetime::ItemDespawnTimer::new(5.0),
                        ));
                    }
                    storage.mud += STASIS_MUD_OUTPUT;

                    debug!("TASK_EXEC: Soul {:?} refined 5 StasisMud", ctx.soul_entity);

                    *ctx.task = AssignedTask::Refine(RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Done,
                    });
                    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                } else {
                    *ctx.task = AssignedTask::Refine(RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Refining { progress },
                    });
                }
            } else {
                commands
                    .entity(mixer_entity)
                    .remove::<hw_jobs::Designation>();
                commands.entity(mixer_entity).remove::<hw_jobs::TaskSlots>();
                return ctx.abort_closed(commands, "refine mixer gone during refine");
            }
        }
        RefinePhase::Done => {
            commands
                .entity(mixer_entity)
                .remove::<hw_jobs::Designation>();
            commands.entity(mixer_entity).remove::<hw_jobs::TaskSlots>();
            commands.entity(mixer_entity).remove::<hw_jobs::IssuedBy>();
            ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                source: mixer_entity,
                amount: 1,
            });
            return ctx.complete_task(commands, "refine done");
        }
    }

    TaskHandlerControl::Continue
}
