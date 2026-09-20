//! One-shot production fixture and read-only flow evidence. Never repairs gameplay.
use super::super::{PerfCapture, PerfCapturePhase};
use super::*;
use hw_core::area::TaskArea;
use hw_core::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarPolicy, FamiliarWorkRule,
};
use hw_core::relationships::{CommandedBy, RestingIn, StoredIn};
use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{AssignedTask, StoredByMixer, WorkType};
use hw_logistics::{
    BelongsTo, ResourceItem, ResourceType, Stockpile, StockpileAcceptance, StockpilePolicy,
};
use hw_world::{WorldMap, Yard};

struct Lane {
    ordinal: usize,
    mixer: Entity,
    destinations: Vec<Entity>,
    produced: HashSet<Entity>,
    delivered: HashSet<Entity>,
    start: Option<Snapshot>,
    last: Snapshot,
    bins: [u32; 3],
    frames: u64,
    active_frames: u64,
}

impl Lane {
    fn new(ordinal: usize, mixer: Entity, destinations: Vec<Entity>) -> Self {
        Self {
            ordinal,
            mixer,
            destinations,
            produced: default(),
            delivered: default(),
            start: None,
            last: default(),
            bins: [0; 3],
            frames: 0,
            active_frames: 0,
        }
    }

    fn record_item(&mut self, entity: Entity, origin: Option<Entity>, destination: Option<Entity>) {
        if origin == Some(self.mixer) {
            self.produced.insert(entity);
        }
        if self.produced.contains(&entity)
            && destination.is_some_and(|d| self.destinations.contains(&d))
        {
            self.delivered.insert(entity);
        }
    }

    fn evidence(&self) -> Value {
        let start = self.start.unwrap_or_default();
        let produced = self.last.produced.saturating_sub(start.produced);
        let cycles = (produced / 5) as i64;
        json!({"ordinal": self.ordinal, "produced": produced,
            "delivered": self.last.delivered.saturating_sub(start.delivered),
            "sand_in": i64::from(self.last.sand) - i64::from(start.sand) + cycles,
            "rock_in": i64::from(self.last.rock) - i64::from(start.rock) + cycles,
            "water_in": self.last.water as i64 - start.water as i64 + cycles,
            "produced_per_20s": self.bins, "frames": self.frames, "active_frames": self.active_frames})
    }
}

#[derive(Clone, Copy, Default)]
struct Snapshot {
    produced: usize,
    delivered: usize,
    sand: u32,
    rock: u32,
    water: usize,
}

#[derive(Resource, Default)]
pub(crate) struct BuildingArtActiveState {
    initialized: bool,
    failed: bool,
    lanes: Vec<Lane>,
    frames: u64,
    seconds: f64,
    particles_max: usize,
    particles_sum: u64,
    ui_particles_max: usize,
    ui_particles_sum: u64,
}

pub(crate) fn setup(world: &mut World) {
    if world.resource::<PerfScenarioConfig>().workload() != PerfWorkload::BuildingArtActive
        || world.resource::<BuildingArtActiveState>().initialized
        || world.resource::<BuildingArtActiveState>().failed
        || world.resource::<BuildingArtStaticState>().phase != Phase::Ready
    {
        return;
    }
    if let Err(reason) = initialize(world) {
        error!("BUILDING_ART_ACTIVE: {reason}");
        world.resource_mut::<BuildingArtActiveState>().failed = true;
        world.write_message(AppExit::error());
    }
}

