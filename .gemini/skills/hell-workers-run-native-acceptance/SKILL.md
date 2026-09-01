---
name: hell-workers-run-native-acceptance
description: Run hell-workers native acceptance and renderer, GPU, or performance validation through the established no-prompt terminal launcher, with sequential Capture and Memory builds, fail-closed artifact monitoring, and bounded resource use.
---

# Run Hell Workers Native Acceptance

Use the repository performance runner as the source of truth while keeping the
launcher, feature order, resource budget, and artifact checks deterministic. Do
not turn a successful headless run into renderer evidence.

## Choose the path

1. Use the bundled `task-dashboard` recipe when validating Task Dashboard CPU,
   allocation, or the hidden / visible / active-filter contract.
2. Do not use that performance recipe as proof of pixel layout, animation,
   pointer/keyboard handling, pause behavior, or another interactive visual
   contract. Define the expected observation and use a dedicated actual-window
   scenario through the same no-prompt launcher.
3. For another workload, apply the same execution contract below to
   `scripts/perf.py`; keep the actual-window command behind the established
   direct `kitty --directory ... --detach` launcher.
4. Use a headless run only for fixed-step correctness or a CPU-only route smoke.
   Require X11 or Wayland plus an exact backend and adapter for renderer, GPU,
   present, or frame-time evidence.
5. Treat the default 1-second warm-up and 2-second measure as acceptance smoke.
   Use the repository's documented 30/60-second matrix when a formal baseline or
   regression percentage is required.

## Run the Task Dashboard recipe

From the repository root, generate a fresh, non-mutating plan:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-task-dashboard --repo "$PWD" --adapter Intel --backend vulkan \
  --window-backend x11
```

Choose adapter, graphics backend, and window backend from the user's explicit
target or a compatible valid baseline. `Intel / Vulkan / X11` is the proven
standard-workstation recipe, not evidence by itself; only the completed manifest's
actual adapter/backend and requested window backend satisfy that evidence.

Review the compact JSON. It includes resource measurements, the unique artifact
directory, the exact `launcher_command`, and the bounded `status_command`.

Execute `launcher_command` directly. Its first executable must remain `kitty` so
the already-established launcher approval applies. Do not wrap it in `bash`,
`sh`, Python, command substitution, or another permission boundary. Do not ask
the user for a fresh display or GUI permission.

Poll `status_command` every 15–30 seconds. A status exit code of 2 means work is
still running; 0 means every artifact and comparison passed; 1 means invalid,
failed, or stale. Keep user updates under 60 seconds apart during long builds.
Do not read complete build or game logs while polling. On failure, inspect only
the reported error and a bounded tail of `orchestrator.log`.

The recipe performs this sequence under one repository-wide lock:

1. Capture-flavor fixed-step headless audit.
2. Short settle, then actual X11/Wayland Capture session and comparison.
3. Memory-flavor build, short settle, then actual-window native allocator / GNU
   time Memory session and comparison.
4. Cross-check source fingerprint, instrumentation, binary hashes, adapter,
   backend, dashboard modes, repeat counts, and comparison status.

It never passes `--skip-build` or `--binary`. Cargo may report the second
Capture-flavor check as fresh, but each validated session still owns its build
contract. The Memory feature switch occurs only after Capture validation.

To revalidate existing artifacts without launching the game:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  verify-artifacts \
  --audit target/native-acceptance/example/audit \
  --capture target/native-acceptance/example/capture \
  --memory target/native-acceptance/example/memory \
  --adapter Intel --backend vulkan --window-backend x11 --min-runs 3
```

## Run the player-facing notifications recipe

Use the dedicated Track A2 profile for typed placement rejection presentation,
save/load outcome notifications, bounded dedupe, pause-independent toast expiry,
toast pick-through, and blocking notification history.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-notifications --repo "$PWD" --seed 20260802 --adapter Intel \
  --backend vulkan --window-backend x11 --present-mode novsync
