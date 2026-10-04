# M5 worker implementation handoff (not acceptance or release evidence)

Dispatch: `ctx_80ff040a2a22`; task: `task_01e2916333b3`.
Assigned base: `43cf907fbbeaf96f2de424c3b0c4187a166d3371` (coordinator-supplied).
This report is a worker handoff, not an authoritative docs update.

## Read-only audit

Read AGENTS.md and skimmed README.md, docs/DEVELOPMENT.md and docs/README.md;
read the migration plan M5 section, building-asset-sets.md and
building-art-direction.md. The existing generic M1-b/M1-c mechanisms already
cover all four M5 kinds; do not add independent image handles to GameAssets.

| Kind | Current fallback | Dedicated roles / invariant |
| --- | --- | --- |
| WheelbarrowParking | `textures/items/wheel_barrow/wheel_barrow_parking.png`, 64×64 WU | `world`, `catalog`; empty parking only, real vehicle is a separate entity |
| SandPile | `textures/resources/sandpile/sandpile.png`, 32×32 WU; shared sand item icon | `world`, `catalog`; no depletion or resource/item changes |
| BonePile | `textures/resources/bone_pile/bone_pile.png`, 32×32 WU | `world`, `catalog`; do not replace bone item icon or legacy resource image |
| OutdoorLamp | BonePile image, 32×32 WU; fallback catalog is glow_circle | `world_off`, `world_on`, `catalog`; existing PoweredVisualState remains authority |

Audited paths (all repository-relative):

