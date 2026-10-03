# TAK-14 M6 nine-kind worker handoff

Coordinator review artifact, not an authoritative specification, release, validation result, or M6 closure. Supplied base: `ba1df06caa70a68e6e07c49f356ede43da0538cb` (not independently checked with Git). No build, test, native run, performance measurement, analyzer, asset production/promotion/install/rollback, Git operation, or authoritative documentation edit was performed. File-local Rust/Python formatting was performed. All new code and tests need coordinator validation.

## Critical prerequisite correction

Coordinator answer `msg_f0726c770af3` to resumed question `msg_ef2026733a48` established that eight non-Door kinds do **not** yet have approved releases. M2's worker handoff says `accepted=false/runtime_published=false`; M3 says `art_approved=false/runtime_published=false`; M5 remains unproduced/unapproved/unreleased. The complete binding groups in `release.rs` are admission contracts, not approval records. Synthetic test identities must never be entered into the release ledger. M6/C2/C3/C5 cannot close from this patch.

Only Door g7 is identified as released: `release_approved`, manifest SHA-256 `4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449`; locator `manifests/door-production-v1.doorset`, file SHA-256 `21075f6b5c6e4412ccc17632ecc3b44bcb619bb901a06062fa0905c0be303002`; receipt `door_sets/7/authority/promotion-receipt.json`, SHA-256 `95ba6a19494d84825db54075cf8735d2024d18e6983dc54343cdc71c19a046ad`. These are coordinator-provided identity data, consistent with `scripts/m4_door_audit_handoff.md`, not fresh worker byte verification: the locator is absent in this checkout. No mirror was installed.

## Per-kind audit

The following is source inspection, not runtime acceptance. “Shared” refers to the paths detailed below. The eight non-Door rows have **no approved production generation/hash** to enumerate; their normal binding must eventually provide an exact `ReleaseApproved` identity and canonical locator.

| Kind | Normal authority / locator suffix | World and state | Consumer / lifecycle specifics |
| --- | --- | --- | --- |
| Tank | shared; `building-tank-v1.buildingset` | Meshless root, body/water leaves; water visibility follows stored content. Approved production motion parameters are absent; release keeps authored transforms. | Shared ghost, partner ghost, Blueprint, pulse, move destination, catalog; shared completion/save shell. Existing numerical fixture covers 0/1/full and capacity zero. Real contents and reservations remain gameplay-owned. |
| MudMixer | shared; `building-mud-mixer-v1.buildingset` | Meshless root, body/rotor leaves. Clay rotor motion is explicitly ArtPreview-only; production state motion is not approved. | Shared previews including move, completion/save shell. Existing refining/stop/pause/resume fixture checks reuse, but its motion is not production release evidence. Runtime work resumes according to existing rehydrate rules. |
| RestArea | shared; `building-rest-area-v1.buildingset` | Meshless root and body leaf. Occupancy is logical `RestingIn`; no invented mesh state or gameplay change. | Shared previews/completion/save shell; no new move admission. Empty/occupied native evidence still required. Existing M3 tests preserve resting ownership. |
| SoulSpa | shared; `building-soul-spa-v1.buildingset` | Body plus four fixed slot leaves. All 16 worker masks, slot order, construction visibility are existing contracts. | Dedicated site placement/construction/cancellation, not generic Blueprint completion. Shared projection/catalog; save rebuild restores the site then runtime workers are re-derived. Existing tests cover reverse tile order, reservations, empty slots and construction. |
| Door | dedicated g7 identity above; `door-production-v1.doorset` | One root mesh, EW/NS × Closed/Open/Locked; 3 meshes, albedo, 2 previews, one material. | Dedicated ghost/Blueprint/pulse/catalog systems and topology. Shared shell factory on completion/load; no new move action. Dedicated loading/fallback policy differs from equipment pending policy and is preserved. |
| WheelbarrowParking | shared; `building-wheelbarrow-parking-v1.buildingset` | Foreground2d child, dedicated image; no structural root. | Shared previews/completion/load. Initial facilities call the same shell factory; actual carts and capacity remain separate gameplay data. No move admission added. |
| SandPile | shared; `building-sand-pile-v1.buildingset` | Foreground2d dedicated pile image; resource/item images are separate. | Shared previews/completion/load. No resource amounts, placement, haul or walkability change. No move admission added. |
| BonePile | shared; `building-bone-pile-v1.buildingset` | Foreground2d dedicated pile image; resource/item images are separate. | Shared previews/completion/load. No resource amounts, placement, haul or walkability change. No move admission added. |
| OutdoorLamp | shared; `building-outdoor-lamp-v1.buildingset` | Foreground2d off/on images from the same generation; fallback restores current power tint. | Shared previews/completion/load. Presentation mirror observes gameplay power; load re-derives runtime power. No wiring, light/Room or save schema change. |