```

Run only the returned direct `kitty` command and poll its `status_command`.
The profile uses the regular game window and production placement tooltip plus
notification adapter/reducer/presenter. It isolates save and settings roots
under the job runtime, pauses `Time<Virtual>` while waiting for real-time toast
expiry, and saves an in-game screenshot containing the final tooltip, toast,
history panel, and PASS banner.

Revalidate a completed job with the exact values recorded in `job.json`:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  verify-notifications --repo "$PWD" --artifact <job-root>/artifact \
  --runtime-root <job-root>/runtime --run-id <run-id> \
  --harness-fingerprint <harness-sha256> --adapter Intel \
  --backend vulkan --window-backend x11
```

## Run the Save Catalog recipe

Use the dedicated C2 profile for manual-slot, recovery, autosave transaction, and
actual-window Save Catalog acceptance. Do not substitute a generic `perf.py`
run or a root-desktop screenshot.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-save-catalog --repo "$PWD" --seed 20260802 --adapter Intel --backend vulkan \
  --window-backend x11 --present-mode novsync
```

Run only the returned direct `kitty` command, then poll its `status_command`
every 15–30 seconds. The recipe first drives V1–V5 through the production
`UiIntent → catalog/modal → capture → Last dispatcher` route, then runs the
sequential Capture and Memory save-transaction matrix (small/medium/large,
3 preflight + 20 measured operations each).

Save Catalog screenshot evidence is deliberately **X11-only** until a
comparable per-window Wayland capture contract exists. The monitor finds the
launched Cargo process subtree, matches its X11 client window through
`_NET_WM_PID`, and captures that one client window with `import -window`. It
does not fall back to the root desktop, so a separate window or overlay cannot
satisfy the catalog marker check. The acknowledgement and final result retain
the fixed capture scope, X11 window ID, and owner PID. `xprop` and ImageMagick
`import` are required preflight tools for this profile.

To revalidate a completed C2 job without launching the game, use the exact
fingerprints and paths recorded by its `job.json`:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  verify-save-catalog --repo "$PWD" \
  --artifact <job-root>/artifact --runtime-root <job-root>/runtime \
  --performance-root target/perf-runs/save-transaction-<run-id> \
  --source-fingerprint <source-sha256> --harness-fingerprint <harness-sha256> \
  --seed 20260802 --run-id <run-id> --adapter Intel --backend vulkan \
  --window-backend x11
```

## Run the P02 actual-window presentation matrix

For the P02 TopDown subject, run the dedicated production-fixture matrix in
addition to S0/S1/formal. It launches the game-owned `indoor-light/p02/static`
fixture for High/Medium/Low × DPI 1.0/1.5/2.0 × Render3d visible/hidden and
captures only the X11 client owned by the launched process tree:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/p02_presentation_acceptance.py \
  plan --repo "$PWD" --adapter Intel
```

Run only the returned direct `kitty` command. The v9 validator requires all 18
cases and ten phase-tagged bounded client-window PNGs per case. Its 30-second
measurement window leaves time for every ACK-held storyboard phase after the
normal warm-up. Rust holds each
generation until the launcher writes a matching nonce/generation ACK after the
PNG is captured, so a screenshot cannot be attributed to a stale phase. The
validator recomputes ROI pixel predicates for Door state, Wall/Soul depth and
alpha, Bridge visibility, Wall completion bounce, and Foreground animation;
then cross-checks Vulkan/X11 evidence, raw performance validation, PNG hashes,
and the production P02 exactly-one / state sidecars as true invariants. The
Rust sidecar requires Door, Bridge, and Wall visuals to be visible in GPU
cases and hidden in CPU cases. The Bridge probe additionally requires a
Camera3dRtt-compatible render layer, the production Bridge mesh/material
handles, and resident assets; a headless run,
`visual_test`, root-display capture, or incomplete matrix cannot satisfy this
profile. Before planning, provision the ignored runtime asset mirror into the
clean worktree; v9 hashes that full local asset view at plan, rechecks it before
every case, and binds it to the manifest/revalidation. Revalidate with `verify
--job-root <job-root>`.

## Run the wall-density Capture matrix

Use the dedicated wall-density profile while freezing or comparing production
Wall performance. It runs the exact `wall-density-v1` Small=N and Medium=4N
fixture for completed and provisional walls, with three 30/60-second runs per
case on High/DPI 1/Vulkan/X11:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/wall_density_acceptance.py \
  plan --repo "$PWD" --adapter Intel
```

