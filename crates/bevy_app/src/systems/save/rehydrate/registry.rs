//! Phase-aware registry for candidate validation and post-load rebuilding.
//!
//! Domain adapters register raw hooks while plugins are built. `SavePlugin::finish`
//! resolves that graph once, before schedules run, and transactions retain only the
//! immutable plan. Normal apply and rollback therefore cannot drift into different
//! reconstruction sequences.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use bevy::prelude::*;

pub(super) type CandidateValidator = fn(&World) -> Result<(), String>;
pub(super) type LivePrerequisite = fn(&World) -> Result<(), String>;
/// Mutation callbacks are infallible after candidate/live validation. They must
/// not call `World::flush` or `World::clear_trackers`; phase barriers belong to
/// [`ResolvedRehydratePlan::run`] and tracker ownership stays in the transaction.
pub(super) type RehydrateCallback = fn(&mut World);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum RehydratePhase {
    DurableNormalize,
    RuntimeNormalize,
    AttachShells,
    RebuildDerived,
    WakeDomains,
}

#[derive(Debug, Clone, Copy)]
struct ValidationHook {
    name: &'static str,
    validate: CandidateValidator,
}

#[derive(Debug, Clone, Copy)]
struct PrerequisiteHook {
    name: &'static str,
    validate: LivePrerequisite,
}

#[derive(Debug, Clone, Copy)]
struct RehydrateStep {
    name: &'static str,
    phase: RehydratePhase,
    after: &'static [&'static str],
    requires: &'static [&'static str],
    run: RehydrateCallback,
}

#[derive(Resource, Default)]
struct RehydrateRegistry {
    validation_hooks: Vec<ValidationHook>,
    prerequisite_hooks: Vec<PrerequisiteHook>,
    steps: Vec<RehydrateStep>,
}

#[derive(Resource, Debug, Clone, Default)]
pub(in crate::systems::save) struct ResolvedRehydratePlan {
    validation_hooks: Vec<ValidationHook>,
    prerequisite_hooks: Vec<PrerequisiteHook>,
    steps: Vec<RehydrateStep>,
}

impl ResolvedRehydratePlan {
    pub(in crate::systems::save) fn validate_candidate(
        &self,
        candidate: &World,
    ) -> Result<(), String> {
        for hook in &self.validation_hooks {
            (hook.validate)(candidate)
                .map_err(|error| format!("candidate validator '{}': {error}", hook.name))?;
        }
        Ok(())
    }

    pub(in crate::systems::save) fn validate_live(&self, world: &World) -> Result<(), String> {
        for hook in &self.prerequisite_hooks {
            (hook.validate)(world)
                .map_err(|error| format!("live prerequisite '{}': {error}", hook.name))?;
        }
        Ok(())
    }

    pub(in crate::systems::save) fn run(&self, world: &mut World) {
        let mut attach_shells_finished = false;
        for step in &self.steps {
            if !attach_shells_finished && step.phase > RehydratePhase::AttachShells {
                world.flush();
                attach_shells_finished = true;
            }
            (step.run)(world);
        }
        world.flush();
    }