Canonical shared prefix is `manifests/`. Kind-to-slug admission remains defined by `BuildingAssetKind::slug`; this report confers no authority. There is no generic nine-kind Instant Build path: `debug_instant_complete_walls_system` handles wall sites and floor placement has its own toggle. Do not add instant equipment/Spa completion to satisfy a visual test. Initial Parking is an observed initial-placement route; no equivalent initial path is claimed for the other eight.

## Shared production and failure paths

- `assets/building_asset_set/{release,loader,validation,residency,pool}.rs`: explicit `HW_BUILDING_ASSET_RELEASES` exact bindings, canonical locator, authority and manifest/dependency validation. Candidate and art-preview are distinct admissions; none authorizes a release. Default startup without explicit bindings retains its existing fallback.
- `systems/visual/building_presentation/{structure,preview}.rs` and `plugins/visual.rs`: poll, structural sync, preview sync, deferred application after Door presentation and before transform/UI preparation. Consumers re-evaluate while paused. Existing preview paths include ghost, Tank companion, Blueprint, pulse, move destination and open/new catalog. Only existing Tank/Mixer move workflows are claimed; a synthetic consumer is not a new user action.
- `systems/jobs/building_completion/spawn.rs::attach_building_shell` and completion commit share the shell path with save rehydration. SoulSpa uses `interface/selection/soul_spa_place/spawn.rs` and `systems/jobs/soul_spa_construction/cancellation.rs`. Parking initial facilities live in `systems/logistics/initial_spawn/facilities.rs`.
- Equipment pool owns at most one active and one pending generation per kind. Incomplete or rejected pending generations preserve that kind's prior active set; publication waits for typed dependencies. Failed attempts consume the generation floor. Stale invalidation cannot remove the newer active generation. Missing/invalid active data restores legacy presentation. This is distinct from Door's Loading/fallback-on-replacement policy.
- Runtime lower/reused generation requests are rejected. Offline asset rollback means coordinator-owned locator/authority rollback and restart, not claiming that a running pool accepts A→B→A at the original lower number. Save transaction rollback is a different path, now tested with synthetic mixed shells.
- Owner cleanup uses existing child ownership, `cleanup_building_3d_visuals_system`, and `hw_visual::reset_for_world_replace`. Shared asset pool resources survive world replacement. Entity cleanup does not prove renderer/GPU deallocation.
- Save schema remains unchanged: Tank contents/rest relationships persist through existing serialization; Mixer work, Spa worker bindings/mask and Lamp power are re-derived by existing runtime rules. The added test deliberately uses the actual transaction API with a presentation-only test rehydrate step; it does not prove the entire production save graph.

## Changes and rationale

