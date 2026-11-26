use super::services::Services;
use hw_infra::{
    air_transport::Fields,
    ventilation::{Opening, OpeningFlow, Ventilation, network::AirNetwork},
};

pub(super) enum Solver {
    Cached(Box<AirNetwork>),
    #[cfg(feature = "profiling")]
    Reference(Box<Reference>, Option<super::replay::Record>),
    #[cfg(feature = "profiling")]
    Replay(Box<super::replay::Replay>),
}

impl Solver {
    pub fn new(
        svc: &Services,
        ports: &[Opening],
        patterns: &mut hw_infra::ventilation::network::PatternCache,
        #[cfg(feature = "profiling")] mode: super::perf::Mode,
        #[cfg(feature = "profiling")] tape: Option<&std::sync::Arc<super::replay::Trajectory>>,
    ) -> Self {
        #[cfg(feature = "profiling")]
        if svc.volumes.len() == 256 && ports.len() == 512 {
            use super::perf::Mode;
            match mode {
                Mode::Replay => {
                    return Self::Replay(Box::new(super::replay::Replay::new(
                        svc,
                        ports,
                        tape.expect("replay trajectory").clone(),
                    )));
                }
                Mode::Record | Mode::Reference => {
                    return Self::Reference(
                        Box::default(),
                        (mode == Mode::Record).then(|| super::replay::Record::new(svc)),
                    );
                }
                Mode::Cached => {}
            }
        }
        Self::Cached(Box::new(AirNetwork::with_patterns(
            &svc.layout.floors,
            &svc.volumes,
            ports,
            &svc.walls,
            patterns,
        )))
    }

    #[cfg(feature = "profiling")]
    pub fn mode(&self) -> super::perf::Mode {
        use super::perf::Mode;
        match self {
            Self::Cached(_) => Mode::Cached,
            Self::Replay(_) => Mode::Replay,
            Self::Reference(_, Some(_)) => Mode::Record,
            Self::Reference(_, None) => Mode::Reference,
        }
    }

    pub fn update(
        &mut self,
        _tick: u64,
        wind: f64,
        drives: &[f64],
        removal: &[f64],
        _svc: &Services,
        _ports: &[Opening],
    ) -> bool {
        match self {
            Self::Cached(n) => n.update(wind, drives, removal),
            #[cfg(feature = "profiling")]
            Self::Replay(r) => r.update(_tick),
            #[cfg(feature = "profiling")]
            Self::Reference(r, _) => r.update(wind, drives, removal, _svc, _ports),
        }
    }
    pub fn readings(&self) -> (&[Ventilation], &[OpeningFlow]) {
        match self {
            Self::Cached(n) => (&n.rates, &n.flows),
            #[cfg(feature = "profiling")]
            Self::Replay(r) => r.readings(),
            #[cfg(feature = "profiling")]
            Self::Reference(r, _) => (&r.rates, &r.flows),
        }
    }
    pub fn step(&mut self, _tick: u64, outside: Fields, values: &mut [Fields], sources: &[Fields]) {
        match self {
            Self::Cached(n) => n.step(outside, values, sources),
            #[cfg(feature = "profiling")]
            Self::Replay(r) => r.step(_tick, values),
            #[cfg(feature = "profiling")]
            Self::Reference(r, record) => {
                r.step(outside, values, sources);
                if let Some(tape) = record {
                    tape.save(_tick, values, sources);
                }
            }
        }
    }
}

// Keep the pre-optimization equations as an independent native timing control.
#[cfg(feature = "profiling")]
pub(super) use reference::Reference;
#[cfg(feature = "profiling")]
mod reference {
    use super::*;
    use hw_infra::{
        air_transport::Transport,
        ventilation::{self, AirBoundary},
    };

    #[derive(Default)]
    pub(in crate::systems::ventilation) struct Reference {
        pressures: Vec<f64>,
        removal: Vec<f64>,
        pub(in crate::systems::ventilation) flows: Vec<OpeningFlow>,
        pub(in crate::systems::ventilation) rates: Vec<Ventilation>,
        heat: Option<Transport>,
        species: Option<Transport>,
        filter: Option<Transport>,
    }
    impl Reference {
        pub(in crate::systems::ventilation) fn update(
            &mut self,
            wind: f64,
            drives: &[f64],
            removal: &[f64],
            svc: &Services,
            ports: &[Opening],
        ) -> bool {
            let mut ports = ports.to_vec();
            let pressures: Vec<_> = ports
                .iter_mut()
                .zip(drives)
                .map(|(p, &drive)| {
                    if let AirBoundary::Outdoor { pressure } = &mut p.b {
                        *pressure = *pressure * wind + drive;
                        *pressure
                    } else {
                        0.
                    }
                })
                .collect();
            let changed = self.heat.is_none() || self.pressures != pressures;
            if changed {
                (self.rates, self.flows) = ventilation::solve(&svc.layout.floors, &ports);
                self.heat = Some(Transport::prepare(
                    &svc.volumes,
                    &ports,
                    &self.flows,
                    &svc.walls,
                    true,
                ));
                self.species = Some(Transport::prepare(
                    &svc.volumes,
                    &ports,
                    &self.flows,
                    &[],
                    false,
                ));
                self.pressures = pressures;
            }
            if changed || self.removal != removal {
                self.filter = removal.iter().any(|&r| r > 0.).then(|| {
                    Transport::with_removal(&svc.volumes, &ports, &self.flows, &[], false, removal)
                });
                self.removal = removal.to_vec();
            }
            changed
        }
        pub(super) fn step(&self, outside: Fields, values: &mut [Fields], sources: &[Fields]) {
            let species = self.species.as_ref().unwrap();
            self.heat
                .as_ref()
                .unwrap()
                .step(0, outside, values, sources);
            species.step(1, outside, values, sources);
            self.filter
                .as_ref()
                .unwrap_or(species)
                .step(2, outside, values, sources);
        }
    }
}