    #[cfg(test)]
    pub(super) fn step_names(&self) -> Vec<&'static str> {
        self.steps.iter().map(|step| step.name).collect()
    }

    #[cfg(test)]
    pub(super) fn validator_names(&self) -> Vec<&'static str> {
        self.validation_hooks.iter().map(|hook| hook.name).collect()
    }

    #[cfg(test)]
    pub(super) fn prerequisite_names(&self) -> Vec<&'static str> {
        self.prerequisite_hooks
            .iter()
            .map(|hook| hook.name)
            .collect()
    }

    #[cfg(test)]
    pub(in crate::systems::save) fn with_validator_for_test(
        name: &'static str,
        validate: CandidateValidator,
    ) -> Self {
        Self {
            validation_hooks: vec![ValidationHook { name, validate }],
            prerequisite_hooks: Vec::new(),
            steps: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(in crate::systems::save) fn with_step_for_test(
        name: &'static str,
        run: RehydrateCallback,
    ) -> Self {
        Self {
            validation_hooks: Vec::new(),
            prerequisite_hooks: Vec::new(),
            steps: vec![RehydrateStep {
                name,
                phase: RehydratePhase::RuntimeNormalize,
                after: &[],
                requires: &[],
                run,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegistryError {
    DuplicateName(&'static str),
    UnknownDependency {
        step: &'static str,
        dependency: &'static str,
    },
    UnknownPrerequisite {
        step: &'static str,
        prerequisite: &'static str,
    },
    UnusedPrerequisite(&'static str),
    PhaseRegression {
        step: &'static str,
        dependency: &'static str,
    },
    Cycle(Vec<&'static str>),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateName(name) => {
                write!(
                    formatter,
                    "rehydrate hook name '{name}' is registered more than once"
                )
            }
            Self::UnknownDependency { step, dependency } => write!(
                formatter,
                "rehydrate step '{step}' depends on unknown step '{dependency}'"
            ),
            Self::UnknownPrerequisite { step, prerequisite } => write!(
                formatter,
                "rehydrate step '{step}' requires unknown live prerequisite '{prerequisite}'"
            ),
            Self::UnusedPrerequisite(prerequisite) => write!(
                formatter,
                "live prerequisite '{prerequisite}' is not required by any rehydrate step"
            ),
            Self::PhaseRegression { step, dependency } => write!(
                formatter,
                "rehydrate step '{step}' depends on later-phase step '{dependency}'"
            ),
            Self::Cycle(steps) => write!(
                formatter,
                "rehydrate step dependency cycle contains: {}",
                steps.join(", ")
            ),
        }
    }
}

impl RehydrateRegistry {
    fn resolve(mut self) -> Result<ResolvedRehydratePlan, RegistryError> {
        let mut names = BTreeSet::new();
        for hook in &self.validation_hooks {
            if !names.insert(hook.name) {
                return Err(RegistryError::DuplicateName(hook.name));
            }
        }
        for hook in &self.prerequisite_hooks {
            if !names.insert(hook.name) {
                return Err(RegistryError::DuplicateName(hook.name));
            }
        }
        for step in &self.steps {
            if !names.insert(step.name) {
                return Err(RegistryError::DuplicateName(step.name));
            }
        }

        self.validation_hooks.sort_by_key(|hook| hook.name);
        self.prerequisite_hooks.sort_by_key(|hook| hook.name);
        let prerequisite_names: BTreeSet<_> = self
            .prerequisite_hooks
            .iter()
            .map(|hook| hook.name)
            .collect();
        let mut used_prerequisites = BTreeSet::new();
        for step in &self.steps {
            for &prerequisite in step.requires {
                if !prerequisite_names.contains(prerequisite) {
                    return Err(RegistryError::UnknownPrerequisite {
                        step: step.name,
                        prerequisite,
                    });
                }
                used_prerequisites.insert(prerequisite);
            }
        }
        if let Some(&unused) = prerequisite_names.difference(&used_prerequisites).next() {
            return Err(RegistryError::UnusedPrerequisite(unused));
        }
        let steps_by_name: HashMap<_, _> =
            self.steps.iter().map(|step| (step.name, *step)).collect();
        let mut indegree: HashMap<&'static str, usize> =
            self.steps.iter().map(|step| (step.name, 0)).collect();
        let mut dependents: HashMap<&'static str, Vec<&'static str>> = HashMap::new();

        for step in &self.steps {
            for &dependency in step.after {
                let Some(dependency_step) = steps_by_name.get(dependency) else {
                    return Err(RegistryError::UnknownDependency {
                        step: step.name,
                        dependency,
                    });
                };
                if dependency_step.phase > step.phase {
                    return Err(RegistryError::PhaseRegression {
                        step: step.name,
                        dependency,
                    });
                }
                *indegree.get_mut(step.name).expect("registered step") += 1;
                dependents.entry(dependency).or_default().push(step.name);
            }
        }

        let mut ready: BTreeSet<(RehydratePhase, &'static str)> = self
            .steps
            .iter()
            .filter(|step| indegree[step.name] == 0)
            .map(|step| (step.phase, step.name))
            .collect();
        let mut ordered = Vec::with_capacity(self.steps.len());
        while let Some((phase, name)) = ready.pop_first() {
            let step = steps_by_name[name];
            debug_assert_eq!(phase, step.phase);
            ordered.push(step);
            if let Some(children) = dependents.get(name) {
                for child in children {
                    let child_indegree = indegree.get_mut(child).expect("registered child");
                    *child_indegree -= 1;
                    if *child_indegree == 0 {
                        let child_step = steps_by_name[child];
                        ready.insert((child_step.phase, child_step.name));
                    }
                }
            }
        }

        if ordered.len() != self.steps.len() {
            let mut cycle: Vec<_> = indegree
                .into_iter()
                .filter_map(|(name, count)| (count > 0).then_some(name))
                .collect();
            cycle.sort_unstable();
            return Err(RegistryError::Cycle(cycle));
        }

        // A dependency-free later phase must never jump ahead of an earlier
        // phase whose same-phase dependency is unresolved. The ready key above
        // normally provides this property; assert it at the freeze boundary.
        if ordered.windows(2).any(|pair| pair[0].phase > pair[1].phase) {
            let phases: BTreeMap<_, _> =
                ordered.iter().map(|step| (step.name, step.phase)).collect();
            panic!("resolved rehydrate phases regressed: {phases:?}");
        }

        Ok(ResolvedRehydratePlan {
            validation_hooks: self.validation_hooks,
            prerequisite_hooks: self.prerequisite_hooks,
            steps: ordered,
        })
    }
}

pub(super) fn register_candidate_validator(
    app: &mut App,
    name: &'static str,
    validate: CandidateValidator,
) {
    assert!(
        !app.world().contains_resource::<ResolvedRehydratePlan>(),
        "candidate validator '{name}' was registered after the rehydrate plan was frozen"
    );
    app.init_resource::<RehydrateRegistry>();
    app.world_mut()
        .resource_mut::<RehydrateRegistry>()
        .validation_hooks
        .push(ValidationHook { name, validate });
}

pub(super) fn register_live_prerequisite(
    app: &mut App,
    name: &'static str,
    validate: LivePrerequisite,
) {
    assert!(
        !app.world().contains_resource::<ResolvedRehydratePlan>(),
        "live prerequisite '{name}' was registered after the rehydrate plan was frozen"
    );
    app.init_resource::<RehydrateRegistry>();
    app.world_mut()
        .resource_mut::<RehydrateRegistry>()
        .prerequisite_hooks
        .push(PrerequisiteHook { name, validate });
}

pub(super) fn register_rehydrate_step(
    app: &mut App,
    name: &'static str,
    phase: RehydratePhase,
    after: &'static [&'static str],
    requires: &'static [&'static str],
    run: RehydrateCallback,
) {
    assert!(
        !app.world().contains_resource::<ResolvedRehydratePlan>(),
        "rehydrate step '{name}' was registered after the plan was frozen"
    );
    app.init_resource::<RehydrateRegistry>();
    app.world_mut()
        .resource_mut::<RehydrateRegistry>()
        .steps
        .push(RehydrateStep {
            name,
            phase,
            after,
            requires,
            run,
        });
}

pub(super) fn freeze_rehydrate_registry(app: &mut App) {
    let raw = app
        .world_mut()
        .remove_resource::<RehydrateRegistry>()
        .unwrap_or_default();
    let plan = raw
        .resolve()
        .unwrap_or_else(|error| panic!("invalid rehydrate registry: {error}"));
    app.insert_resource(plan);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Trace(Vec<&'static str>);

    #[derive(Component)]
    struct DeferredShell;

    #[derive(Resource, Default)]
    struct ObservedShellCount(usize);

    fn noop(_: &mut World) {}

    fn live_noop(_: &World) -> Result<(), String> {
        Ok(())
    }

    fn trace_a(world: &mut World) {
        world.resource_mut::<Trace>().0.push("a");
    }

    fn trace_b(world: &mut World) {
        world.resource_mut::<Trace>().0.push("b");
    }

    fn trace_c(world: &mut World) {
        world.resource_mut::<Trace>().0.push("c");
    }

    fn attach_deferred_shell(world: &mut World) {
        world.commands().spawn(DeferredShell);
    }

    fn observe_deferred_shell(world: &mut World) {
        let count = world
            .query_filtered::<Entity, With<DeferredShell>>()
            .iter(world)
            .count();
        world.resource_mut::<ObservedShellCount>().0 = count;
    }

    fn registry(steps: Vec<RehydrateStep>) -> RehydrateRegistry {
        RehydrateRegistry {
            validation_hooks: Vec::new(),
            prerequisite_hooks: Vec::new(),
            steps,
        }
    }

    fn step(
        name: &'static str,
        phase: RehydratePhase,
        after: &'static [&'static str],
    ) -> RehydrateStep {
        RehydrateStep {
            name,
            phase,
            after,
            requires: &[],
            run: noop,
        }
    }

    #[test]
    fn graph_rejects_duplicate_unknown_cycle_and_phase_regression() {
        let duplicate = registry(vec![
            step("same", RehydratePhase::RuntimeNormalize, &[]),
            step("same", RehydratePhase::RuntimeNormalize, &[]),
        ]);
        assert_eq!(
            duplicate.resolve().unwrap_err(),
            RegistryError::DuplicateName("same")
        );

        let unknown = registry(vec![step(
            "child",
            RehydratePhase::RuntimeNormalize,
            &["missing"],
        )]);
        assert_eq!(
            unknown.resolve().unwrap_err(),
            RegistryError::UnknownDependency {
                step: "child",
                dependency: "missing"
            }
        );

        let cycle = registry(vec![
            step("a", RehydratePhase::RuntimeNormalize, &["b"]),
            step("b", RehydratePhase::RuntimeNormalize, &["a"]),
        ]);
        assert_eq!(
            cycle.resolve().unwrap_err(),
            RegistryError::Cycle(vec!["a", "b"])
        );

        let regression = registry(vec![
            step("late", RehydratePhase::WakeDomains, &[]),
            step("early", RehydratePhase::RuntimeNormalize, &["late"]),
        ]);
        assert_eq!(
            regression.resolve().unwrap_err(),
            RegistryError::PhaseRegression {
                step: "early",
                dependency: "late"
            }
        );

        let unknown_prerequisite = RehydrateRegistry {
            validation_hooks: Vec::new(),
            prerequisite_hooks: Vec::new(),
            steps: vec![RehydrateStep {
                name: "step",
                phase: RehydratePhase::RuntimeNormalize,
                after: &[],
                requires: &["missing"],
                run: noop,
            }],
        };
        assert_eq!(
            unknown_prerequisite.resolve().unwrap_err(),
            RegistryError::UnknownPrerequisite {
                step: "step",
                prerequisite: "missing",
            }
        );

        let unused_prerequisite = RehydrateRegistry {
            validation_hooks: Vec::new(),
            prerequisite_hooks: vec![PrerequisiteHook {
                name: "unused",
                validate: live_noop,
            }],
            steps: Vec::new(),
        };
        assert_eq!(
            unused_prerequisite.resolve().unwrap_err(),
            RegistryError::UnusedPrerequisite("unused")
        );
    }

    #[test]
    fn resolution_is_phase_then_name_stable_across_registration_order() {
        let first = registry(vec![
            step("runtime-b", RehydratePhase::RuntimeNormalize, &[]),
            step("durable", RehydratePhase::DurableNormalize, &[]),
            step("runtime-a", RehydratePhase::RuntimeNormalize, &[]),
        ])
        .resolve()
        .unwrap();
        let second = registry(vec![
            step("runtime-a", RehydratePhase::RuntimeNormalize, &[]),
            step("runtime-b", RehydratePhase::RuntimeNormalize, &[]),
            step("durable", RehydratePhase::DurableNormalize, &[]),
        ])
        .resolve()
        .unwrap();

        assert_eq!(first.step_names(), second.step_names());
        assert_eq!(
            first.step_names(),
            vec!["durable", "runtime-a", "runtime-b"]
        );
    }

    #[test]
    fn runner_executes_each_resolved_step_once() {
        let plan = RehydrateRegistry {
            validation_hooks: Vec::new(),
            prerequisite_hooks: Vec::new(),
            steps: vec![
                RehydrateStep {
                    name: "c",
                    phase: RehydratePhase::WakeDomains,
                    after: &["b"],
                    requires: &[],
                    run: trace_c,
                },
                RehydrateStep {
                    name: "a",
                    phase: RehydratePhase::DurableNormalize,
                    after: &[],
                    requires: &[],
                    run: trace_a,
                },
                RehydrateStep {
                    name: "b",
                    phase: RehydratePhase::AttachShells,
                    after: &["a"],
                    requires: &[],
                    run: trace_b,
                },
            ],
        }
        .resolve()
        .unwrap();
        let mut world = World::new();
        world.init_resource::<Trace>();

        plan.run(&mut world);

        assert_eq!(world.resource::<Trace>().0, vec!["a", "b", "c"]);
    }

    #[test]
    fn runner_owns_the_shell_to_derived_flush_barrier() {
        let plan = registry(vec![
            RehydrateStep {
                name: "shell",
                phase: RehydratePhase::AttachShells,
                after: &[],
                requires: &[],
                run: attach_deferred_shell,
            },
            RehydrateStep {
                name: "derived",
                phase: RehydratePhase::RebuildDerived,
                after: &["shell"],
                requires: &[],
                run: observe_deferred_shell,
            },
        ])
        .resolve()
        .unwrap();
        let mut world = World::new();
        world.init_resource::<ObservedShellCount>();

        plan.run(&mut world);

        assert_eq!(world.resource::<ObservedShellCount>().0, 1);
    }

    struct LateRegistrationPlugin;

    struct ProductionAdapterPlugin;

    impl Plugin for ProductionAdapterPlugin {
        fn build(&self, app: &mut App) {
            super::super::register_logic_rehydrate_pipeline(app);
            super::super::register_visual_rehydrate_pipeline(app);
        }
    }

    impl Plugin for LateRegistrationPlugin {
        fn build(&self, app: &mut App) {
            register_rehydrate_step(
                app,
                "test.late-build",
                RehydratePhase::WakeDomains,
                &["domains.wake"],
                &[],
                noop,
            );
        }
    }

    #[test]
    fn save_plugin_finish_freezes_the_exact_production_plan_after_all_builds() {
        let mut app = App::new();
        app.add_plugins((
            super::super::super::SavePlugin,
            ProductionAdapterPlugin,
            LateRegistrationPlugin,
        ));

        app.finish();

        assert_eq!(
            app.world().resource::<ResolvedRehydratePlan>().step_names(),
            vec![
                "construction.normalize",
                "deconstruction.floor-ownership",
                "familiar.settings",
                "power-consumer.policy",
                "soul-spa.normalize",
                "stockpile.policy",
                "transport-request.targets",
                "task-logistics.runtime",
                "deconstruction.runtime",
                "presentation.shells",
                "construction.runtime",
                "obstacle.runtime",
                "domains.wake",
                "test.late-build",
            ]
        );
        assert_eq!(
            app.world()
                .resource::<ResolvedRehydratePlan>()
                .validator_names(),
            vec![
                "deconstruction.orders",
                "durable.topology",
                "familiar.roster",
                "presentation.spatial-roots",
                "task-logistics.owners",
            ]
        );
        assert_eq!(
            app.world()
                .resource::<ResolvedRehydratePlan>()
                .prerequisite_names(),
            vec!["presentation.assets-time"]
        );
    }
}
