# TAK-14 M4-Door worker handoff

This is a coordinator handoff artifact, not an authoritative specification or an acceptance record.
Base/observed HEAD: `2cc387bfd84e7a517d46e8f9e661681275208e45`.
Worker scope: static audit and minimal source repair; no builds, tests, native runs, release operations, Git metadata writes, or docs/assets edits.

## Classification and change

- Existing g7 asset: retain. No evidence found requiring remodelling, retexturing, reprojection, or a new release.
- Runtime gap: ordinary placement on an empty NS-supported cell incorrectly selected EW. `WallTopologyIndex::connection_mask(target)` returns `None` unless the target itself is a connector. A ghost is deliberately not a connector; a Blueprint/completed Door is one.
- Repair: only `crates/bevy_app/src/systems/visual/door_preview.rs`. Add `placement_preview_axis`, reading the four neighbors through the existing read-only topology accessor and feeding the existing axis resolver. Keep Blueprint/world readers, Wall topology/dirty sets, EW tie-break, unsupported EW default, placement validity, tint and anchors unchanged.
- Regression additions in that same file: empty NS cell, both support pairs, EW pair, support removal, no ghost topology mutation; extend the existing paused catalog test to NS Blueprint root, pulse child and empty-cell ghost across late readiness, g7/g8, identity mismatch, loading and rejected states. Synthetic g8 is a unit fixture only, not a new asset release.
- Only formatting was run (`rustfmt --edition 2024` on the edited file). All regression cases remain unexecuted by the worker.

## Asset inventory and provenance

The worker checkout has no installed g7 locator/mirror. Coordinator message `msg_3fced7a78ff9` provided fresh primary byte/hash verification; the worker separately read that exact locator/receipt, inspected GLB JSON metadata and viewed the three PNGs. No mirror was created.

Root: `/home/satotakumi/projects/hell-workers/assets/`.
Locator: `manifests/door-production-v1.doorset`, SHA-256 `21075f6b5c6e4412ccc17632ecc3b44bcb619bb901a06062fa0905c0be303002`.
Identity: `door-production-v1`, generation `7`, `release_approved`, `art_approved`, manifest `4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449`, normal `not_used_by_design`.

| Role | Path relative to asset root | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Closed | `door_sets/7/models/door_closed.glb` | 15256 | `6f2ceebf51a9ba26dc31a4a199362fab7926eeeeadf4cf62756adf3bdcc202ab` |
| Open | `door_sets/7/models/door_open.glb` | 15252 | `15ff1a62e4eb82217160ceaf29d90073ce37984523f108e11d45807fc7c42fe2` |
| Locked | `door_sets/7/models/door_locked.glb` | 16096 | `bc27862ffe06e48d5c3c9ba3f2263d73ba62df745df0b2752195c8bc5d94cbb0` |
| Albedo | `door_sets/7/textures/buildings/door/door_albedo.png` | 447937 | `a2010af11bb4d2fa7f32f96f62b33c18907e368dc7818efc795e93215d08a211` |
| EW preview | `door_sets/7/textures/buildings/door/door_preview_ew.png` | 34160 | `f52babe5734a36ba88c67a35cd7c3e4f96b900c2aa35ee70fef22de937c87bd3` |
| NS preview | `door_sets/7/textures/buildings/door/door_preview_ns.png` | 37079 | `e8a8eeac87432ff4b69f2d76cee2476970992ff529e5599a26707811ba26d827` |
| Receipt | `door_sets/7/authority/promotion-receipt.json` | 1198 | `95ba6a19494d84825db54075cf8735d2024d18e6983dc54343cdc71c19a046ad` |

Receipt ID `door-g7-preview-20260912-053506`, manifest/new_active/generation agree with the locator and preview repair plan. Approval reference is `evidence/release-approval.json`, hash `7f2a48c3d7514137e220e8f4fc7d93421632fdcaf48c718c7b7f3503549ee44f`; this audit creates no new approval.

GLB JSON: one node/mesh/primitive each, POSITION/NORMAL/TEXCOORD_0; triangle counts Closed/Open/Locked `204/204/216`, matching `tools/blender_ai_workflow/fixtures/door-production-v1.geometry.json` and below 240. Local X/Y bounds are ±16; Open max Z is 9.5159187. No embedded images/materials: the actual consumer loads `Mesh0/Primitive0` and supplies the shared runtime structural material/albedo. This matches the exporter/validator path; the old plan's phrase “one material” should describe runtime material ownership, not require a new GLB material.

PNG inspection shows distinct EW/NS projections, dark wood, pale bone reinforcement and restrained iron detail; albedo is a material atlas rather than a projected building picture. This is static inspection, not new actual-window art approval or a rerun of geometric/UV validators.

## Consumer audit (static source findings, not runtime passes)