Run only the returned direct `kitty` command and poll `status --job-root
<job-root>` every 15–30 seconds. The profile rejects dirty subjects, source or
harness drift, asset-view drift, wrong window/adapter settings, missing runs,
and any raw fixture/layout/CSV mismatch. It recalculates validation and records
the p95/p99 median and MAD for all 12 Capture runs. This profile intentionally
reports `draw_groups=not-collected`; it does not satisfy the Wall M0 RenderDoc
draw-group gate until that separate evidence is added. Revalidate a completed
Capture bundle with `verify --job-root <job-root>`.

## Run the wall-density RenderDoc matrix

Use the separate wall RenderDoc profile for the M0 draw-group gate. It captures
completed and provisional Small=N / Medium=4N as four sequential Vulkan/X11
processes from one read-only `profiling-renderdoc` binary capsule, then replays
every RDC twice with the wall-specific extractor:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/wall_renderdoc_acceptance.py \
  plan --repo "$PWD" --adapter Intel
```

Run only the returned direct `kitty` command and poll `status --job-root
<job-root>` every 15–30 seconds. The profile binds the clean subject, source and
harness fingerprints, RenderDoc tools, capsule, runtime fixture checkpoint,
RDC, and two normalized replays. It accepts completed walls only when
`D_N <= 6`, `D_4N <= 6`, and `D_4N == D_N`; provisional walls require
`D_4N <= 4 * D_N + 6`. This draw-only profile complements rather than replaces
the 12-run wall-density frame-time matrix. Revalidate a completed bundle with
`verify --job-root <job-root>`.

## Run the current Wall actual-window calibration

Use the Wall calibration profile to capture the current production fallback
from the normal `wall-density-v1` spawn route without reusing the historical
P02 stage selector:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/wall_art_acceptance.py \
  plan --repo "$PWD" --adapter Intel
```

Run only the returned direct `kitty` command and poll `status --job-root
<job-root>` every 15–30 seconds. The M0 calibration profile binds one
High/DPI 1/Vulkan/X11 client PNG to the game-owned Wall entity, resident
fallback mesh, projected ROI, fixture checksum, clean subject, source,
harness, binary, and complete asset-view fingerprints. It is a current-source
visual reference only: it does not replace the registered historical P02
artifact, the 12-run Wall Capture baseline, or the separate RenderDoc
draw-group gate. Revalidate with `verify --job-root <job-root>`.

## Run the RtT-light migration recipe

Use this path for the frozen `rtt-light-v1` baseline. Do not substitute a
generic Task Dashboard run, a headless audit, or a RenderDoc screenshot for one
of its required legs.

### Run the bounded P08 release closure

Use `closure` as the default P08 completion path when the implementation and
workspace gates already pass and a fresh final-state renderer check is needed.
It intentionally does not rebuild the historical `current -> p01 -> p02 -> p06
-> p07` performance baseline chain.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-rtt-light --repo "$PWD" --level closure --stage p08 \
  --adapter Intel --window-backend x11 \
  --renderdoccmd /usr/bin/renderdoccmd --qrenderdoc /usr/bin/qrenderdoc \
  --renderdoc-library /usr/lib64/renderdoc/librenderdoc.so