fn initialize(world: &mut World) -> Result<(), String> {
    let config = world.resource::<PerfScenarioConfig>();
    let copies = layout::copies(config.size());
    let mut familiars = world
        .query_filtered::<Entity, With<Familiar>>()
        .iter(world)
        .collect::<Vec<_>>();
    if familiars.len() != copies / 2 {
        return Err("Familiar inventory differs".into());
    }
    familiars.sort_by_key(|e| e.to_bits());
    let state = world.resource::<BuildingArtStaticState>();
    let mixers = state
        .specs
        .iter()
        .zip(&state.owners)
        .filter(|(s, _)| s.kind == BuildingType::MudMixer && s.quarter() >= 2)
        .map(|(s, &owner)| (s.clone(), owner))
        .collect::<Vec<_>>();
    let extras = state.actors[copies / 4 * 15..].to_vec();
    if extras.len() != mixers.len() * 7 {
        return Err("worker inventory differs".into());
    }
    let rock_image = world
        .resource::<crate::assets::GameAssets>()
        .icon_rock_small
        .clone();
    let mut lanes = Vec::new();
    for (index, (spec, mixer)) in mixers.into_iter().enumerate() {
        let familiar = familiars[index];
        let yards = world
            .query::<(Entity, &Yard)>()
            .iter(world)
            .filter(|(_, yard)| yard.contains(spec.center))
            .map(|(entity, yard)| (entity, yard.clone()))
            .collect::<Vec<_>>();
        if yards.len() != 1 {
            return Err("active lane must have one Yard".into());
        }
        let (yard_entity, yard) = &yards[0];
        let mut policy = FamiliarPolicy::default();
        policy.set_all_allowed(false);
        for work in [
            WorkType::Refine,
            WorkType::Haul,
            WorkType::HaulToMixer,
            WorkType::HaulWaterToMixer,
            WorkType::WheelbarrowHaul,
        ] {
            policy.set_rule(work, FamiliarWorkRule::default());
        }
        world.entity_mut(familiar).insert((
            ActiveCommand {
                command: FamiliarCommand::GatherResources,
            },
            FamiliarOperation {
                max_controlled_soul: 8,
                ..default()
            },
            policy,
            TaskArea::from_points(yard.min, yard.max),
        ));
        let position = WorldMap::grid_to_world(spec.anchor.0, 12);
        let mut transform = world
            .get_mut::<Transform>(familiar)
            .ok_or("missing Familiar transform")?;
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        let refiners = world
            .query::<(Entity, &AssignedTask)>()
            .iter(world)
            .filter_map(|(entity, task)| {
                matches!(task, AssignedTask::Refine(data) if data.mixer == mixer).then_some(entity)
            })
            .collect::<Vec<_>>();
        if refiners.len() != 1 {
            return Err("initial refiner differs".into());
        }
        world.entity_mut(refiners[0]).insert(CommandedBy(familiar));
        for &worker in &extras[index * 7..index * 7 + 7] {
            let mut entity = world.entity_mut(worker);
            let mut soul = entity.get_mut::<DamnedSoul>().ok_or("missing Soul")?;
            soul.dream = 100.0;
            soul.fatigue = 0.0;
            soul.stress = 0.0;
            let mut transform = entity
                .get_mut::<Transform>()
                .ok_or("missing Soul transform")?;
            transform.translation.x = position.x;
            transform.translation.y = position.y;
            entity.insert((
                CommandedBy(familiar),
                Destination(position),
                Path::default(),
                AssignedTask::None,
                IdleState::default(),
            ));
        }
        for _ in 0..30 {
            let position = WorldMap::grid_to_world(spec.anchor.0, 11);
            world.spawn((
                ResourceItem(ResourceType::Rock),
                Sprite {
                    image: rock_image.clone(),
                    custom_size: Some(Vec2::splat(16.0)),
                    ..default()
                },
                Transform::from_translation(position.extend(hw_core::constants::Z_ITEM_PICKUP)),
                Visibility::Visible,
            ));
        }
        let mut destinations = Vec::new();
        for y in 11..21 {
            let grid = (spec.anchor.0 + 2, y);
            let map = world.resource::<WorldMap>();
            if !map.is_walkable(grid.0, grid.1) || map.has_stockpile(grid) {
                return Err(format!("invalid output stockpile cell {grid:?}"));
            }
            let entity = world
                .spawn((
                    Stockpile {
                        capacity: 10,
                        resource_type: None,
                    },
                    StockpilePolicy {
                        acceptance: StockpileAcceptance::Only(ResourceType::StasisMud),
                        ..StockpilePolicy::for_capacity(10)
                    },
                    BelongsTo(*yard_entity),
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                        custom_size: Some(Vec2::splat(hw_core::constants::TILE_SIZE)),
                        ..default()
                    },
                    Transform::from_translation(
                        WorldMap::grid_to_world(grid.0, grid.1).extend(hw_core::constants::Z_MAP),
                    ),
                    Visibility::Visible,
                ))
                .id();
            world
                .resource_mut::<WorldMap>()
                .register_stockpile_tile(grid, entity);
            destinations.push(entity);
        }
        lanes.push(Lane::new(spec.ordinal, mixer, destinations));
    }
    *world.resource_mut::<BuildingArtActiveState>() = BuildingArtActiveState {
        initialized: true,
        lanes,
        ..default()
    };
    world.resource_mut::<PerfScenarioApplied>().workload = true;
    Ok(())
}

