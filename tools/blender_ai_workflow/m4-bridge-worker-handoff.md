# M4 Bridge implementation handoff — not acceptance or release evidence

Task `task_d55aaf0517aa`, Dispatch `ctx_b60922d88fa3`, supplied base
`d40c3b78ba76f3c32d51a062dd57228a473e348a`.
The revised ticket explicitly adds `crates/hw_ui/src/selection` to the write set.
No build, test, analysis server, Blender/export/codec execution, native launch,
Git command, commit, promotion/install/rollback/release, or external write ran.
Only source authoring and file-local rustfmt (edition 2024, skip_children) ran.
No GLB/PNG/.blend bytes were generated; their generators and proposal are ready
for coordinator execution, following the same source-handoff boundary as M3/M5.

## Decisions and contracts

- Readiness was an approved investigation, not a product fix: the old UI test
  intentionally reproduced width 2–4 versus all-River 2×5 failure. The coordinator
  clarified that adopting its exact live-terrain contract belongs to M4.
- `hw_ui::selection::resolve_bridge_crossing` is the one production resolver:
  each of two adjacent live logical terrain columns must have one nonempty
  contiguous River interval; their union must fit five rows. Anchor Y is minimum
  River Y minus floor((5-span)/2). Remaining spare rows go north. It checks every
  ordered deck and outside-bank cell for bounds, building, stockpile and raw
  blocker, requires dry external banks and existing walkability on non-River
  deck cells. It does not search past blockers or edit terrain.
- Existing Bridge `RiverYMin` shape, order, center offset (0.5,2), 2×5 size,
  gameplay navigation, actor height, costs, Room, save schema and UI wording are
  unchanged. Resolver failures use existing typed rejection reasons. Missing/gap
  uses NotRiverTile; span/blocker/wet-bank/nonwalkable uses NotWalkable.
- Root `live_building_geometry` feeds both normal ghost and commit. The shared
  validator resolves again from live state and rejects altered, incomplete,
  duplicate/reordered footprints. For rejected crossings only, the red preview
  uses the cursor Y; it cannot be committed. Site/Yard admission remains unchanged.
- Bridge now has an explicit asset kind, body mesh, albedo/world_preview/catalog,
  Complete representative state, one active and one pending pool slot. Canonical
  paths, receipts, exact profiling candidate admission and typed residency gates
  are reused. No candidate locator is added to normal assets.
- Completion and save-rebuilt shells both use the existing shared factory:
  Bridge becomes a meshless EquipmentRoot with one body/fallback child. Production
  root is ground-centered; fallback retains legacy bridge mesh/material and
  0.09-tile center height independently of bounce scale. One shared opaque
  material is retained per Bridge generation; no unused Spa material is allocated.
- The common descriptor reaches ghost, Blueprint, construction pulse and catalog;
  save-restored Blueprint receives it at the same paused PostUpdate boundary.
  Generation replacement, invalidation and owner cleanup use existing shared code.
- Release bindings add an explicit complete nine-set group (the prior eight sets
  plus Bridge; Door remains separate). Partial/substituted groups remain rejected.
  This is wiring only, not release authorization or installation.
- Bridge authoring uses a dedicated low wooden deck with two edge iron straps,
  ground origin and exact 64×160 WU extent. Proposed deck top 0.6 WU / strap top
  0.9 WU are **not frozen**. There are 144 triangles (cap 192), one body role,
  a padded face atlas, long-edge-oriented strokes and separate world/catalog
  renders from the existing calibrated 59°/RtT-corrected projection. Reject the
  model if current-height Soul feet/held items fail at banks, ends or center.
  Do clay passage before numeric freeze, planes and strokes. No elevation rule
  or river change is permitted as an art fix.

## Changed source inventory

All paths are repository-relative; some files also received local rustfmt.

- `crates/hw_ui/src/selection/{mod.rs,placement.rs,placement/validation.rs,placement/tests.rs}`:
  resolver API, validation and rejection/forged-footprint fixtures.
- `crates/bevy_app/src/interface/selection/{placement_geometry.rs,bridge_readiness_tests.rs,building_place/placement.rs}`:
  common live geometry, generated-terrain and production commit tests.