```

Run only the returned direct `kitty` command and poll its status. The bounded
closure uses two feature builds and exactly three game starts: one preflight
and one measured medium/GPU Capture run, followed by one sealed
`profiling-renderdoc` capture. It requires the final P08 GPU pipeline and
receiver bindings to validate, and requires CPU publication, GPU upload, Soul
recovery, and Room summary to agree on the same epoch/revision checkpoint.

Closure evidence is deliberately not registered as a frozen performance
baseline. It does not claim historical frame-time or RSS comparability, and it
does not satisfy the 25-case/86-process formal matrix. Use the formal path below
only when the user explicitly requests a historical performance audit or a new
registered baseline and accepts that resource budget.

### Run the historical frozen formal matrix

Run the prerequisites in order on the same clean subject commit and source
fingerprint. Set `<stage-id>` explicitly (`current` for the frozen reference,
`p01` for the Scene-only P01 subject, `p02` for the TopDown presentation subject); do not rely on the compatibility default:

1. Run the Task Dashboard S0 recipe and retain its valid job root.
2. Generate an RtT S1 plan, execute its returned direct `kitty` command, and
   retain its valid job root:

   ```bash
   PYTHONDONTWRITEBYTECODE=1 python3 \
     .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
     plan-rtt-light --repo "$PWD" --level s1 --stage <stage-id> \
     --adapter Intel --window-backend x11
   ```

   S1 is a 51-process smoke: fixed audit, then Capture and Memory over the
   three size × CPU/GPU matrix. It verifies Capture/audit binary identity,
   Memory binary separation, actual Vulkan adapter, exact window backend, and
   artifact matrix before it becomes a formal prerequisite.

3. Only after S0 and S1 are valid, generate the formal plan with the required
   correctness ancestor and actual RenderDoc tool paths:

   ```bash
   PYTHONDONTWRITEBYTECODE=1 python3 \
     .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
     plan-rtt-light --repo "$PWD" --level formal --stage <stage-id> \
     --adapter Intel --window-backend x11 \
     --prerequisite-commit <full-correctness-sha> \
     --s0-job-root target/native-acceptance/<s0-job> \
     --s1-job-root target/native-acceptance/<s1-job> \
     --renderdoccmd /path/to/renderdoccmd --qrenderdoc /path/to/qrenderdoc \
     --renderdoc-library /path/to/librenderdoc.so
   ```

Treat a `blocked` plan as a stop condition. Formal execution requires the frozen
contract, a clean committed subject, same-source S0/S1 evidence, resource
preflight, and usable RenderDoc tools. Run only the returned `launcher_command`
directly; poll its `status_command` every 15–30 seconds.

The formal job is 65 sequential game processes under the repository lock. It
seals the `profiling-renderdoc` binary, runs one actual-game RD0 capture from the
same-source S1 environment before the long matrix, then runs audit, behavior,
Capture, one fixed formal RenderDoc capture, and Memory. It keeps the binary in a read-only
capsule and requires RD0/formal capsule identity plus normalized double-replay
topology equality. It settles after behavior and formal RenderDoc, retains every
artifact, and registers an attempt only after the offline bundle validation
passes.
The RenderDoc capsule must use both the `profiling-renderdoc` Cargo feature and
the `profiling-renderdoc` Cargo profile. The profile inherits the profiling
optimization level, enables debug assertions because wgpu-hal otherwise disables
its RenderDoc bridge, and disables LTO with 16 codegen units to bound build RAM.
Do not substitute the ordinary
`target/profiling/bevy_app` output.

Reuse the same clean validation worktree and its workspace `target/` for retries.
With an unchanged profile/features, Cargo's freshness check must reuse the existing
artifacts; Python harness or documentation changes do not justify a new Cargo target
or a full Rust rebuild. The common subject fingerprint hashes source and asset content
plus logical locators, not worktree paths or asset mtimes, so an identical detached
checkout remains the same subject. A profile/feature/toolchain change is the explicit
exception that requires rebuilding and resealing the affected capsule.

Revalidate a registered attempt without launching the game with:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  verify-rtt-light --repo "$PWD" --attempt target/perf-runs/rtt-light/<contract>/<generation>/attempts/<uuid>
```

Report audit, behavior, Capture, RenderDoc, and Memory independently. Include
the actual adapter/backend, subject commit, artifact attempt path, and any
blocked prerequisite; never call S1 or an unregistered attempt a formal
baseline.

## Preserve the no-prompt boundary