type Items<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ResourceItem,
        Option<&'static StoredByMixer>,
        Option<&'static StoredIn>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct ObserveParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    capture: Res<'w, PerfCapture>,
    state: ResMut<'w, BuildingArtActiveState>,
    time: Res<'w, Time<Virtual>>,
    mixers: Query<
        'w,
        's,
        (
            &'static MudMixerStorage,
            Option<&'static StoredItems>,
            &'static MudMixerVisualState,
        ),
    >,
    items: Items<'w, 's>,
    rest: Query<'w, 's, (), With<RestingIn>>,
    tasks: Query<'w, 's, &'static AssignedTask>,
    particles: Query<'w, 's, (), With<hw_visual::dream::DreamParticle>>,
    ui_particles: Query<'w, 's, (), With<hw_visual::dream::DreamGainUiParticle>>,
    camera: Query<'w, 's, &'static Transform, With<hw_ui::camera::MainCamera>>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn observe(mut p: ObserveParams) {
    if p.config.workload() != PerfWorkload::BuildingArtActive
        || !p.state.initialized
        || p.state.failed
        || !matches!(
            p.capture.phase(),
            PerfCapturePhase::Warmup | PerfCapturePhase::Measure
        )
    {
        return;
    }
    if let Err(reason) = observe_inner(&mut p) {
        error!("BUILDING_ART_ACTIVE: {reason}");
        p.state.failed = true;
        p.exit.write(AppExit::error());
    }
}

fn observe_inner(p: &mut ObserveParams) -> Result<(), String> {
    if p.time.is_paused() {
        return Err("active simulation paused".into());
    }
    let camera = p.camera.single().map_err(|_| "main camera differs")?;
    if camera.translation.truncate() != WorldMap::grid_to_world(46, 37)
        || camera.scale != Vec3::splat(layout::CAMERA_SCALE)
    {
        return Err("active camera changed".into());
    }
    let blocks = layout::copies(p.config.size()) / 4;
    let generating = p.tasks.iter().filter(|task| matches!(task,
        AssignedTask::GeneratePower(data) if matches!(data.phase, hw_jobs::GeneratePowerPhase::Generating))).count();
    if p.rest.iter().count() != blocks * 6 || generating != blocks * 7 {
        return Err("Rest/Spa real worker load changed".into());
    }
    let measuring = matches!(p.capture.phase(), PerfCapturePhase::Measure);
    let bin = ((p.capture.elapsed_secs / 20.0) as usize).min(2);
    for lane in &mut p.state.lanes {
        let (storage, water, visual) = p.mixers.get(lane.mixer).map_err(|_| "mixer disappeared")?;
        let before = lane.produced.len();
        for (entity, item, origin, stored) in &p.items {
            if item.0 != ResourceType::StasisMud {
                continue;
            }
            lane.record_item(
                entity,
                origin.map(|owner| owner.0),
                stored.map(|owner| owner.0),
            );
        }
        let snapshot = Snapshot {
            produced: lane.produced.len(),
            delivered: lane.delivered.len(),
            sand: storage.sand,
            rock: storage.rock,
            water: water.map_or(0, StoredItems::len),
        };
        if measuring {
            if lane.start.is_none() {
                // Previous warmup observation is the exact boundary snapshot.
                lane.start = Some(lane.last);
            }
            lane.bins[bin] += (snapshot.produced - before) as u32;
            lane.frames += 1;
            lane.active_frames += u64::from(visual.is_active);
        }
        lane.last = snapshot;
    }
    if measuring {
        let particles = p.particles.iter().count();
        p.state.particles_max = p.state.particles_max.max(particles);
        p.state.particles_sum += particles as u64;
        let ui_particles = p.ui_particles.iter().count();
        p.state.ui_particles_max = p.state.ui_particles_max.max(ui_particles);
        p.state.ui_particles_sum += ui_particles as u64;
        p.state.frames += 1;
        p.state.seconds += p.time.delta_secs_f64();
    }
    Ok(())
}

