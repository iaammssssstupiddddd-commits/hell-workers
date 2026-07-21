//! Dream transfer の獲得表現（UI particle / popup）。
//!
//! Logic が実際に `DreamPool` へ移した量だけを Message から取り込み、camera や
//! UI が一時的に存在しなくても durable ledger に保持する。

use super::components::DreamGainPopup;
use super::ui_handles::DreamBubbleUiHandles;
use crate::floating_text::{
    FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text,
};
use crate::handles::MaterialIconHandles;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::camera::MainCamera;
use hw_core::constants::*;
use hw_core::events::{DreamTransferVisualSource, DreamTransferredVisualMessage};
use hw_core::soul::DreamQuality;
use hw_core::ui_nodes::{UiMountSlot, UiNodeRegistry, UiSlot};

#[derive(Debug, Clone)]
struct PendingDreamPresentation {
    soul: Entity,
    quality: DreamQuality,
    source: DreamTransferVisualSource,
    popup_pending: f32,
    ui_particle_pending: f32,
    elapsed: f32,
    idle_elapsed: f32,
    final_received: bool,
    received_this_frame: bool,
}

impl PendingDreamPresentation {
    fn matches(&self, message: &DreamTransferredVisualMessage) -> bool {
        self.soul == message.soul
            && self.quality == message.quality
            && same_source_identity(self.source, message.source)
    }

    fn is_complete(&self) -> bool {
        self.popup_pending <= f32::EPSILON && self.ui_particle_pending <= f32::EPSILON
    }

    fn should_flush_tail(&self) -> bool {
        self.final_received || self.idle_elapsed >= DREAM_POPUP_INTERVAL
    }

    fn advance_frame(&mut self, dt: f32) {
        self.elapsed += dt;
        if self.received_this_frame {
            self.idle_elapsed = 0.0;
        } else {
            self.idle_elapsed += dt;
        }
    }
}

fn same_source_identity(left: DreamTransferVisualSource, right: DreamTransferVisualSource) -> bool {
    match (left, right) {
        (
            DreamTransferVisualSource::Sleeping { .. },
            DreamTransferVisualSource::Sleeping { .. },
        ) => true,
        (
            DreamTransferVisualSource::RestArea {
                rest_area: left, ..
            },
            DreamTransferVisualSource::RestArea {
                rest_area: right, ..
            },
        ) => left == right,
        _ => false,
    }
}

/// Visual channel ごとの未配信量を保持する。
///
/// popup と UI particle は同じ transfer の複製表現なので、それぞれ独立して
/// debit し、二つを足して DreamPool の移送量とは比較しない。
#[derive(Resource, Debug, Default)]
pub struct DreamPresentationLedger {
    pending: Vec<PendingDreamPresentation>,
}

impl DreamPresentationLedger {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn pending_popup_mass(&self) -> f32 {
        self.pending.iter().map(|entry| entry.popup_pending).sum()
    }

    pub fn pending_ui_particle_mass(&self) -> f32 {
        self.pending
            .iter()
            .map(|entry| entry.ui_particle_pending)
            .sum()
    }
}

/// Message retention や presentation dependency に依存せず、毎 frame 全 transfer を
/// durable ledger へ移す。`GameSystemSet::Visual` の run condition 外で登録すること。
pub fn ingest_dream_transfers_system(
    mut messages: MessageReader<DreamTransferredVisualMessage>,
    mut ledger: ResMut<DreamPresentationLedger>,
) {
    for pending in &mut ledger.pending {
        pending.received_this_frame = false;
    }

    for message in messages.read().copied() {
        debug_assert!(message.amount.is_finite() && message.amount > 0.0);
        if !message.amount.is_finite() || message.amount <= 0.0 {
            continue;
        }

        if let Some(pending) = ledger
            .pending
            .iter_mut()
            .find(|pending| pending.matches(&message))
        {
            pending.source = message.source;
            pending.ui_particle_pending += message.amount;
            if matches!(message.source, DreamTransferVisualSource::Sleeping { .. }) {
                pending.popup_pending += message.amount;
            }
            pending.idle_elapsed = 0.0;
            pending.final_received |= message.is_final;
            pending.received_this_frame = true;
            continue;
        }

        ledger.pending.push(PendingDreamPresentation {
            soul: message.soul,
            quality: message.quality,
            source: message.source,
            popup_pending: if matches!(message.source, DreamTransferVisualSource::Sleeping { .. }) {
                message.amount
            } else {
                0.0
            },
            ui_particle_pending: message.amount,
            elapsed: 0.0,
            idle_elapsed: 0.0,
            final_received: message.is_final,
            received_this_frame: true,
        });
    }
}