- Use the direct `kitty` launcher already established for this repository. The
  helper must run as its child; the helper must not try to launch `kitty` itself.
- If `kitty` is unavailable, do not request broad or repeated elevation. Run only
  the safe headless portion if useful, label actual-window acceptance incomplete,
  and report the single concrete launcher blocker.
- Never treat `--dry-run`, headless llvmpipe, or a software-adapter warning as
  proof of the requested renderer or GPU.
- Freeze relevant Rust, Cargo, asset metadata, and performance-runner files while
  the recipe is active. The helper fails if their fingerprint changes.
- Preserve invalid and interrupted artifacts for diagnosis. Never overwrite an
  existing output directory.

## Keep memory and disk bounded

- Keep Cargo builds, game processes, Capture, and Memory fully sequential. The
  helper holds only its tiny `/tmp/hell-workers-native-acceptance.lock` across
  the whole recipe.
- Use two Cargo jobs only when `MemAvailable` is at least 16 GiB and one job
  below that. Refuse a native recipe start below 10 GiB available RAM or 15 GiB
  free on the actual Cargo target filesystem. Immediately before each build,
  game, capture, or replay stage, require 8 GiB `MemAvailable` and record the
  admission snapshot. The 8 GiB threshold is a stage-start gate: once admitted,
  do not terminate that stage solely because `MemAvailable` later dips below
  8 GiB. Do not use `/tmp` capacity as a
  build budget. Continue to enforce stage deadlines and owned-process-group cleanup.
  SwapTotal/SwapFree are recorded as diagnostic telemetry only: a low or
  unavailable swap balance does not block a run while the RAM floor is met.
  On Linux, unavailable `MemAvailable` is a failure, not an exemption.
- Every native build and game process must set `CARGO_TARGET_DIR` to the
  repository `target/`, `CARGO_INCREMENTAL=0`, and `TMPDIR`/`TMP`/`TEMP` to
  `target/.native-acceptance-tmp`. `CARGO_HOME` and `RUSTUP_HOME` retain only
  safe persistent overrides; inherited tmpfs values are replaced with the
  account defaults. The helper normalizes these values; do not run a raw Cargo
  command or pass an alternate target for acceptance.
- Job roots and all performance artifacts belong under
  `target/native-acceptance/` or `target/perf-runs/`. The plan rejects an
  explicit `/tmp` or memory-backed job/artifact path and reports
  legacy `/tmp/hell-workers-*-target` directories by allocated size without
  deleting them. Formal RenderDoc capture/replay staging belongs in
  `target/.renderdoc-tmp`.
- `scripts/perf.py` and `scripts/dev.py` apply the same disk-backed target,
  temporary-directory, toolchain-cache, and one/two-job normalization, and
  refuse Cargo compilation below 8 GiB `MemAvailable`. Swap counters remain
  diagnostic telemetry and do not add a second start gate while the RAM floor
  is met. They validate default and explicit artifact roots, including resolved
  symlink/mount paths; Tracy, csvexport, and RenderDoc child processes inherit
  the controlled temporary directory. Do not bypass them with inherited Cargo
  variables or a copied binary.
- Use native allocator counters plus GNU time for routine Memory evidence. Do not
  run full Tracy allocation traces or RenderDoc unless the task explicitly needs
  those distinct measurements.
- Do not apply `nice`, `ionice`, or CPU affinity to formal measurements; they
  change the timing conditions.
- Do not automatically delete artifacts or caches. If capacity is low, report
  exact directory sizes and ask for a separate cleanup decision.

## Report the result

Report the three independent outcomes: fixed correctness, actual renderer
Capture, and native Memory. Include the actual adapter/backend, valid run counts,
artifact root, and whether the short acceptance or formal timing matrix was used.
State any incomplete leg explicitly. Never summarize an invalid session as a
pass because individual runs happened to finish.

## Validate this Skill

After changing the Skill or helper, run:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  self-test
python3 scripts/check_agent_rules.py
```

Also run the current product's Skill Creator `quick_validate.py` against
`.codex/skills/hell-workers-run-native-acceptance`.
