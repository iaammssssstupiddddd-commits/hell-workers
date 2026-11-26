//! Optional counters for the native ventilation workload.
use bevy::prelude::*;
use hw_infra::ventilation::SolveCounts;

#[derive(Resource, Default)]
pub(crate) struct AirPerf {
    pub mode: Mode,
    pub full_publication: bool,
    pub input_ns: u64,
    pub rooms_written: usize,
    pub vents_written: usize,
    pub groups_changed: usize,
    pub publish_allocations: u64,
    pub publish_bytes: u64,
    pub wind: f64,
    pub inspected_net: f64,
    pub high_fans: usize,
    pub inspection_ns: u64,
    pub inspections: u64,
    pub pinned_vent: Option<Entity>,
    /// Fixed tick range, full/local pairs compared offline after native timing.
    pub publication_trace: Vec<u8>,
    pub actual_mode: Mode,
    pub checked_steps: u64,
    pub use_tape: bool,
    pub climate_build_ns: u64,
    pub climate_prepare_ns: u64,
    pub climate_step_ns: u64,
    pub climate_publish_ns: u64,
    pub synced_fans: usize,
    pub exterior_net_abs: f64,
    pub dimensions_requested: bool,
    pub transport: Option<super::transport_probe::TransportProbe>,
    pub calls: u64,
    pub rebuilds: u64,
    pub latest_cpu_ns: u64,
    pub build_cpu_ns: u64,
    pub climate_cpu_ns: u64,
    pub climate_tick: u64,
    pub climate_zones: usize,
    pub rooms: usize,
    pub grates: usize,
    pub counts: SolveCounts,
}

pub(crate) fn cpu_time_ns(process: bool) -> u64 {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let mut t = std::mem::MaybeUninit::uninit();
        // SAFETY: clock_gettime fills one timespec on success. Read only after checking its return.
        let clock = if process {
            libc::CLOCK_PROCESS_CPUTIME_ID
        } else {
            libc::CLOCK_THREAD_CPUTIME_ID
        };
        let result = unsafe { libc::clock_gettime(clock, t.as_mut_ptr()) };
        assert_eq!(result, 0, "ventilation benchmark requires thread CPU time");
        let t = unsafe { t.assume_init() };
        t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = process;
        panic!("ventilation CPU capture requires GNU/Linux")
    }
}

impl AirPerf {
    pub(super) fn sampled(&mut self, start: u64) {
        self.latest_cpu_ns = cpu_time_ns(false) - start;
        self.calls += 1;
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Mode {
    #[default]
    Cached,
    Reference,
    Record,
    Replay,
}
impl Mode {
    pub fn requested() -> Self {
        match std::env::var("HW_VENTILATION_SOLVER").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("cached") => Self::Cached,
            Ok("reference") => Self::Reference,
            Ok("record") => Self::Record,
            Ok("replay") => Self::Replay,
            _ => panic!("unknown ventilation benchmark solver"),
        }
    }
}