- `crates/bevy_app/src/systems/visual/placement_ghost.rs`: normal ghost geometry.
- `crates/bevy_app/src/assets/building_asset_set/{mod.rs,schema.rs,validation.rs,pool.rs,tests.rs,pool/tests.rs,release.rs,acceptance.rs}`:
  Bridge input/generation/release wiring; generic typed loader/pool cases now include Bridge.
- `crates/bevy_app/src/assets/building_asset_set/acceptance/bridge_probe.rs`:
  opt-in read-only normal-world observations, no seeded construction/completion.
- `crates/bevy_app/src/systems/visual/building_presentation/{mod.rs,structure.rs,tests.rs}`:
  Bridge descriptor/root/fallback and generation/consumer/cleanup tests.
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`: shared Bridge factory.
- `crates/bevy_app/src/systems/save/rehydrate/tests/construction.rs`: paused Blueprint shell contract.
- `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`: Bridge non-movable intent test only.
- `tools/blender_ai_workflow/scripts/building_asset_pipeline.py` and
  `tools/blender_ai_workflow/tests/test_building_asset_pipeline.py`: Bridge production
  inventory and complete future release group (test fixtures are not approval).
- `tools/blender_ai_workflow/fixtures/building-bridge-v1.geometry.json`.
- `tools/blender_ai_workflow/scripts/{building_bridge_art.py,create_building_bridge_scene.py,build_building_bridge_candidate.py}`.
- `tools/blender_ai_workflow/tests/test_building_bridge_art.py`.
- `scripts/building_art_acceptance.py`: refuses Bridge in the old nine-kind static recipe.
- `scripts/building_bridge_acceptance.py` and `scripts/tests/test_building_bridge_acceptance.py`:
  independent frozen declarative recipe and input-boundary tests.
- This handoff.

## Exact coordinator quality and authoring steps (not executed)

Use primary validation/storage coordination, one heavy slot/job/test thread,
with this candidate source and its existing cache held for review. No assertion
below is a passing result. Start with formatting/check/Clippy and focused tests:

```sh
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py cargo -- test -p hw_ui bridge_
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 bridge_
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 building_asset_set
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 building_presentation
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 move_intent_rejects_stale_non_movable_and_pending_targets
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling bridge_
python3 -m unittest scripts.tests.test_building_bridge_acceptance
python3 -m unittest discover -s tools/blender_ai_workflow/tests -p test_building_bridge_art.py
python3 -m unittest discover -s tools/blender_ai_workflow/tests -p test_building_asset_pipeline.py
```

Also include existing M2/M3/M5 authoring, release-binding and static recipe Python
regression in change-aware scope. Run the normal analyzer diagnostic check at the
coordinator; none was started here.

Coordinator-only staged authoring commands (substitute allocated paths/names):

```sh
python3 tools/blender_ai_workflow/scripts/build_building_bridge_candidate.py build Bridge --name <fresh-clay-name> --stage clay
python3 tools/blender_ai_workflow/scripts/build_building_bridge_candidate.py verify Bridge --name <fresh-clay-name>
python3 tools/blender_ai_workflow/scripts/build_building_bridge_candidate.py prepare Bridge --name <fresh-clay-name> --generation <allocated-generation> --destination <fresh-isolated-view> --codec <same-source-codec>
```

`build` invokes guarded Blender and staging GLB export, then geometry/UV/glTF and
projection checks. `prepare` allows clay for actual-game passage, as well as later
planes/strokes; every output remains **ArtPreview**, no implicit approval. Repeat
with fresh names after clay/numeric acceptance for planes and strokes. Runtime
output is `<fresh-isolated-view>/runtime`. Never run these in the worker role.

Freeze explicit positive budget JSON fields `frame_p95_ratio_max`,
`frame_p99_ratio_max`, `rss_delta_bytes_max`, `native_bytes_delta_max` before:

```sh
python3 scripts/building_bridge_acceptance.py plan --repo <frozen-subject> --bridge-runtime <runtime> --codec <same-source-codec> --subject-sha256 <source-fingerprint> --budget <frozen-budget.json> --output <fresh-plan.json>
python3 scripts/building_bridge_acceptance.py verify --plan <fresh-plan.json>
```

This follows the existing M3/M5 declarative recipe model. It freezes source,
codec, exact Bridge identity, geometry/preview/part contracts, all eleven legs and
performance budget, but **only verifies input integrity**. It is not a native
runner, automatic gameplay driver, acceptance verifier or release evidence.

## Native, performance and art execution required

Apply `hell-workers-run-native-acceptance` with the primary `dev.py validation`
registered launcher; actual game/native execution remains coordinator-owned.
Create an isolated asset view using the exact runtime manifest/artifact inventory,
then an exact `HW_BUILDING_ART_SESSION` (mode art-preview, full identity, canonical
locator, unique 32-hex nonce, fresh absolute status_path). Start an ordinary world
with `HELL_WORKERS_WORLDGEN_SEED=20260920`, **no building-art-static workload**.
Set `HW_BRIDGE_ACCEPTANCE_PROBE` to one of the recipe's eleven leg IDs.

Probe writes `<status_path stem>.bridge-trace.json` every 0.5 seconds, bounded to
600 samples and 128 owners/actors/consumers. It records logical crossings, bridge
owners/bits, River walkability, Soul positions, body roots/parts, preview/pulse/
catalog image handles and expected descriptor handles. It never places a building,
adds zones, supplies materials, completes work or emits a pass. Use bounded legs
under five minutes; only actual UI input and domain outcome/capture evidence can
prove ordinary placement, actual Soul passage, construction, cancel, save/load
and deconstruction. Missing/capped/failed traces are not success. Save/Load domain
outcomes/world epoch and action provenance must be captured separately; the probe
alone does not authenticate them.

Execute each explicit recipe leg: placement from both banks, passage in both lanes,
adjacent Bridge, ordinary construction, Instant Build separately, cancellation
before/after delivery, partial/completed save/load paused/resume, deconstruction
restoring River nonwalkability and preserving neighbor, non-movable intent,
gallery/quality/DPI, and ten-round generation/cleanup. Compare ordinary ghost,
Blueprint/pulse, catalog and completed world at the same generation. Capture
legacy, late, failed/missing and invalidated paths, not just the active body.

Before measured runs disable the diagnostic probe. Run Bridge-specific N=4 and
4N=16 control/candidate, then cumulative ten-building scenes on the same native
source/binary/layout/activity/camera/population/GPU/backend/warmup. Discover only
legal unmodified generated crossings; insufficient placement capacity is failure,
not permission to alter terrain. Keep other building generations fixed and do not
preload candidate Bridge roles in control. Capture and Memory are sequential,
repeat three, compare frame quantiles, native bytes/RSS, mesh/image/material bytes
and finite roots/parts/handles to frozen budgets. A root-child infrastructure cost
comparison additionally requires distinct source builds, not only same-binary A/B.

Actual adapter/owned window and renderer evidence, clay passage, numeric freeze,
UV/strokes, independent art adoption, candidate authority and eventual release
remain separate gates. Old nine-kind M6, static fixtures, map-only tests and
fixture-created completion are inapplicable as Bridge normal-placement evidence.
The generic building-art promotion evidence verifier cannot accept Bridge via its
old static recipe; a proper independent native evidence adapter is still required
before any future promotion, outside this worker's release authority.

## Help impact and remaining coordinator work

Applied `hell-workers-review-help-impact`, read `docs/help-screen.md`, traced
Architect normal input → live geometry → shared validator → Blueprint/preview,
and reviewed `architect-building` plus `building-type::bridge` coverage.
Decision: **Update required**, because Bridge placement becomes reachable on normal
width 2–4 generated rivers and its bank/obstacle conditions change. This cannot be
an internal-only/no-impact claim. Existing stable inventory and Published target
can remain; the existing architect-building entry needs the new placement and
completion/passage/cancel/deconstruction explanation. Worker did not change UI
copy because the task explicitly preserves it and snapshot generation is a
forbidden test execution. Coordinator must resolve that copy boundary, update the
provider, regenerate/review the exact snapshot with the documented ignored test,
run Help checks and update authoritative docs in the same integration batch.

No authoritative docs, snapshots, binary assets or approval state were changed.
Coordinator owns validation results, fixed review, Help update, primary docs and
indexes, asset generation and art acceptance, native/performance execution,
release evidence adapter/promotion/install/rollback/release, storage check and
integration/commit. Preserve this candidate/cache for feedback; no generated
storage was created or removed here. The new Rust/Python paths are uncompiled and
untested; generated art and all acceptance gates remain unverified.
