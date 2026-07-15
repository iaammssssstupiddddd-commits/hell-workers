use bevy::prelude::*;

use crate::entities::damned_soul::DamnedSoulSpawnEvent;
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::{
    DesignationRequest, EncouragementRequest, EscapeRequest, FamiliarAiStateChangedEvent,
    FamiliarIdleVisualRequest, FamiliarOperationMaxSoulChangedEvent, FamiliarStateRequest,
    GatheringManagementRequest, GatheringSpawnRequest, IdleBehaviorRequest,
    ResourceReservationRequest, SoulTaskUnassignRequest, SquadManagementRequest,
    TaskAssignmentRequest,
};
use hw_core::events::{
    OnGatheringJoined, OnGatheringParticipated, OnReleasedFromService, OnTaskAbandoned,
    OnTaskAssigned, SoulEncouragedVisualMessage, SoulExhaustedVisualMessage,
    SoulRecruitedVisualMessage, SoulStressBreakdownVisualMessage, TaskCompletedVisualMessage,
};
use hw_visual::speech::conversation::events::{
    ConversationCompleted, ConversationToneTriggered, RequestConversation,
};

macro_rules! root_message_types {
    ($callback:ident, $argument:expr) => {
        $callback!(
            $argument;
            DamnedSoulSpawnEvent,
            FamiliarSpawnEvent,
            FamiliarOperationMaxSoulChangedEvent,
            FamiliarAiStateChangedEvent,
            TaskAssignmentRequest,
            ResourceReservationRequest,
            SquadManagementRequest,
            IdleBehaviorRequest,
            EscapeRequest,
            GatheringManagementRequest,
            DesignationRequest,
            FamiliarStateRequest,
            EncouragementRequest,
            FamiliarIdleVisualRequest,
            RequestConversation,
            ConversationCompleted,
            ConversationToneTriggered,
            SoulRecruitedVisualMessage,
            SoulStressBreakdownVisualMessage,
            SoulExhaustedVisualMessage,
            TaskCompletedVisualMessage,
            SoulEncouragedVisualMessage,
            OnReleasedFromService,
            OnGatheringJoined,
            OnTaskAbandoned,
            OnGatheringParticipated,
            OnTaskAssigned,
            GatheringSpawnRequest,
            SoulTaskUnassignRequest,
        );
    };
}

macro_rules! add_root_messages {
    ($app:expr; $($message:ty),+ $(,)?) => {
        $(
            $app.add_message::<$message>();
        )+
    };
}

macro_rules! clear_root_messages_by_type {
    ($world:expr; $($message:ty),+ $(,)?) => {
        $(
            clear_message::<$message>($world);
        )+
    };
}

pub struct MessagesPlugin;

impl Plugin for MessagesPlugin {
    fn build(&self, app: &mut App) {
        root_message_types!(add_root_messages, app);
        crate::systems::save::register_load_reset_hook(app, "root-messages", clear_root_messages);
    }
}

/// Clears every root-owned message buffer before a persistent world is
/// replaced. The same type inventory initializes the buffers in
/// [`MessagesPlugin`], so new root message types cannot be registered without
/// also participating in this reset.
pub(crate) fn clear_root_messages(world: &mut World) {
    root_message_types!(clear_root_messages_by_type, world);
}

