//! Passive every-frame input journal, including the entire pre-arm prefix.
use super::*;
use serde::Deserialize;

const EVENT_CAP: usize = 256;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputArm {
    schema_version: u8,
    nonce: String,
    pid: u32,
    world_epoch: u64,
    observed_frame: u64,
}

#[derive(Resource)]
struct Journal {
    case: Case,
    mouse: MessageCursor<MouseButtonInput>,
    keyboard: MessageCursor<KeyboardInput>,
    events: Vec<Value>,
    arm_bytes: Option<Vec<u8>>,
    arm_frame: Option<u64>,
    last_frame: u64,
    last_real: f64,
    epoch: u64,
    failure: Option<String>,
}

fn validate_arm(arm: &InputArm, observer: &UiObserver, epoch: u64) -> Result<(), String> {
    if arm.schema_version != 1
        || arm.nonce != observer.nonce
        || arm.pid != std::process::id()
        || arm.world_epoch != epoch
        || arm.observed_frame < 46
        || arm.observed_frame > observer.frame
        || observer.frame - arm.observed_frame > 120
    {
        return Err("foreign, stale or premature Door input arm".into());
    }
    Ok(())
}

fn read_arm(path: &std::path::Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match path.symlink_metadata() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
        return Err("Door input arm must be a bounded regular file".into());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    if path
        .symlink_metadata()
        .map_err(|error| error.to_string())?
        .len()
        != metadata.len()
    {
        return Err("Door input arm changed during read".into());
    }
    Ok(Some(bytes))
}

fn record(
    world: &mut World,
    observer: &UiObserver,
    journal: &mut Journal,
    case: Case,
) -> Result<(), String> {
    let epoch = world.resource::<WorldEpoch>().get();
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if case != journal.case
        || epoch != journal.epoch
        || observer.frame <= journal.last_frame
        || !now.is_finite()
        || now < journal.last_real
    {
        return Err("Door input journal case/epoch/frame/clock changed".into());
    }
    journal.last_frame = observer.frame;
    journal.last_real = now;
    let mut windows =
        world.query_filtered::<Entity, (With<Window>, With<bevy::window::PrimaryWindow>)>();
    let window = windows
        .single(world)
        .map_err(|_| "Door input requires one primary window")?;
    let mut batch = Vec::new();
    if let Some(messages) = world.get_resource::<Messages<MouseButtonInput>>() {
        for event in journal.mouse.read(messages).take(EVENT_CAP + 1) {
            batch.push(
                json!({"producer":"MouseButtonInput", "frame": observer.frame,
                "real_seconds":now,"world_epoch":epoch,"window":event.window.to_bits(),
                "window_debug":format!("{:?}",event.window),"button":format!("{:?}",event.button),
                "state":format!("{:?}",event.state)}),
            );
        }
    }
    if let Some(messages) = world.get_resource::<Messages<KeyboardInput>>() {
        for event in journal.keyboard.read(messages).take(EVENT_CAP + 1) {
            batch.push(json!({"producer":"KeyboardInput","frame":observer.frame,
                "real_seconds":now,"world_epoch":epoch,"window":event.window.to_bits(),
                "window_debug":format!("{:?}",event.window),"key":format!("{:?}",event.key_code),
                "state":format!("{:?}",event.state),"repeat":event.repeat}));
        }
    }
    if journal.events.len() + batch.len() > EVENT_CAP {
        let remaining = EVENT_CAP - journal.events.len();
        journal.events.extend(batch.into_iter().take(remaining));
        return Err("Door input event cap exceeded; bounded prefix preserved".into());
    }
    let foreign_window = batch.iter().any(|row| row["window"] != window.to_bits());
    let prefix_activity = journal.arm_bytes.is_none() && !batch.is_empty();
    journal.events.extend(batch);
    if foreign_window || prefix_activity {
        return Err("foreign window or pre-arm Door input activity".into());
    }
    let receipts = world.get_resource::<crate::interface::selection::building_place::observed_outcomes::PlacementReceipts>();
    if receipts.is_some_and(|receipts| {
        receipts.failed || (journal.arm_bytes.is_none() && !receipts.rows.is_empty())
    }) {
        return Err("Door placement receipt overflow or pre-arm placement".into());
    }
    let bytes = read_arm(&observer.root.join("input-arm.json"))?;
    match (&journal.arm_bytes, bytes) {
        (None, Some(bytes)) => {
            if !world.contains_resource::<preparation::Target>() {
                return Err("Door arm appeared before preparation".into());
            }
            let arm: InputArm =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            validate_arm(&arm, observer, epoch)?;
            journal.arm_bytes = Some(bytes);
            journal.arm_frame = Some(observer.frame);
        }
        (Some(old), Some(bytes)) if old == &bytes => {}
        (Some(_), _) => return Err("Door input arm was replaced or removed".into()),
        (None, None) => {}
    }
    Ok(())
}

