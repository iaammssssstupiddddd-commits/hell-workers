//! Temporary passive native diagnosis; remove after ownership diagnosis.
use bevy::prelude::*;
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const LIMIT: usize = 262144;
const BUFFER: usize = 4096;
const BYTES: usize = 64 * 1024 * 1024;

#[derive(Resource)]
struct Trace {
    path: PathBuf,
    rows: Vec<Value>,
    total: usize,
    bytes: usize,
    created: bool,
    limited: bool,
    limit_written: bool,
    failed: bool,
    last_flush: Instant,
}

impl Trace {
    fn new(path: PathBuf) -> Self {
        assert!(!path.exists(), "diagnostic output must be fresh");
        Self {
            path,
            rows: Vec::new(),
            total: 0,
            bytes: 0,
            created: false,
            limited: false,
            limit_written: false,
            failed: false,
            last_flush: Instant::now(),
        }
    }

    fn push(&mut self, row: Value) {
        if self.failed || self.limited {
            return;
        }
        if self.total >= LIMIT || self.rows.len() >= BUFFER {
            self.limited = true;
            return;
        }
        self.total += 1;
        self.rows.push(row);
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.failed || self.limit_written || (self.rows.is_empty() && !self.limited) {
            return Ok(());
        }
        let mut bytes = serde_json::to_vec(&json!({"schema":1,"total":self.total,
            "limited":self.limited,"rows":self.rows}))?;
        bytes.push(b'\n');
        let exhausted = self.bytes + bytes.len() > BYTES;
        if exhausted {
            self.limited = true;
            bytes =
                b"{\"schema\":1,\"limited\":true,\"reason\":\"byte cap\",\"rows\":[]}\n".to_vec();
            if self.bytes + bytes.len() > BYTES {
                self.failed = true;
                return Err(std::io::Error::other(
                    "diagnostic byte cap lacks limit record space",
                ));
            }
        }
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .append(self.created)
                .create_new(!self.created)
                .open(&self.path)?;
            file.write_all(&bytes)?;
            file.flush()
        })();
        if result.is_err() {
            self.failed = true;
            return result;
        }
        self.created = true;
        self.bytes += bytes.len();
        if !exhausted {
            self.rows.clear();
        }
        self.limit_written |= self.limited;
        self.last_flush = Instant::now();
        Ok(())
    }
}

fn bounded(text: &str) -> (String, bool) {
    (
        text.chars().take(64).collect(),
        text.chars().take(65).count() > 64,
    )
}

pub(super) fn register(app: &mut App, root: &Path) {
    app.insert_resource(Trace::new(root.join("despawn-diagnostic.jsonl")))
        .add_observer(trace_despawn)
        .add_systems(
            Last,
            flush_trace.after(crate::assets::building_asset_set::ProductionObservationSet),
        );
}

fn trace_despawn(
    on: On<Despawn, (Transform, Node, Name, Text2d, Text)>,
    entities: Query<EntityRef, Without<bevy::ecs::resource::IsResource>>,
    mut trace: ResMut<Trace>,
    time: Option<Res<Time<Real>>>,
    epoch: Option<Res<hw_core::WorldEpoch>>,
    frame: Option<Res<bevy::diagnostic::FrameCount>>,
) {
    if trace.failed || trace.limited {
        return;
    }
    let Ok(entity) = entities.get(on.entity) else {
        return;
    };
    let mut row = json!({"entity":format!("{:?}",on.entity),
        "parent":entity.get::<ChildOf>().map(|p|format!("{:?}",p.parent())),
        "time":time.map(|t|t.elapsed_secs_f64()),"epoch":epoch.map(|e|e.get()),"frame":frame.map(|f|f.0)});
    if entity.contains::<hw_visual::dream::DreamTrailGhost>() {
        row["kind"] = json!("DreamTrailGhost");
    } else {
        row["name"] = json!(entity.get::<Name>().map(|n| bounded(n.as_str())));
        row["text2d"] = json!(entity.get::<Text2d>().map(|t| bounded(&t.0)));
        row["text"] = json!(entity.get::<Text>().map(|t| bounded(&t.0)));
        row["speech"] = json!(entity.contains::<hw_visual::speech::components::SpeechBubble>());
        row["background"] =
            json!(entity.contains::<hw_visual::speech::components::SpeechBubbleBackground>());
        row["sprite"] = json!(entity.contains::<Sprite>());
        row["node"] = json!(entity.contains::<Node>());
    }
    trace.push(row);
}

fn flush_trace(mut trace: ResMut<Trace>, mut exit: MessageReader<AppExit>) {
    if (exit.read().next().is_some()
        || trace.rows.len() >= BUFFER / 2
        || trace.last_flush.elapsed() >= Duration::from_secs(1))
        && let Err(error) = trace.flush()
    {
        error!("native diagnostic write failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "hw-despawn-{}-{}.jsonl",
            std::process::id(),
            rand::random::<u64>()
        ))
    }

    #[test]
    fn byte_limit_records_unobserved_rows_once_without_consuming_them() {
        let path = path();
        let mut trace = Trace::new(path.clone());
        trace.bytes = BYTES - 100;
        trace.push(json!({"text":"x".repeat(512)}));
        trace.flush().unwrap();
        assert!(trace.limited && trace.limit_written);
        assert_eq!(trace.rows.len(), 1);
        let first = std::fs::read(&path).unwrap();
        trace.flush().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn buffered_trace_is_bounded_and_write_failure_preserves_rows() {
        let mut trace = Trace::new(path());
        for i in 0..BUFFER + 2 {
            trace.push(json!(i));
        }
        assert!(trace.limited);
        assert_eq!(trace.rows.len(), BUFFER);
        trace.path = trace.path.join("missing-parent");
        assert!(trace.flush().is_err());
        assert!(trace.failed);
        assert_eq!(trace.rows.len(), BUFFER);
        assert_eq!(bounded(&"猫".repeat(100)), ("猫".repeat(64), true));
    }

    #[test]
    fn actual_typed_trace_flushes_owned_tree_on_exit_once() {
        let mut app = App::new();
        app.add_message::<AppExit>();
        let path = path();
        app.insert_resource(Trace::new(path.clone()))
            .add_observer(trace_despawn)
            .add_systems(
                Last,
                flush_trace.after(crate::assets::building_asset_set::ProductionObservationSet),
            );
        let parent = app
            .world_mut()
            .spawn((Name::new("root"), Transform::default()))
            .id();
        let child = app
            .world_mut()
            .spawn((Name::new("child"), ChildOf(parent), Transform::default()))
            .id();
        app.world_mut().despawn(parent);
        assert_eq!(app.world().resource::<Trace>().rows.len(), 2);
        app.add_systems(
            Last,
            (|mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            })
            .in_set(crate::assets::building_asset_set::ProductionObservationSet),
        );
        app.update();
        let text = std::fs::read_to_string(&path).unwrap();
        let record: Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(record["rows"].as_array().unwrap().len(), 2);
        assert!(
            record["rows"].as_array().unwrap().iter().any(
                |r| r["entity"] == format!("{child:?}") && r["parent"] == format!("{parent:?}")
            )
        );
        assert!(app.world().resource::<Trace>().rows.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}
