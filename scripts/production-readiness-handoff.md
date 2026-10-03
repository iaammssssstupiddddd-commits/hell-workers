# TAK-14 production-readiness worker handoff

This is a worker integration report, not an authoritative specification, acceptance
record, art decision or release. No builds, tests, analysis server, native launch,
performance run, asset generation, registry registration, commit or external write
was performed. Rust files were formatted. Coordinator validation is required.

## Implemented boundaries

- M2 `production_state` is inside the canonical `.buildingset` digest. Candidate
  admission and release receipts consequently bind the state values as part of
  the exact kind/generation/authority/manifest identity. The field is omitted
  when absent, preserving non-M2 and draft canonical bytes.
- Tank requires `{kind: Tank, partial_y_wu, full_y_wu}`, with finite
  `0 < partial < full <= 2 * TILE_SIZE`. Mixer requires `{kind: MudMixer, axis,
  radians_per_second}`, finite unit axis (squared-length tolerance 0.0001), and
  `0 < rate <= TAU`. These are schema safety bounds, not approved artwork values.
  ArtPreview rejects this field; its existing isolated clay motion remains
  separate. No production values are inferred from clay or supplied by default.
- The production consumer reads this validated state. Water visibility retains
  the existing logical Empty rule; Partial/Full heights come from the manifest.
  The rotor uses its authored quaternion and manifest axis/rate, advancing only
  while active with unpaused Virtual Time. Owner phase, generation handling,
  part ownership, fallback and save representation retain their existing paths.
- Existing M2 release/candidate manifests without a production state now fail
  closed, intentionally. They must be independently authored/reviewed and
  re-encoded/reapproved with the final values before use; this patch does not
  update, promote, install or approve any manifest. Non-M2 and Door do not gain
  a production-state field. The 2/4/8/9 binding admission rules are unchanged.

## Offline verifier and unavailable host registration adapter

`building_production_acceptance.py` provides `registry-contract`, `plan`, and
`verify-results`; there is deliberately no run/build/install/promote operation.
The old Bridge recipe still returns input integrity only and now points to this
separate result contract. Its eleven legs are checked independently, including
both banks, both lanes/directions, partial/complete load, cancellation stages,
ordinary construction versus Instant Build, deconstruction, non-movability,
quality/DPI views and ten cleanup/generation cycles.

The former free-standing `registry_export` format is no longer accepted.
`registry-contract` now reports the public host registration boundary and
`available=false`. The coordinator confirmed that the live catalog contains only
building-static-v1, building-art-v1, building-m2-interactive-v1 and
building-m2-generate-v1: **no production lifecycle recipe or reviewed instrument
adapter is available**. Both planning and result verification refuse before
creating a plan/output or consuming performance results. No success fixture,
production recipe, registration, launch intent or admission is generated here.

Saved context and capabilities use the host CLI `{ok:true,result:...}` envelope,
pinned by file SHA-256. The context contains controller, recipe and bridge
revisions, exact request subject (binding, run/consumer generation, generation,
repo, head and source), and owner. The saved register intent contains exactly
action/spec/command/subject/controller_sha256/recipe_revision. Public receipt
consistency checks compare its registration marker with that exact intent and
request UUID, including the host fingerprint: sorted compact JSON, default ASCII
escaping, SHA-256, **no trailing newline**. These are evidence consistency checks,
not authentication. Peer credentials, operator control, scheduling/storage and
the private host_admission marker remain solely broker-owned. The product never
reads, copies or fabricates that marker or broker secrets.

A future separately reviewed host recipe adapter must bind the declared command,
driver/verifier hashes, immutable plan path/digest, output roots, instruments,
batch phase and receipt to the current request. No such adapter or future recipe
schema is guessed in this patch. Even internally consistent saved records remain
rejected today; editing the saved catalog cannot enable a recipe. Existing
collector session admission also requires reconciliation with that future adapter
before use. Lifecycle/raw/performance predicate implementations remain in place,
but their existence does not make this boundary available.
`promotion_authority=false` applies to plans, observations and verification output.
A technical evidence verification does not approve art or release.

The canonical plan specification supplies:

- `scope`, absolute `repo`, clean `subject` (the existing building-art subject
  tuple), codec and driver `{path, sha256}`, separate Capture/Memory binaries,
  actual adapter/driver/Vulkan/x11/present_mode environment;
- saved `registration_context` and `host_capabilities` `{path,sha256}` records,
  `registration_request` UUID, exact `registration_intent`, and
  `gameplay_contract_sha256` (these inputs do not override recipe unavailability);
- released target inventories (`root`, exact `identity`, `manifest_file_sha256`)
  and two admitted released generation trials A/C per non-Door target, starting
  with the primary release. Unreleased candidate/draft inventories cannot pass;
- N and 4N cases, copies 4/16, normal `world_seed=20260920`, terrain hash and exact layout/logical-state/other-group/camera/
  population/activity hashes; non-target groups remain fixed within each pair;
- a finite strictly positive max-delta budget for every `METRICS` member, positive
  relative MAD, rationale and pre-run freeze timestamp;
- `baseline=null` for groups; for full ten, an explicit distinct-source baseline repo/subject and its
  Capture/Memory binaries. An unrelated baseline or same-binary foundation
  comparison is rejected. Group comparisons retain the candidate source/binary.