| Files | Change |
| --- | --- |
| `crates/bevy_app/src/assets/building_asset_set/release.rs`, `mod.rs` | Profiling-only `M6Comparison`. Require all eight exact release bindings in both modes. Candidate requests normal published sets; explicit legacy-control makes no requests for their manifests/dependencies. Reject Bridge/art-session mixing; ordinary startup unchanged. |
| `crates/bevy_app/src/assets/building_asset_set/pool.rs` | Expose profiling active/pending counts; namespace synthetic mesh/image handles by kind so mixed fixtures cannot falsely share IDs. |
| `crates/bevy_app/src/assets/building_asset_set/pool/tests.rs` | Simultaneous eight-kind pending validation, failed replacement and recovery without unrelated generation changes. Uses existing in-memory loader fixtures. |
| `crates/bevy_app/src/systems/visual/building_presentation/tests.rs`, `tests/mixed.rs` | Ten rounds of kind-local generation replacement/invalidation/fallback/recovery, Blueprint/pulse/catalog and world consumers, old-part cleanup and world reset. Door sentinel prevents equipment overwrite; dedicated Door tests still required. |
| `crates/bevy_app/src/systems/save/transaction.rs` | Nine synthetic shells, ten paused normal replacement/injected-failure rollback cycles, retained pool identities, recreated roots and foreground images. |
| `crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/mod.rs` | Extend the existing deterministic paused fixture to explicit M6 mode; exact Door identity, all eight pools, counts, parts and stable resources. Separate sidecar contract `building-art-m6-nine-v1`. Preserve original baseline layout, state, seed and old mode. |
| `scripts/perf_tool/arguments.py`, `argument_validation/density.py`, `execution.py` | Public `--building-m6-mode`; repeat=1 allowed only for explicit paired M6 runs; mode recorded in requested environment. |
| `scripts/perf_tool/artifact_readers/building_art_static.py`, new `building_m6.py` | Independent strict M6 oracle: identities, fixed records, transforms/visibility, roots/leaves/pools/resources, canonical hash and stable-frame checks. |
| `scripts/building_art_static_acceptance.py` | Historical verifier rejects the new contract; an M6 run cannot masquerade as the old baseline. |
| new `scripts/building_m6_acceptance.py` | Frozen-input plan/run/verify, released-byte inventory, paired 24-leg matrix, independent raw re-verification, median/MAD and explicit budgets, optional distinct-source historical baseline. |
| new `scripts/tests/test_building_m6_acceptance.py` | Synthetic positive/negative oracle, ordering, CLI, resource plateau, budget/noise/missing-repeat/binary/environment checks. |
| this file | Temporary coordinator handoff; adopt findings into primary documents before removing. |

No Bridge source, asset or acceptance baseline was changed. Existing per-kind visual implementations were not remade. No runtime state parameters were invented to bypass missing asset approvals.

## Coordinator focused validation (not run)

Use the primary validation/storage workflow and the frozen integrated subject. Commands below are proposed payloads for the coordinator's registered validation, not worker execution. Keep one Cargo job and one Rust test thread under existing guards.

```bash
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling m6_ -- --test-threads=1
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling building_presentation::tests -- --test-threads=1
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling door_preview::tests -- --test-threads=1
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling assets::building_asset_set -- --test-threads=1
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling systems::save::transaction::tests -- --test-threads=1
PYTHONPATH=scripts python3 -m unittest discover -s scripts/tests -p 'test_building_m6_acceptance.py'
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py ci check --base ba1df06caa70a68e6e07c49f356ede43da0538cb --mode auto
```

Also include profiling-enabled Clippy/compile in the change-aware scope because the new runtime mode is feature-gated. Existing relevant test names: `tank_state_changes_reuse_water_leaf_and_keep_fallback_materials`, `mixer_refining_stop_pause_resume_only_rotates_the_existing_rotor`, `spa_fixed_slots_follow_all_worker_masks_and_construction_phase`, `m3_consumers_generations_and_owner_cleanup_preserve_logical_state`, `lamp_images_replace_tint_and_invalidation_restores_current_power_state`, `rebuilt_lamp_shell_uses_retained_generation_and_current_mirror_while_paused`, `m2_exact_preview_generation_reaches_ghost_companion_blueprint_pulse_move_and_catalog`, `reused_door_equipment_wall_ghost_restores_geometry_and_candidate_stays_private`. These are scoped evidence, not substitute releases/native acceptance.

## Exact performance entrypoint and evidence scope

First finish actual M2/M3/M5 approvals and install/mount the exact canonical releases through coordinator-owned authority. Both control and candidate planning fail if a release is absent; do not turn failure into an asset-missing baseline. Supply the existing building codec executable and an explicit reviewed budget file. Define `M6_REPO`, `M6_CODEC`, `M6_BUDGET`, `M6_BATCH_SPEC`, `M6_PLAN`, `M6_JOB_ROOT`, and `M6_ADAPTER` to real absolute candidate/codec/JSON/job paths and the expected adapter substring. The plan must be outside the fresh job root. The batch spec uses the existing workflow keys `id/repo/owner/consumers/verify_command`; verifier argv is `python3 <candidate>/scripts/building_m6_acceptance.py verify --plan <absolute-plan>`.