fn clear_message<T: Message>(world: &mut World) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<T>>() {
        messages.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;
    use bevy::ecs::system::SystemParam;
    use hw_core::events::{
        OnEncouraged, OnExhausted, OnSoulRecruited, OnStressBreakdown, OnTaskCompleted,
        publish_soul_encouraged, publish_soul_exhausted, publish_soul_recruited,
        publish_stress_breakdown, publish_task_completed,
    };
    use hw_core::jobs::WorkType;

    macro_rules! assert_root_messages_registered {
        ($app:expr; $($message:ty),+ $(,)?) => {
            $(
                assert!($app.world().contains_resource::<Messages<$message>>());
            )+
        };
    }

    #[test]
    fn root_message_reset_inventory_matches_registered_types() {
        let mut app = minimal_app();
        app.add_plugins(MessagesPlugin);

        root_message_types!(assert_root_messages_registered, app);
        clear_root_messages(app.world_mut());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum NotificationCase {
        SoulRecruited,
        StressBreakdown,
        Exhausted,
        ReleasedFromService,
        GatheringJoined,
        TaskAbandoned,
        TaskAssigned,
        TaskCompleted,
        GatheringParticipated,
        Encouraged,
    }

    impl NotificationCase {
        const ALL: [Self; 10] = [
            Self::SoulRecruited,
            Self::StressBreakdown,
            Self::Exhausted,
            Self::ReleasedFromService,
            Self::GatheringJoined,
            Self::TaskAbandoned,
            Self::TaskAssigned,
            Self::TaskCompleted,
            Self::GatheringParticipated,
            Self::Encouraged,
        ];
    }

    #[derive(Resource, Clone, Copy)]
    struct NotificationFixture {
        case: NotificationCase,
        soul: Entity,
        familiar: Entity,
        task: Entity,
        current_target: Entity,
        spot: Entity,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Receipt {
        DomainSoulRecruited {
            soul: Entity,
            familiar: Entity,
        },
        DomainStressBreakdown {
            soul: Entity,
        },
        DomainExhausted {
            soul: Entity,
        },
        DomainTaskCompleted {
            soul: Entity,
            assignment: Entity,
            current_target: Entity,
            work_type: WorkType,
        },
        DomainEncouraged {
            familiar: Entity,
            soul: Entity,
        },
        VisualSoulRecruited {
            soul: Entity,
            familiar: Entity,
        },
        VisualStressBreakdown {
            soul: Entity,
        },
        VisualExhaustedSpeech {
            soul: Entity,
        },
        VisualExhaustedExpression {
            soul: Entity,
        },
        VisualReleasedFromService {
            soul: Entity,
        },
        VisualGatheringJoined {
            soul: Entity,
        },
        VisualTaskAbandoned {
            soul: Entity,
        },
        VisualTaskAssigned {
            soul: Entity,
            assignment: Entity,
            current_target: Entity,
            work_type: WorkType,
        },
        VisualTaskCompleted {
            soul: Entity,
            assignment: Entity,
            current_target: Entity,
            work_type: WorkType,
        },
        VisualGatheringParticipated {
            soul: Entity,
            spot: Entity,
        },
        VisualEncouraged {
            familiar: Entity,
            soul: Entity,
        },
    }

    #[derive(Resource, Default)]
    struct Receipts(Vec<Receipt>);

    #[derive(SystemParam)]
    struct PresentationReaders<'w, 's> {
        recruited: MessageReader<'w, 's, SoulRecruitedVisualMessage>,
        stress_breakdown: MessageReader<'w, 's, SoulStressBreakdownVisualMessage>,
        exhausted_speech: MessageReader<'w, 's, SoulExhaustedVisualMessage>,
        exhausted_expression: MessageReader<'w, 's, SoulExhaustedVisualMessage>,
        task_completed: MessageReader<'w, 's, TaskCompletedVisualMessage>,
        encouraged: MessageReader<'w, 's, SoulEncouragedVisualMessage>,
        released: MessageReader<'w, 's, OnReleasedFromService>,
        gathering_joined: MessageReader<'w, 's, OnGatheringJoined>,
        task_abandoned: MessageReader<'w, 's, OnTaskAbandoned>,
        task_assigned: MessageReader<'w, 's, OnTaskAssigned>,
        gathering_participated: MessageReader<'w, 's, OnGatheringParticipated>,
    }

    fn publish_notification(
        mut commands: Commands,
        fixture: Res<NotificationFixture>,
        mut published: Local<bool>,
    ) {
        if *published {
            return;
        }
        *published = true;

        match fixture.case {
            NotificationCase::SoulRecruited => {
                publish_soul_recruited(&mut commands, fixture.soul, fixture.familiar);
            }
            NotificationCase::StressBreakdown => {
                publish_stress_breakdown(&mut commands, fixture.soul);
            }
            NotificationCase::Exhausted => {
                publish_soul_exhausted(&mut commands, fixture.soul);
            }
            NotificationCase::ReleasedFromService => {
                commands.write_message(OnReleasedFromService {
                    entity: fixture.soul,
                });
            }
            NotificationCase::GatheringJoined => {
                commands.write_message(OnGatheringJoined {
                    entity: fixture.soul,
                });
            }
            NotificationCase::TaskAbandoned => {
                commands.write_message(OnTaskAbandoned {
                    entity: fixture.soul,
                });
            }
            NotificationCase::TaskAssigned => {
                commands.write_message(OnTaskAssigned {
                    entity: fixture.soul,
                    assignment_entity: fixture.task,
                    current_target_entity: fixture.current_target,
                    current_work_type: WorkType::Chop,
                });
            }
            NotificationCase::TaskCompleted => {
                publish_task_completed(
                    &mut commands,
                    fixture.soul,
                    fixture.task,
                    fixture.current_target,
                    WorkType::Chop,
                );
            }
            NotificationCase::GatheringParticipated => {
                commands.write_message(OnGatheringParticipated {
                    entity: fixture.soul,
                    spot_entity: fixture.spot,
                });
            }
            NotificationCase::Encouraged => {
                publish_soul_encouraged(&mut commands, fixture.familiar, fixture.soul);
            }
        }
    }

    fn observe_soul_recruited(on: On<OnSoulRecruited>, mut receipts: ResMut<Receipts>) {
        let event = on.event();
        receipts.0.push(Receipt::DomainSoulRecruited {
            soul: event.entity,
            familiar: event.familiar_entity,
        });
    }

    fn observe_stress_breakdown(on: On<OnStressBreakdown>, mut receipts: ResMut<Receipts>) {
        receipts.0.push(Receipt::DomainStressBreakdown {
            soul: on.event().entity,
        });
    }

    fn observe_exhausted(on: On<OnExhausted>, mut receipts: ResMut<Receipts>) {
        receipts.0.push(Receipt::DomainExhausted {
            soul: on.event().entity,
        });
    }

    fn observe_task_completed(on: On<OnTaskCompleted>, mut receipts: ResMut<Receipts>) {
        let event = on.event();
        receipts.0.push(Receipt::DomainTaskCompleted {
            soul: event.entity,
            assignment: event.assignment_entity,
            current_target: event.current_target_entity,
            work_type: event.current_work_type,
        });
    }

    fn observe_encouraged(on: On<OnEncouraged>, mut receipts: ResMut<Receipts>) {
        let event = on.event();
        receipts.0.push(Receipt::DomainEncouraged {
            familiar: event.familiar_entity,
            soul: event.soul_entity,
        });
    }

    fn collect_presentation_receipts(
        mut readers: PresentationReaders,
        mut receipts: ResMut<Receipts>,
    ) {
        for event in readers.recruited.read() {
            receipts.0.push(Receipt::VisualSoulRecruited {
                soul: event.entity,
                familiar: event.familiar_entity,
            });
        }
        for event in readers.stress_breakdown.read() {
            receipts
                .0
                .push(Receipt::VisualStressBreakdown { soul: event.entity });
        }
        for event in readers.exhausted_speech.read() {
            receipts
                .0
                .push(Receipt::VisualExhaustedSpeech { soul: event.entity });
        }
        for event in readers.exhausted_expression.read() {
            receipts
                .0
                .push(Receipt::VisualExhaustedExpression { soul: event.entity });
        }
        for event in readers.task_completed.read() {
            receipts.0.push(Receipt::VisualTaskCompleted {
                soul: event.entity,
                assignment: event.assignment_entity,
                current_target: event.current_target_entity,
                work_type: event.current_work_type,
            });
        }
        for event in readers.encouraged.read() {
            receipts.0.push(Receipt::VisualEncouraged {
                familiar: event.familiar_entity,
                soul: event.soul_entity,
            });
        }
        for event in readers.released.read() {
            receipts
                .0
                .push(Receipt::VisualReleasedFromService { soul: event.entity });
        }
        for event in readers.gathering_joined.read() {
            receipts
                .0
                .push(Receipt::VisualGatheringJoined { soul: event.entity });
        }
        for event in readers.task_abandoned.read() {
            receipts
                .0
                .push(Receipt::VisualTaskAbandoned { soul: event.entity });
        }
        for event in readers.task_assigned.read() {
            receipts.0.push(Receipt::VisualTaskAssigned {
                soul: event.entity,
                assignment: event.assignment_entity,
                current_target: event.current_target_entity,
                work_type: event.current_work_type,
            });
        }
        for event in readers.gathering_participated.read() {
            receipts.0.push(Receipt::VisualGatheringParticipated {
                soul: event.entity,
                spot: event.spot_entity,
            });
        }
    }

    fn test_app(case: NotificationCase) -> (App, NotificationFixture) {
        let mut app = minimal_app();
        let fixture = NotificationFixture {
            case,
            soul: app.world_mut().spawn_empty().id(),
            familiar: app.world_mut().spawn_empty().id(),
            task: app.world_mut().spawn_empty().id(),
            current_target: app.world_mut().spawn_empty().id(),
            spot: app.world_mut().spawn_empty().id(),
        };

        app.insert_resource(fixture)
            .init_resource::<Receipts>()
            .add_plugins(MessagesPlugin)
            .add_observer(observe_soul_recruited)
            .add_observer(observe_stress_breakdown)
            .add_observer(observe_exhausted)
            .add_observer(observe_task_completed)
            .add_observer(observe_encouraged)
            .add_systems(
                Update,
                (publish_notification, collect_presentation_receipts).chain(),
            );

        (app, fixture)
    }

    fn expected_receipts(fixture: NotificationFixture) -> Vec<Receipt> {
        match fixture.case {
            NotificationCase::SoulRecruited => vec![
                Receipt::DomainSoulRecruited {
                    soul: fixture.soul,
                    familiar: fixture.familiar,
                },
                Receipt::VisualSoulRecruited {
                    soul: fixture.soul,
                    familiar: fixture.familiar,
                },
            ],
            NotificationCase::StressBreakdown => vec![
                Receipt::DomainStressBreakdown { soul: fixture.soul },
                Receipt::VisualStressBreakdown { soul: fixture.soul },
            ],
            NotificationCase::Exhausted => vec![
                Receipt::DomainExhausted { soul: fixture.soul },
                Receipt::VisualExhaustedSpeech { soul: fixture.soul },
                Receipt::VisualExhaustedExpression { soul: fixture.soul },
            ],
            NotificationCase::ReleasedFromService => {
                vec![Receipt::VisualReleasedFromService { soul: fixture.soul }]
            }
            NotificationCase::GatheringJoined => {
                vec![Receipt::VisualGatheringJoined { soul: fixture.soul }]
            }
            NotificationCase::TaskAbandoned => {
                vec![Receipt::VisualTaskAbandoned { soul: fixture.soul }]
            }
            NotificationCase::TaskAssigned => vec![Receipt::VisualTaskAssigned {
                soul: fixture.soul,
                assignment: fixture.task,
                current_target: fixture.current_target,
                work_type: WorkType::Chop,
            }],
            NotificationCase::TaskCompleted => vec![
                Receipt::DomainTaskCompleted {
                    soul: fixture.soul,
                    assignment: fixture.task,
                    current_target: fixture.current_target,
                    work_type: WorkType::Chop,
                },
                Receipt::VisualTaskCompleted {
                    soul: fixture.soul,
                    assignment: fixture.task,
                    current_target: fixture.current_target,
                    work_type: WorkType::Chop,
                },
            ],
            NotificationCase::GatheringParticipated => {
                vec![Receipt::VisualGatheringParticipated {
                    soul: fixture.soul,
                    spot: fixture.spot,
                }]
            }
            NotificationCase::Encouraged => vec![
                Receipt::DomainEncouraged {
                    familiar: fixture.familiar,
                    soul: fixture.soul,
                },
                Receipt::VisualEncouraged {
                    familiar: fixture.familiar,
                    soul: fixture.soul,
                },
            ],
        }
    }

    #[test]
    fn notification_matrix_delivers_declared_domain_and_presentation_transports() {
        for case in NotificationCase::ALL {
            let (mut app, fixture) = test_app(case);

            app.update();

            let receipts = &app.world().resource::<Receipts>().0;
            let expected = expected_receipts(fixture);
            assert_eq!(receipts.len(), expected.len(), "{case:?}: receipt count");
            for receipt in expected {
                assert!(receipts.contains(&receipt), "{case:?}: missing {receipt:?}");
            }
        }
    }
}