| Surface | Actual path / conclusion |
| --- | --- |
| Admission | `assets/door_asset_set.rs`: canonical locator inventory, generation paths, receipt/core byte/hash checks; explicit candidate policy, release approved normal path; resolves exactly 3 meshes + albedo + 2 previews. |
| Activation | `update_door_asset_readiness_system`: all six required load states and shared material gate eligibility. One resolved identity and one shared material slot; generation replacement removes previous material and replaces handles. This dedicated Door loader uses Loading/fallback on incomplete replacement, not the new equipment pool's keep-old-active pending policy. It is unchanged. |
| Completed 2 axes × 3 states | `building3d_cleanup::sync_door_presentation_system`: matching readiness/resolved/material identities; state selects mesh index 0/1/2; topology selects EW/NS; production transform keeps fixed frame independent of state. No semantic state or WorldMap write. |
| Ordinary ghost | `placement_ghost_system` supplies existing validity/color/position; `sync_door_preview_system` supplies g7 EW/NS Closed image. Empty-cell axis bug repaired here only. |
| Blueprint and pulse | Same preview system maps root topology axis, applies same eligible identity/image/64wu canvas/anchor to root and referenced pulse child each frame. Root/child color remains owned by construction/pulse systems. |
| Open/new catalog | `hw_ui::setup::submenus` creates marked cards; `sync_door_catalog_preview_system` checks every Door card each PostUpdate, uses eligible Closed EW or legacy `door_closed`, and leaves UI size/color/input/text alone. |
| Ordering/pause | `plugins/visual.rs`: topology → Door readiness → Door presentation before transform propagation; catalog before UI Prepare. No Virtual-Time run condition. Common equipment preview sync follows Door and skips its final ghost/card image. |
| Late arrival/switch/rejection | Consumers re-read identity each presentation frame, not just Changed owner. Failed/loading/mismatched readiness restores fallback sprites/world. Added preview assertions exercise the consumer side; loader/I/O and GPU readiness are not proven by synthetic handles. |
| Load rebuild | `systems/save/rehydrate.rs` reuses `attach_building_shell`; `hw_visual::reset_for_world_replace` removes transient 3D owners and resets topology, leaving Door asset pool resources intact. First subsequent presentation reselects saved Door state and rebuilt axis. No save schema edit. |
| Finite pool | 3 production mesh handles, 3 image handles, 1 shared structural material; 1 resolved generation in application pool. State switches select handles rather than allocate. Resident GPU lifetime and repeated-load plateau still require coordinator evidence. |
| Owner cleanup | `cleanup_building_3d_visuals_system` despawns roots by removed Building owner; world reset collects Building3dVisual entities; Blueprint pulse is a ChildOf child. No cleanup changes. |
| Move | No new Door move workflow is introduced; existing move admission stays authoritative. Equipment move consumers are outside this change. |

## Evidence reuse boundary

The preview repair plan records g7's three GLBs/albedo as byte-identical to g6 and the two new preview hashes above. Its `0d532b34a3fefbea5fbbcfe798b4b4d68dfb8836` J1 record (Intel Arc / Vulkan / X11 / High / DPI1, 6 screenshots) supports unchanged projection, Blueprint/support/load observations for that historical subject only. Retain that historical asset provenance; do not relabel it as current-head ghost/catalog, new performance, or new art acceptance. The repaired empty NS placement path was not covered by the old target-cell observer. Do not reopen old Door quality/density tracks or Bridge solely to close this minimal change.

## Coordinator validation and native recipe

1. On the exact integrated source, run focused `door_preview::tests`, normal `dev.py check`, workspace Clippy `-D warnings`, and change-aware CI/local scope against the intended full base SHA through the primary validation coordinator. Worker ran none of these and did not start rust-analyzer.
2. Use native skill admission, same candidate/cache hold and the returned no-prompt launcher. Keep g7 installed via coordinator-owned asset view; worker checkout does not contain it. Freeze source/asset/binary identity for independent verify.
3. Mandatory changed-path storyboard: real ordinary Door placement at an empty NS-supported cell, EW-supported cell, both-pair tie-break and support removal; verify chosen preview matches subsequent Blueprint/pulse/completed orientation without changing validity, footprint, tint or anchor. Include paused placement and load rebuild. Capture target-specific visible pixels, not only surrounding wall variance.
4. Catalog: `ui_usability_acceptance.py plan --smoke --input-backend none --layout-scene build` is an existing static entrypoint under the native skill. Capture existing/new Door cards and exact g7 EW image. It alone does not prove late-arrival, invalidation or ordinary ghost behavior; pair real input/observer evidence with focused regressions, or add a coordinator-owned narrowly scoped observer if the chosen harness cannot record those transitions.
5. Existing `wall_door_joint_acceptance.py plan --repo <candidate> --adapter Intel --release --feedback` can supply six Blueprint/support/load feedback checkpoints; registered independent verifier requires `--feedback`. It has no ordinary placement ghost/catalog observer and cannot by itself close items 3–4. Formal J1 removes `--feedback`; avoid unnecessary whole legacy matrix replay. Record renderer/adapter/window identity and limitations separately from static/consumer tests.
6. No new mesh/material allocation path was added. Confirm finite pool/owner cleanup using current scoped evidence; do not claim new Memory or frame-time results from this audit. Coordinator decides any required bounded performance leg, performs formal Help review, fixed read-only review, storage check, docs sync and integration.

## Help and authoritative documentation handoff

Worker preliminary Help recommendation: **No impact**. `SelectBuild` → existing placement validation → Door ghost changes only the image axis to match existing supports; controls, availability, placement results, Door lock/Room/light/save semantics, UI text and the `architect-building` Help entry remain unchanged. This recommendation follows the Help skill and actual provider/consumer inspection; the coordinator owns the formal decision/gate.

Update candidates for coordinator: the overall migration plan's M4 audit/result and remaining native scope; `building-asset-sets.md` dedicated Door distinction; production Door plan's empty-cell ghost rule/runtime material wording; preview repair plan's historical evidence boundary. Keep old track/Bridge statuses intact. Archive/remove this temporary handoff only after its findings are adopted; it is not a replacement for authoritative docs.
