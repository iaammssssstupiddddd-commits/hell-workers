# Copilot Instructions for Hell Workers

## Build, test, and verification commands
- `cargo run` — run the game locally.
- `cargo check` — required compile/type verification before finishing changes.
- `cargo build --target x86_64-pc-windows-gnu` — Windows GNU target build used by this project.
- `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario --perf-log-fps` — high-load perf scenario from `docs/DEVELOPMENT.md`.
- `trunk serve` — optional WASM/web workflow (`Trunk.toml`).
- `cargo test` — run all Rust tests.
- `cargo test test_path_to_boundary_1x1_open -- --exact` — run a single existing test (`src/world/pathfinding.rs`).

## High-level architecture
- Runtime composition in `src/main.rs` is plugin-driven: `MessagesPlugin` + entity plugins + `Startup`, `Input`, `Spatial`, `Logic`, `Visual`, `Interface`.
- Frame update order is fixed by `GameSystemSet`:
  `Input -> Spatial -> Logic -> Actor -> Visual -> Interface`.
  (`Spatial`/`Logic`/`Actor` are paused together via virtual-time gating.)
- `StartupPlugin` builds world state in stages:
  - `Startup`: camera + asset catalog/resource initialization.
  - `PostStartup` chain: map/terrain spawn, initial resources, soul/familiar spawns, UI setup, and initial resource-grid population.
- `Logic` contains three coordinated phased subsystems with explicit `ApplyDeferred` boundaries:
  - Familiar AI: `Perceive -> Update -> Decide -> Execute`.
  - Transport requests: `Perceive -> Decide -> Arbitrate -> Execute -> Maintain` (between Familiar `Update` and `Decide`, then maintenance after Soul `Execute`).
  - Soul AI: `Perceive -> Update -> Decide -> Execute` (runs after Familiar `Execute`).
- Task/haul flow is request-driven:
  - Producers create `Designation` and anchored `TransportRequest` entities.
  - Familiar task finding reads both `DesignationSpatialGrid` and `TransportRequestSpatialGrid`.
  - Familiar emits assignment/reservation requests; Soul executes `AssignedTask` phase state machines.
- UI is slot-mounted:
  - `UiRoot` owns `UiMountSlot` containers (`LeftPanel`, `RightPanel`, `Bottom`, `Overlay`, `TopRight`, `TopLeft`).
  - `UiNodeRegistry` stores stable slot/entity mappings for update systems.
  - `UiInputState.pointer_over_ui` is shared input gating used by UI and camera control.

## Key codebase conventions
- Keep phase responsibilities strict:
  - **Decide** systems generate request messages.
  - **Execute** systems apply those requests and mutate ECS state.
- Register all new `Message` types in `src/plugins/messages.rs`.
- For `EntityEvent` reactions, prefer centralized plugin registration with `app.add_observer(...)`; do not double-register the same handler with spawn-time `.observe(...)`.
- When extending task execution:
  - Add new `AssignedTask` entries as struct variants in `src/systems/soul_ai/execute/task_execution/types.rs`.
  - Aggregate required queries in `TaskQueries` / `TaskAssignmentQueries` (`context.rs`), not ad-hoc system-local query sprawl.
  - Use `TaskExecutionContext` for shared execution data flow.
- Transport hauling uses anchor request patterns; keep request lifecycle/cleanup consistent with transport request systems.
- UI input gating is centralized in `UiInputState.pointer_over_ui`; camera/input guards should use this shared state.
- Keep existing project rules from `docs/DEVELOPMENT.md` and `CLAUDE.md`: no dead code stubs, keep `cargo check` clean, and prefer `From/Into` over widespread `as` casts.
- Asset workflow convention: generate with magenta background (`#FF00FF`) and convert via `python scripts/convert_to_png.py ...`.