impl BuildingArtActiveState {
    pub(crate) fn write_sidecar(
        &self,
        config: &PerfScenarioConfig,
        fixture: &BuildingArtStaticState,
    ) -> std::io::Result<()> {
        if config.workload() != PerfWorkload::BuildingArtActive {
            return Ok(());
        }
        let records = self.lanes.iter().map(Lane::evidence).collect::<Vec<_>>();
        let digest: [u8; 32] = Sha256::digest(serde_json::to_vec(&fixture.evidence)?).into();
        let value = json!({"schema_version": 1, "contract_id": "building-art-active-nine-v1",
            "initialized": self.initialized, "failed": self.failed, "frames": self.frames, "seconds": self.seconds,
            "particles_max": self.particles_max, "particles_sum": self.particles_sum,
            "ui_particles_max": self.ui_particles_max, "ui_particles_sum": self.ui_particles_sum,
            "initial": fixture.evidence, "layout_sha256": hw_infra::lighting::digest_hex(&digest), "lanes": records});
        let directory = super::super::output::perf_output_directory(config);
        std::fs::create_dir_all(&directory)?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("building_art_active.json"))?;
        serde_json::to_writer_pretty(file, &value).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_identity_survives_pickup_but_only_real_arrival_counts() {
        let mut world = World::new();
        let mixer = world.spawn_empty().id();
        let destination = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        let mud = world.spawn_empty().id();
        let mut lane = Lane::new(2, mixer, vec![destination]);
        lane.record_item(mud, Some(mixer), None);
        lane.record_item(mud, Some(mixer), None);
        assert_eq!(lane.produced.len(), 1);
        lane.record_item(mud, None, None); // carrying/ground is not delivery
        lane.record_item(mud, None, Some(other));
        assert!(lane.delivered.is_empty());
        lane.record_item(other, None, Some(destination)); // not this mixer's product
        assert!(lane.delivered.is_empty());
        lane.record_item(mud, None, Some(destination));
        lane.record_item(mud, None, Some(destination));
        assert_eq!(lane.delivered.len(), 1);
    }

    #[test]
    fn flow_accounting_excludes_initial_stock_and_warmup_production() {
        let mut lane = Lane::new(2, Entity::PLACEHOLDER, vec![]);
        lane.start = Some(Snapshot {
            produced: 5,
            delivered: 5,
            sand: 1,
            rock: 1,
            water: 1,
        });
        lane.last = Snapshot {
            produced: 10,
            delivered: 5,
            sand: 0,
            rock: 0,
            water: 0,
        };
        let evidence = lane.evidence();
        assert_eq!(evidence["produced"], 5);
        for key in ["delivered", "sand_in", "rock_in", "water_in"] {
            assert_eq!(evidence[key], 0, "initial stock is not new input: {key}");
        }
        lane.last.sand = 2;
        lane.last.rock = 3;
        lane.last.water = 5;
        let evidence = lane.evidence();
        assert_eq!(evidence["sand_in"], 2);
        assert_eq!(evidence["rock_in"], 3);
        assert_eq!(evidence["water_in"], 5);
    }
}