pub(super) fn record_frame(world: &mut World, observer: &UiObserver) -> Result<(), String> {
    if !world.contains_resource::<Journal>() {
        world.insert_resource(Journal {
            case: current_case().ok_or("missing Door case")?,
            mouse: default(),
            keyboard: default(),
            events: Vec::new(),
            arm_bytes: None,
            arm_frame: None,
            last_frame: 0,
            last_real: 0.,
            epoch: world.resource::<WorldEpoch>().get(),
            failure: None,
        });
    }
    world.resource_scope(|world, mut journal: Mut<Journal>| {
        let result = record(world, observer, &mut journal, current_case().ok_or("missing Door input case")?);
        if let Err(reason) = &result { journal.failure = Some(reason.clone()); }
        if result.is_err() || observer.frame.is_multiple_of(3) {
            let receipts = world.get_resource::<crate::interface::selection::building_place::observed_outcomes::PlacementReceipts>()
                .map(|receipts| receipts.rows.clone()).unwrap_or_default();
            let value = json!({"schema_version":1,"scope":SCOPE,"nonce":observer.nonce,
                "pid":std::process::id(),"frame":observer.frame,"real_seconds":journal.last_real,
                "world_epoch":journal.epoch,"arm_frame":journal.arm_frame,"events":journal.events,
                "receipts":receipts,"failure":journal.failure});
            let partial=observer.root.join("door-input.partial");
            fs::write(&partial,value.to_string()).and_then(|()|fs::rename(&partial,observer.root.join("door-input.json")))
                .map_err(|error| error.to_string())?;
        }
        result
    })
}

