//! Passive native control: replay transport outputs at the original virtual tick.
//! Geometry order is checked before playback. Only the dedicated recording run writes.
use super::{services::Services, solver::Reference};
use hw_infra::{
    air_transport::{Fields, weather},
    ventilation::{Opening, OpeningFlow, Ventilation},
};
use std::{
    fs::File,
    io::{BufWriter, Cursor, Read, Write},
    path::PathBuf,
};

fn path(s: &Services) -> PathBuf {
    let first = s.room_tiles[0][0];
    PathBuf::from(std::env::var_os("HW_AIR_TAPE").expect("benchmark tape directory"))
        .join(format!("air-{}-{}.bin", first.0, first.1))
}

fn geometry(s: &Services) -> Vec<u8> {
    serde_json::to_vec(&(
        &s.room_tiles,
        s.layout.ports.iter().map(|p| p.grid).collect::<Vec<_>>(),
    ))
    .unwrap()
}

pub(super) struct Record(BufWriter<File>);
impl Record {
    pub fn new(s: &Services) -> Self {
        assert!(s.devices.is_empty());
        let mut writer = BufWriter::new(File::create_new(path(s)).expect("new reference tape"));
        writer.write_all(&geometry(s)).unwrap();
        Self(writer)
    }
    pub fn save(&mut self, tick: u64, values: &[Fields], sources: &[Fields]) {
        self.0.write_all(&tick.to_le_bytes()).unwrap();
        for row in sources.iter().chain(values) {
            for &v in row {
                self.0.write_all(&v.to_le_bytes()).unwrap();
            }
        }
        self.0.flush().unwrap();
    }
}

pub(super) struct Trajectory {
    first: u64,
    fields: Vec<Fields>,
    n: usize,
}
impl Trajectory {
    pub fn load(s: &Services) -> Self {
        assert!(s.devices.is_empty());
        let data = std::fs::read(path(s)).expect("recorded passive trajectory");
        let header = geometry(s);
        assert!(
            data.starts_with(&header),
            "replay layout differs from recording"
        );
        let mut reader = Cursor::new(&data[header.len()..]);
        let mut first = None;
        let mut rows = Vec::new();
        let n = s.volumes.len();
        let mut bytes = [0; 8];
        while reader.position() < reader.get_ref().len() as u64 {
            reader.read_exact(&mut bytes).unwrap();
            let tick = u64::from_le_bytes(bytes);
            assert_eq!(
                tick,
                *first.get_or_insert(tick) + (rows.len() / (2 * n)) as u64
            );
            for _ in 0..2 * n {
                rows.push(std::array::from_fn(|_| {
                    reader.read_exact(&mut bytes).unwrap();
                    f64::from_le_bytes(bytes)
                }));
            }
        }
        Self {
            first: first.expect("nonempty recording"),
            fields: rows,
            n,
        }
    }
    pub fn frame(&self, tick: u64) -> (&[Fields], &[Fields]) {
        let start = tick
            .checked_sub(self.first)
            .expect("tick predates trajectory") as usize
            * 2
            * self.n;
        self.fields[start..start + 2 * self.n].split_at(self.n)
    }
}

pub(super) struct Replay {
    tape: std::sync::Arc<Trajectory>,
    rates: Vec<Vec<Ventilation>>,
    flows: Vec<Vec<OpeningFlow>>,
    phase: Option<usize>,
}
impl Replay {
    pub fn new(s: &Services, ports: &[Opening], tape: std::sync::Arc<Trajectory>) -> Self {
        let n = s.volumes.len();
        // In a passive fixture all port and room ventilation readings depend
        // only on weather. Materialize the reference readings before measurement.
        let mut reference = Reference::default();
        let mut rates = Vec::new();
        let mut flows = Vec::new();
        for phase in 0..16 {
            reference.update(
                weather(phase * 30).1,
                &vec![0.; ports.len()],
                &vec![0.; n],
                s,
                ports,
            );
            rates.push(reference.rates.clone());
            flows.push(reference.flows.clone());
        }
        Self {
            tape,
            rates,
            flows,
            phase: None,
        }
    }
    pub fn update(&mut self, tick: u64) -> bool {
        let phase = ((tick / 30) % 16) as usize;
        self.phase.replace(phase) != Some(phase)
    }
    pub fn readings(&self) -> (&[Ventilation], &[OpeningFlow]) {
        let phase = self.phase.unwrap();
        (&self.rates[phase], &self.flows[phase])
    }
    pub fn step(&self, tick: u64, values: &mut [Fields]) {
        values.copy_from_slice(self.tape.frame(tick).1);
    }
}