Budget JSON must contain exactly `max_delta`, `max_relative_mad`, and a nonempty `reason`. `max_delta` must give finite nonnegative numbers for every key: `p95_ms`, `p99_ms`, `peak_live_bytes`, `max_rss_kib`, `asset_mesh_accessor_bytes`, `resident_mesh_count`, `resident_structural_material_count`, `resident_image_count`, `resident_image_cpu_bytes`, `structural_material_shallow_bytes`. No default permissive budget or adoption decision is supplied by this patch.

```bash
python3 scripts/dev.py validation plan --spec "$M6_BATCH_SPEC" -- python3 "$M6_REPO/scripts/building_m6_acceptance.py" plan --repo "$M6_REPO" --codec "$M6_CODEC" --budget "$M6_BUDGET" --adapter "$M6_ADAPTER" --job-root "$M6_JOB_ROOT" --output "$M6_PLAN"
```

Execute the returned registered `launcher_command` exactly, through the native skill's established no-prompt terminal route; do not invoke `run` directly or set the launcher marker manually. Monitor `job.json` heartbeat/failure and logs under the skill's fail-closed rules. On terminal success, independently verify:

```bash
python3 "$M6_REPO/scripts/building_m6_acceptance.py" verify --plan "$M6_PLAN"
```

The helper runs Capture first, then Memory, each with small N=36 and medium 4N=144, three adjacent pairs per size. Pair order is control/candidate, candidate/control, control/candidate. Each leg uses the existing `scripts/perf.py` guarded profiling feature/profile command, 30s warmup/60s measure, seed 20260920, GPU Vulkan/X11, 1280×720, DPI1, novsync, high RTT, 15/60 Souls, zero Familiars, and one repetition. Capture and Memory binaries must differ; each instrumentation's binary/environment must remain identical across its pairs. Inputs/source/asset hashes are frozen and rechecked; all raw runs are independently verified again at the end. No non-target set is requested by the M6 binding group; Bridge is excluded.

N/4N retain the existing nine-kind layout and paused baseline states. With k=4/16 copies per kind, target structural roots are 5k, foreground sprites 4k, candidate mesh entities 11k (control 5k), active equipment pools 8/0 (control 0/0 pending). Records independently enumerate parts and visibility. Resource counts/bytes must plateau across N/4N and remain stable throughout the observed session; cold-load timing is deliberately fail-closed and has not been exercised.

Frame p95/p99 and native peak/RSS are independently parsed in their appropriate legs; three-repeat medians/MAD are calculated without summing percentile differences. `resident_image_cpu_bytes` is decoded CPU image data; `structural_material_shallow_bytes` is count × Rust material value size; `asset_mesh_accessor_bytes` inventories exact GLB source accessor payloads used by the admitted assets. These are **not** total mesh allocator bytes or GPU allocation/driver residency. GPU bytes are explicitly unknown. Further renderer-specific byte/allocator evidence is needed for a complete budget; no fabricated conversion is used.

Optional historical comparison: add `--foundation-baseline "$M6_FOUNDATION_ROOT" --foundation-identity "$M6_FOUNDATION_IDENTITY"` to the plan command. This requires distinct original source/commit identities, common non-target asset bytes, the original frozen manifest, and its original verifier in its own source environment. It emits direct historical-vs-candidate Capture N/4N and Memory4N comparisons only where those original sessions exist. It does not add unrelated quantiles or claim missing historical N Memory/resource-byte evidence. A separately approved cumulative foundation budget remains necessary. Without this option the result is explicitly `same-source-paused-asset-increment-only`; even a passing result always retains `accepted=false/promotion_authority=false`.

## Required native storyboard beyond the paused matrix

Use the same integrated source/released identities and normal gameplay entrypoints. Record actual window, adapter/backend, target-visible pixels, state and owner/pool counters; preserve normal placement, resources, walking, power and save semantics. The static matrix is not an active gameplay observer and does not automatically execute this storyboard.