Door uses the existing dedicated g7/M6 inventory verifier rather than a new
Door loader or authority. Full requires all ten kinds; Bridge-free nine-kind
static passes cannot satisfy it. Existing static fixtures are not relabelled.

A result references immutable hashed artifacts relative to its own directory.
Every run binds the plan hash, fresh campaign/run nonce, exact subject, binary,
codec, driver, releases, generation trials, environment and time interval.
Owned X11 captures must carry matching pid/window/nonce, image and trace hashes,
sample indices and capture timestamps in an independent ACK artifact. Renderer
logs must identify the exact Vulkan adapter and contain no warnings/errors.
Lifecycle normalization must retain the underlying runtime observation artifact
and the complete, unchanged sample and event sequences. Each sample may add only
its exact ordinal `raw_sample_index`; every other sample field must match the
raw snapshot, including nested owners, roots, identities and states. All trace
fields other than the fixed session envelope and leg selector must likewise
match fields already present in that raw artifact, including domain events,
witnesses, cycles, production state and baseline/gameplay contract references.
Canonical JSON comparison also rejects boolean/number substitutions. The driver
cannot synthesize missing predicates, discard samples/events or remap indices.
No independent sidecar witness authentication is implemented or inferred from a
self-declared artifact hash. Missing raw fields remain a hard failure; the
current observer alone still cannot satisfy the full lifecycle contract.

Review fix `production-lifecycle-raw-binding` adds rejection tests for modified
owners, root/mesh references, identities, states, events, witness owners, cleanup
counts, production state, contract references, missing raw fields and reordered/
removed observations. These new tests were not executed by the worker. This
offline verifier/test-only revision has **No impact** on player-facing Help;
coordinator validation and fixed review remain required.

Performance is 24 serial processes: Capture then Memory, N then 4N, three
adjacent pairs per size with the middle order reversed. Warmup is at least 30s
and measurement at least 60s. The verifier computes p95/p99 from frame samples,
medians/MAD and deltas, checks RSS units against the resource-usage artifact and
native counters against the existing allocator CSV parser. Native values are
required only for Memory, timing decisions only for Capture. GPU measured and
estimated bytes, CPU mesh/image bytes, material shallow bytes and target root/
part/handle counts remain separate. Shared mesh/image/material counts must not
scale with N→4N; pending generations, retired references and orphan parts must
be zero after settlement. Control cannot preload target releases.

## Observations supplied versus still required

The profiling-only `HW_BUILDING_PRODUCTION_OBSERVATIONS` configuration accepts
`{nonce, output}` and records bounded ordinary-world snapshots (up to 1200,
512 target owners/roots/blueprints), input press/release, Save/Load outcomes and
deconstruction outcomes (4096-event cap). It never drives operations. The
Bridge snapshot is reused for actual terrain/owner/actor/consumer observations.
Pool-owned handle count is labelled separately from total application handles.
Resident mesh/image/material counts, decoded image bytes and material shallow
bytes are observed, with current pool identities and world epochs.

**Unobserved mesh allocator, native/RSS, GPU and total application-handle bytes/
counts remain null.** The diagnostic observer is not a timing/memory instrument
and cannot pass the performance verifier. It must be disabled for measurement.
Normal input/domain witness normalization, allocator/GPU collection, all required
lifecycle observations, capture/ACK production and host registry integration are
coordinator follow-ups. The raw observer alone cannot currently produce a passing
group/full result. This is deliberate fail-closed behavior, per coordinator reply
`msg_c93690899593`: this work supplements the verification contract and production
state consumer; missing GPU/native/input/lifecycle evidence must stay rejected.

## Help impact and validation handoff

Applied `hell-workers-review-help-impact` and read `docs/help-screen.md` with
coordinator permission. Decision for this worker delta: **No impact**. Existing
inventory/refining state → structural presentation state → water visibility/
position and rotor motion retains the existing player meaning; metadata now
supplies production geometry values. No input, task rule, label, notification,
public workflow or Help reachability is added. Observations are profiling-only,
and helper results do not publish assets. Invalid production state follows the
existing asset failure/fallback boundary. The coordinator must review the full
integration diff and run the Help gate; no passing gate is claimed here.

Added Rust regression coverage for missing/wrong-kind/nonfinite/out-of-range and
hash-stale state contracts, plus production water/axis motion and pause/part
ownership. Added Python contract tests for finite budgets, ordered matrix, all
Bridge legs, registry non-authority, ambiguous JSON and raw witness rejection.
These tests were **not run**. Coordinator owns formatting/policy checks, Rust
compile/Clippy/diagnostics, focused regressions and change-aware/full verification
on the exact integrated subject through primary `dev.py validation` and its
storage rules. Review the actual result schema and driver normalization with the
fixed reviewer before registry admission; the current helper is not a native
launcher or an already accepted driver.

Remaining product work: asset production and numeric/art choices, independent
approvals, real registered actual-window Capture/Memory and performance evidence,
release/install/rollback through existing authority, authoritative docs/Help final
review, index/plan lifecycle and storage checks. No job or native binary copy was
created or deleted by this worker; keep this candidate/source for coordinator
review and follow-up. This worker report must not be used to close TAK-14 itself.