#[derive(SystemParam)]
pub struct DreamPresentationParams<'w, 's> {
    commands: Commands<'w, 's>,
    time: Res<'w, Time>,
    handles: Option<Res<'w, MaterialIconHandles>>,
    ui_handles: Option<Res<'w, DreamBubbleUiHandles>>,
    ledger: ResMut<'w, DreamPresentationLedger>,
    q_source_transforms: Query<'w, 's, &'static GlobalTransform>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    q_ui_bubble_layer: Query<'w, 's, (Entity, &'static UiMountSlot)>,
    ui_nodes: Option<Res<'w, UiNodeRegistry>>,
    q_ui_transform: Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>,
    q_active_ui_particles: Query<'w, 's, (), With<super::components::DreamGainUiParticle>>,
}

/// Ledger から、現在利用できる presentation channel へだけ transfer を渡す。
/// 依存が欠けた channel の量は ledger に残り、次 frame 以降に再試行される。
pub fn dream_popup_spawn_system(mut p: DreamPresentationParams) {
    let dt = p.time.delta_secs();
    let popup_handles = p.handles.as_deref();
    let ui_handles = p.ui_handles.as_deref();
    let camera = p.q_camera.iter().next();
    let ui_layer = p
        .q_ui_bubble_layer
        .iter()
        .find(|(_, slot)| matches!(slot, UiMountSlot::DreamBubbleLayer))
        .map(|(entity, _)| entity);
    let mut particle_slots =
        DREAM_UI_PARTICLE_MAX_ACTIVE.saturating_sub(p.q_active_ui_particles.iter().count());

    for pending in &mut p.ledger.pending {
        pending.advance_frame(dt);
        let flush_tail = pending.should_flush_tail();
        if pending.elapsed < DREAM_POPUP_INTERVAL && !flush_tail {
            continue;
        }

        let origin = current_or_fallback_origin(pending, &p.q_source_transforms);
        let popup_ready = pending.popup_pending >= DREAM_POPUP_THRESHOLD || flush_tail;
        let mut delivered = false;

        if pending.popup_pending > f32::EPSILON
            && popup_ready
            && let Some(handles) = popup_handles
        {
            spawn_dream_popup(
                &mut p.commands,
                handles,
                origin,
                pending.popup_pending,
                pending.quality,
            );
            pending.popup_pending = 0.0;
            delivered = true;
        }

        if pending.ui_particle_pending > f32::EPSILON
            && particle_slots > 0
            && let (Some(handles), Some((camera, camera_transform)), Some(ui_layer)) =
                (ui_handles, camera, ui_layer)
            && let Ok(start_pos) =
                camera.world_to_viewport(camera_transform, popup_world_position(origin))
        {
            let target_pos =
                dream_pool_target_position(camera, p.ui_nodes.as_deref(), &p.q_ui_transform);
            super::ui_particle::spawn_ui_particle(
                &mut p.commands,
                start_pos,
                target_pos,
                ui_layer,
                handles,
                pending.ui_particle_pending,
            );
            pending.ui_particle_pending = 0.0;
            particle_slots -= 1;
            delivered = true;
        }

        if delivered {
            pending.elapsed = 0.0;
        }
    }

    p.ledger.pending.retain(|pending| !pending.is_complete());
}

fn current_or_fallback_origin(
    pending: &PendingDreamPresentation,
    transforms: &Query<&GlobalTransform>,
) -> Vec2 {
    let (source_entity, fallback) = match pending.source {
        DreamTransferVisualSource::Sleeping { origin } => (pending.soul, origin),
        DreamTransferVisualSource::RestArea { rest_area, origin } => (rest_area, origin),
    };
    transforms
        .get(source_entity)
        .map_or(fallback, |transform| transform.translation().truncate())
}

fn popup_world_position(origin: Vec2) -> Vec3 {
    (origin + Vec2::Y * DREAM_POPUP_OFFSET_Y).extend(Z_FLOATING_TEXT)
}

fn spawn_dream_popup(
    commands: &mut Commands,
    handles: &MaterialIconHandles,
    origin: Vec2,
    amount: f32,
    quality: DreamQuality,
) {
    let config = FloatingTextConfig {
        lifetime: DREAM_POPUP_LIFETIME,
        velocity: Vec2::new(0.0, DREAM_POPUP_VELOCITY_Y),
        initial_color: popup_color(quality),
        fade_out: true,
    };
    let popup_entity = spawn_floating_text(
        commands,
        format!("+{amount:.1} Dream"),
        popup_world_position(origin),
        config.clone(),
        Some(DREAM_POPUP_FONT_SIZE),
        handles.font_ui.clone(),
    );
    commands.entity(popup_entity).insert(DreamGainPopup {
        floating_text: FloatingText {
            lifetime: config.lifetime,
            config,
        },
    });
}