pub(super) fn snapshot(world: &World) -> Value {
    world
        .get_resource::<Journal>()
        .map_or(Value::Null, |journal| {
            json!({
        "arm_frame":journal.arm_frame,"events":journal.events.len(),"failure":journal.failure})
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::ButtonState;

    fn observer() -> UiObserver {
        UiObserver {
            root: std::env::temp_dir().join(format!(
                "door-input-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )),
            nonce: "a".repeat(32),
            scale: 1.,
            frame: 50,
            prepared: true,
            started: Instant::now(),
            save_cursor: default(),
            outcomes: Vec::new(),
            task_cursor: default(),
            task_outcomes: Vec::new(),
            wheels: default(),
        }
    }

    fn journal() -> Journal {
        Journal {
            case: Case::EastWest,
            mouse: default(),
            keyboard: default(),
            events: Vec::new(),
            arm_bytes: None,
            arm_frame: None,
            last_frame: 0,
            last_real: 0.,
            epoch: 0,
            failure: None,
        }
    }

    fn world() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<WorldEpoch>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<Messages<MouseButtonInput>>();
        let window = world
            .spawn((Window::default(), bevy::window::PrimaryWindow))
            .id();
        (world, window)
    }

    #[test]
    fn prearm_messages_are_retained_and_rejected_without_consuming_other_readers() {
        let (mut world, window) = world();
        let observer = observer();
        let mut journal = journal();
        world
            .resource_mut::<Messages<MouseButtonInput>>()
            .write(MouseButtonInput {
                window,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            });
        assert!(record(&mut world, &observer, &mut journal, Case::EastWest).is_err());
        assert_eq!(journal.events.len(), 1);
        assert_eq!(journal.events[0]["frame"], 50);
        let mut other = MessageCursor::<MouseButtonInput>::default();
        assert_eq!(
            other
                .read(world.resource::<Messages<MouseButtonInput>>())
                .count(),
            1
        );
        assert_eq!(world.resource::<Messages<MouseButtonInput>>().len(), 1);
    }

    #[test]
    fn overflow_preserves_a_bounded_invalid_prefix_instead_of_accepting_truncation() {
        let (mut world, window) = world();
        let observer = observer();
        let mut journal = journal();
        for _ in 0..257 {
            world
                .resource_mut::<Messages<MouseButtonInput>>()
                .write(MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                });
        }
        assert!(
            record(&mut world, &observer, &mut journal, Case::EastWest)
                .unwrap_err()
                .contains("cap exceeded")
        );
        assert_eq!(journal.events.len(), EVENT_CAP);
        assert_eq!(world.resource::<Messages<MouseButtonInput>>().len(), 257);
    }

    #[test]
    fn arm_requires_exact_typed_owner_epoch_and_recent_published_frame() {
        let observer = observer();
        let value = json!({"schema_version":1,"nonce":observer.nonce,"pid":std::process::id(),"world_epoch":0,"observed_frame":46});
        let arm: InputArm = serde_json::from_value(value.clone()).unwrap();
        assert!(validate_arm(&arm, &observer, 0).is_ok());
        for (field, bad) in [
            ("schema_version", json!(true)),
            ("pid", json!(true)),
            ("observed_frame", json!(true)),
            ("world_epoch", json!(true)),
            ("extra", json!(true)),
        ] {
            let mut row = value.clone();
            row[field] = bad;
            assert!(serde_json::from_value::<InputArm>(row).is_err());
        }
        for (field, bad) in [
            ("nonce", json!("b".repeat(32))),
            ("pid", json!(std::process::id() + 1)),
            ("world_epoch", json!(1)),
            ("observed_frame", json!(45)),
            ("observed_frame", json!(51)),
        ] {
            let mut row = value.clone();
            row[field] = bad;
            let arm: InputArm = serde_json::from_value(row).unwrap();
            assert!(validate_arm(&arm, &observer, 0).is_err());
        }
    }

    #[test]
    fn bound_arm_cannot_be_replaced_removed_or_used_before_preparation() {
        let (mut world, _) = world();
        let mut observer = observer();
        std::fs::create_dir(&observer.root).unwrap();
        let arm = json!({"schema_version":1,"nonce":observer.nonce,"pid":std::process::id(),"world_epoch":0,"observed_frame":46});
        let path = observer.root.join("input-arm.json");
        std::fs::write(&path, arm.to_string()).unwrap();
        assert!(record(&mut world, &observer, &mut journal(), Case::EastWest).is_err());
        world.insert_resource(preparation::Target {
            case: Case::EastWest,
            anchor: (50, 50),
            supports: Vec::new(),
            initial_buildings: Vec::new(),
            initial_cells: Vec::new(),
        });
        let mut journal = journal();
        assert!(record(&mut world, &observer, &mut journal, Case::EastWest).is_ok());
        observer.frame += 1;
        let mut changed = arm;
        changed["observed_frame"] = json!(47);
        std::fs::write(&path, changed.to_string()).unwrap();
        assert!(record(&mut world, &observer, &mut journal, Case::EastWest).is_err());
        std::fs::remove_file(&path).unwrap();
        observer.frame += 1;
        assert!(record(&mut world, &observer, &mut journal, Case::EastWest).is_err());
        std::fs::remove_dir(observer.root).unwrap();
    }
}