1. Place all nine kinds together through ordinary UI, plus initial Parking. Observe ghost → construction/Blueprint/pulse → completed world and existing/new catalog. Exercise Tank/Mixer real move destinations and Tank companion only where currently admitted. Door must include empty NS support, EW, both-pair tie and support removal. Verify same-generation images without color/anchor/validity regressions.
2. Tank 0/1/full, Mixer idle/refining/pause-resume, Rest empty/occupied, Spa construction and occupied 0/1/4 plus all 16 masks, Lamp off/on, Door both axes × three states, all four dedicated foreground images. Use existing gameplay or separately admitted diagnostic fixture; no gameplay edits. Production Tank/Mixer state art requires the unresolved M2 parameter approval first. The baseline's Tank 0/25/50/50, Spa 0/1/3/15, Door Closed/EW alone do not satisfy this step.
3. With mixed owners live and paused/unpaused, independently exercise late arrival, missing dependency, malformed/rejected pending manifest, valid newer generation, active invalidation and restart after coordinator-owned authority rollback. Verify unrelated kind identities and Door's dedicated loading policy. Synthetic generations do not authorize production mutation.
4. Save/load through the full production graph; inspect Tank content, Mixer runtime reset/resume, Rest ownership, Spa construction/slots, Door axis/state and Lamp re-derived power. Inject failure only via existing admitted transaction diagnostics; verify rollback, owner/part cleanup and no stale preview. Repeat owner removal/load enough to establish finite CPU and renderer pools; synthetic transaction test covers only shell reconstruction.
5. Existing `wall_door_joint_acceptance.py plan --repo <candidate> --adapter <adapter> --release --feedback` and `ui_usability_acceptance.py plan --repo <candidate> --smoke --input-backend none --layout-scene build` can provide scoped Door/catalog checkpoints through the registered native launcher. They are not complete nine-kind lifecycle observers. If normal input cannot expose a required transition, the coordinator must arrange a narrowly scoped observer; do not relabel unrelated screenshots as coverage.

## Reuse, limitations, Help and primary documents

Reuse unchanged asset provenance, reviewed numerical contracts and historical group evidence only for the original matching identities/conditions. Door g7 provenance is reusable; current mixed ghost/catalog/paused save/performance behavior needs current-subject evidence. M2/M3 synthetic and art-preview material is engineering reference, not approval. Bridge normal placement, dedicated baseline and native results are not nine-kind acceptance evidence.

Unverified: compilation/Clippy/all tests; the helper and native launcher path; all actual-window/state/lifecycle evidence; eight releases and approved production state parameters; full save graph; renderer/GPU lifetime and byte accounting; historical cumulative missing legs/budget; fixed reviewer approval, Help gate, CI and storage. A fail-closed fixture mismatch must be investigated, not weakened to pass. The worker found no justified normal gameplay change and claims no runtime regression fix from synthetic tests that have not run.

Preliminary Help decision: **No impact**, following `.codex/skills/hell-workers-review-help-impact/SKILL.md`. The new mode is explicit profiling/operator input; ordinary player controls, feature availability, build/move/save semantics, UI text and existing Help content are unchanged. Actual player path remains existing selection → placement/construction → shared presentation. The coordinator owns the final Help review and exact approval snapshot/gate. Native procedure follows `.codex/skills/hell-workers-run-native-acceptance/SKILL.md`; communications use `/home/satotakumi/.agents/skills/orchestration/SKILL.md`.

Primary documentation candidates (coordinator only): migration plan M6/A1–A10 status and missing release gates; `building-asset-sets.md` authority/group/mode and separate Door loading policy; `rendering-performance.md` paired metric definitions, CPU/source/GPU distinction and incomplete historical cumulative evidence; building lifecycle docs for initial Parking/Instant Build applicability; release/rollback records once real approvals exist; Help decision, plan/index synchronization and storage ledger. Keep the candidate/cache through feedback. No heavy job/binary was created by this worker; do not dispose of coordinator-owned caches. After adoption, remove this temporary handoff when it has no consumer, following the validation storage workflow.