fn popup_color(quality: DreamQuality) -> Color {
    match quality {
        DreamQuality::VividDream => Color::srgb(0.55, 0.95, 1.0),
        DreamQuality::NormalDream | DreamQuality::Awake => Color::srgb(0.65, 0.9, 1.0),
        DreamQuality::NightTerror => Color::srgb(0.78, 0.55, 1.0),
    }
}

fn dream_pool_target_position(
    camera: &Camera,
    ui_nodes: Option<&UiNodeRegistry>,
    ui_transforms: &Query<(&ComputedNode, &UiGlobalTransform)>,
) -> Vec2 {
    let viewport_size = camera
        .logical_viewport_size()
        .unwrap_or(Vec2::new(1920.0, 1080.0));
    let fallback = Vec2::new(viewport_size.x - 80.0, 40.0);
    let Some(icon) = ui_nodes.and_then(|nodes| nodes.get_slot(UiSlot::DreamPoolIcon)) else {
        return fallback;
    };
    ui_transforms
        .get(icon)
        .map_or(fallback, |(node, transform)| {
            transform.translation * node.inverse_scale_factor()
        })
}

pub fn dream_popup_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(
        Entity,
        &mut DreamGainPopup,
        &mut FloatingText,
        &mut Transform,
        &mut TextColor,
    )>,
) {
    for (entity, mut popup, mut floating_text, mut transform, mut color) in q_popups.iter_mut() {
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).try_despawn();
            continue;
        }

        popup.floating_text = (*floating_text).clone();
        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advance_presentation_clock(mut ledger: ResMut<DreamPresentationLedger>) {
        for pending in &mut ledger.pending {
            pending.advance_frame(1.0 / 60.0);
        }
    }

    #[test]
    fn dream_transfer_ingestion_preserves_mass_without_ui_or_camera() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DreamTransferredVisualMessage>()
            .init_resource::<DreamPresentationLedger>()
            .add_systems(
                Update,
                (ingest_dream_transfers_system, advance_presentation_clock).chain(),
            );
        let soul = app.world_mut().spawn_empty().id();

        app.world_mut()
            .write_message(DreamTransferredVisualMessage {
                soul,
                amount: 0.25,
                quality: DreamQuality::NormalDream,
                source: DreamTransferVisualSource::Sleeping {
                    origin: Vec2::new(2.0, 3.0),
                },
                is_final: false,
            });
        app.update();

        let ledger = app.world().resource::<DreamPresentationLedger>();
        assert!((ledger.pending_popup_mass() - 0.25).abs() <= f32::EPSILON);
        assert!((ledger.pending_ui_particle_mass() - 0.25).abs() <= f32::EPSILON);
        assert!(!ledger.pending[0].should_flush_tail());

        app.update();
        let ledger = app.world().resource::<DreamPresentationLedger>();
        assert!((ledger.pending_popup_mass() - 0.25).abs() <= f32::EPSILON);
        assert!((ledger.pending_ui_particle_mass() - 0.25).abs() <= f32::EPSILON);
        assert!(!ledger.pending[0].should_flush_tail());
    }

    #[test]
    fn slow_step_gap_does_not_flush_pending_presentation() {
        let mut pending = PendingDreamPresentation {
            soul: Entity::PLACEHOLDER,
            quality: DreamQuality::NormalDream,
            source: DreamTransferVisualSource::Sleeping { origin: Vec2::ZERO },
            popup_pending: 0.1,
            ui_particle_pending: 0.1,
            elapsed: 0.1,
            idle_elapsed: 0.1,
            final_received: false,
            received_this_frame: false,
        };

        assert!(!pending.should_flush_tail());
        pending.idle_elapsed = DREAM_POPUP_INTERVAL;
        assert!(pending.should_flush_tail());
    }

    #[test]
    fn rest_area_transfer_uses_only_the_acquisition_particle_channel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DreamTransferredVisualMessage>()
            .init_resource::<DreamPresentationLedger>()
            .add_systems(Update, ingest_dream_transfers_system);
        let soul = app.world_mut().spawn_empty().id();
        let rest_area = app.world_mut().spawn_empty().id();

        app.world_mut()
            .write_message(DreamTransferredVisualMessage {
                soul,
                amount: 0.5,
                quality: DreamQuality::VividDream,
                source: DreamTransferVisualSource::RestArea {
                    rest_area,
                    origin: Vec2::ZERO,
                },
                is_final: true,
            });
        app.update();

        let ledger = app.world().resource::<DreamPresentationLedger>();
        assert_eq!(ledger.pending_popup_mass(), 0.0);
        assert!((ledger.pending_ui_particle_mass() - 0.5).abs() <= f32::EPSILON);
        assert!(ledger.pending[0].should_flush_tail());
    }
}