- `crates/bevy_app/src/plugins/startup/asset_catalog.rs`: fallback and item/vehicle handles; unchanged.
- `crates/bevy_app/src/assets.rs`: catalog fallback; unchanged.
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`: common completion/load shell, Foreground2d classification and center anchor; unchanged.
- `crates/bevy_app/src/systems/jobs/building_completion/post_process.rs`: source/parking/power attachment; unchanged.
- `crates/bevy_app/src/systems/visual/building_presentation/preview.rs`: completed owner/VisualLayer child, normal ghost, Blueprint, late pulse, synthetic MovePlantTask, placement/move ghost and open BuildingCatalogPreview all resolve the same pool descriptor; only catalog uses catalog role.
- `crates/bevy_app/src/plugins/visual.rs`: PostUpdate pool → structure → previews → ApplyDeferred after Door and before transform/UI preparation; continues during pause.
- `crates/hw_ui/src/setup/submenus.rs`: 32px square catalog ImageNode. Dedicated square PNG contains the artwork, so no UI text/layout/API change is required.
- `crates/hw_visual/src/power.rs`: legacy Changed<PoweredVisualState> tint in Update; descriptor sync restores white RGB in PostUpdate. No changes to mirror or power writer.
- `crates/bevy_app/src/plugins/logic.rs`: power transaction applies topology/allocation/Unpowered before visual consumers; mirror observers are existing hw_jobs exports. Their defining source is outside worker read scope and was not expanded.
- `crates/bevy_app/src/systems/save/rehydrate.rs`, `rehydrate/construction_shells.rs`, `rehydrate/presentation.rs`: common shell restoration and runtime reconstruction; pool is not reset. Missing Lamp mirror resolves off until existing runtime mirror is rebuilt. No save-schema changes.
- `crates/bevy_app/src/assets/building_asset_set/{schema,release,pool,residency}.rs`: exact authority/identity, one active plus pending per kind, failed pending preserves active, retirement drops strong handles, explicit invalidation restores consumer fallback.
- `crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/{mod,seed}.rs`, `scripts/building_art_static_acceptance.py`, `scripts/building_art_acceptance.py`: existing paused static fixture and guarded native route; static evidence does not prove live power, save/load or art acceptance.
- Existing `build_building_m3_candidate.py`, `building_asset_pipeline.py`, M3 acceptance recipe and shared contract were used as the packaging/identity pattern. Wall/Door release dispatch stays unchanged.

## Changes

1. `crates/bevy_app/src/assets/building_asset_set/release.rs`: explicit complete M2 (2), M2+M3 (4), or M2+M3+M5 (8) release groups; duplicate, partial group, candidate authority and noncanonical locator rejection. Missing environment remains no-op. This grants no approval.
2. `crates/bevy_app/src/assets/building_asset_set/residency.rs`: `world_on` now shares the decoded canvas check with `world_off`; previously only declared representative world and catalog roles were checked.
3. `crates/bevy_app/src/assets/building_asset_set/pool/tests.rs`: Lamp on/off role dimension regression.
4. `crates/bevy_app/src/systems/visual/building_presentation/preview.rs`: preserve current alpha when restoring Lamp fallback tint on invalidation.
5. `crates/bevy_app/src/systems/visual/building_presentation/tests.rs`: all four kinds across world/ghost/Blueprint/pulse/synthetic destination/open catalog generations, invalidation and owner cleanup; run Lamp legacy tint writer before descriptor sync; alpha and paused rebuilt-shell coverage. Synthetic mirror and shell tests are not real supply-loss or save/load evidence.
6. `tools/blender_ai_workflow/scripts/building_asset_pipeline.py`: explicit `bindings --release-group m2|m3|m5`, default remains m2; each member requires existing matching released pointer and manifest.
7. `tools/blender_ai_workflow/fixtures/building-m5-v1.contract.json`: dedicated image roles, fixed 256px RGBA canvas/center anchor and existing 64/32 WU extents, art briefs, protected images and acceptance constraints. This is an unapproved production proposal.
8. `tools/blender_ai_workflow/scripts/build_building_m5_candidate.py`: import original world PNGs only from staging, reject wrong dimensions/clipping/visible magenta, require identical Lamp alpha and a visible RGB state difference; preserve exact world bytes, derive only catalog via centered contain. Embed originals in source identity and reuse existing export/codec for ArtPreview. Verify original bytes, derived catalog, manifest inventory and contract provenance. Never promote/install.
9. `scripts/building_m5_acceptance.py`: protected-image inventory and frozen four-kind recipe, exact candidate/source/codec hashes, before/after protected inventory and independent gates. Gallery, lifecycle and N/4N/cumulative performance remain unaccepted.
10. `scripts/tests/test_building_m5_candidate.py`: unrun focused tests for malformed raster rejection, role separation, state silhouette, staging/symlink boundaries, original byte preservation, approval separation, complete release bindings and protected image drift.
11. This handoff report.

## Image non-modification evidence candidates

Worker did not open/write any product image or generate any candidate raster.
No `assets/`, docs, save, gameplay, hw_ui or hw_visual files were edited.
Coordinator should compare the seven protected paths from the new contract
against the approved base, and use `building_m5_acceptance.py inventory` before
candidate work. The subsequent recipe compares that immutable baseline with
the actual asset root both at plan and verify. Inventory is evidence of bytes,
not proof of art quality or absence of a vehicle painted in a new image.

## Coordinator execution sequence

No command in this section was executed by the worker.

1. Run repository formatting/check/clippy, focused Rust `building_asset_set` and
   `building_presentation` tests, and Python `scripts.tests.test_building_m5_candidate`;
   include existing pipeline/M2/M3 tooling regression. Use the validation coordinator
   and primary storage workflow; no ad hoc heavy execution.
2. Capture protected inventory from the approved asset view with
   `python3 scripts/building_m5_acceptance.py inventory --asset-root <view> --output <staging>/m5-before.json`.
3. Create dedicated originals under `<workflow-asset-root>/staging/...` using the
   contract art briefs. Parking/SandPile/BonePile each require `world.png`; Lamp
   requires `world_off.png` and `world_on.png`. Fixed world canvas is 256×256,
   ground anchor [128,128], WU extents unchanged. The helper does not invent art.
4. For each kind, use `python3 tools/blender_ai_workflow/scripts/build_building_m5_candidate.py prepare <kind> --source <staging-originals> --name <fresh-name> --generation <allocated-generation> --codec <existing-binary>`.
   Output is `<workflow-asset-root>/staging/exports/building-m5/<name>` only.
   `verify <kind> --runtime <output>/runtime --codec <binary>` checks the same originals and export.
5. `building_m5_acceptance.py plan` requires all four `--parking-runtime`,
   `--sand-runtime`, `--bone-runtime`, `--lamp-runtime`, plus `--codec`,
   `--asset-root`, `--protected-baseline`, `--subject-sha256`, `--repo`, and staging `--output`.
   Its `verify --plan` is input integrity only, not native acceptance.
6. Use the native-acceptance skill's registered no-prompt launcher for exact-kind
   building_art_acceptance galleries and the recipe's actual gameplay legs.
   Capture/Memory are sequential, one host heavy slot, one Cargo job/thread.
   Observe Lamp allocation loss, producer removal, paused load off and resume
   recalculation; compare logical lighting radius/effect separately from pixels.
   M5 kinds are not made player-movable; move tests cover the generic consumer only.
7. Freeze N/4N and cumulative nine-kind limits before comparison. Match binary,
   population, camera/zoom, simulation activity, GPU/backend, warmup and measurement.
   Record frame quantiles, native memory, image bytes, finite handles and owner counts.
8. Independent art approval permits existing pipeline export as isolated_candidate;
   existing candidate-mode native evidence and separate release approval feed
   pipeline plan/apply, recover, install and rollback. This helper bypasses none of
   those gates. `bindings --release-group m5` emits the eight exact released identities
   only after all approved members are installed; Door remains its existing pipeline.
9. Help actual review, fixed review, independent verify, docs/index updates, release
   records, storage check and integration/commit remain coordinator-owned.

## Help and documentation handoff

Read the Help impact skill and help-screen spec. Proposed `No impact` rationale
for coordinator review: dedicated M5 image identities and state image selection
preserve existing building commands, input, placement/source/power/light/save
semantics and UI text; opt-in complete released bindings are an operator input,
not a new player workflow. This is a recommendation, not completed Help approval
or a passing gate. No Help snapshot/provider/coverage was edited.

Read the native acceptance skill for the handoff recipe; no job or launcher was
started. Authoritative docs candidates: building-asset-sets.md (8-member release
group, world_on dimensions), building-art-direction.md (approved M5 original
contract after art decision), assets_workflow.md (staged raster import/contain),
building-art-static-reference.md and performance docs (actual measured scope),
the migration plan M5/M6 and generated indexes after accepted closure.

## Explicitly unverified / unfinished

No format checker, compilation, rust-analyzer, tests, codec, native window,
performance job, candidate image production, art approval, promotion, install,
rollback, formal release, storage validation, commit or Git metadata action was
run. All authored tests require coordinator execution. Actual paused save/load,
real supply recalculation and performance/art quality are outstanding. This
handoff completes an implementation candidate only, not C5/C7/C8 acceptance or
the M5 formal-release milestone.
