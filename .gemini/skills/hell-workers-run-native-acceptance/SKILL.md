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

## Run the RtT-light migration recipe

Use this path for the frozen `rtt-light-v1` baseline. Do not substitute a
generic Task Dashboard run, a headless audit, or a RenderDoc screenshot for one
of its required legs.

Run the prerequisites in order on the same clean subject commit and source
fingerprint:

1. Run the Task Dashboard S0 recipe and retain its valid job root.
2. Generate an RtT S1 plan, execute its returned direct `kitty` command, and
   retain its valid job root:

   ```bash
   PYTHONDONTWRITEBYTECODE=1 python3 \
     .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
     plan-rtt-light --repo "$PWD" --level s1 --adapter Intel --window-backend x11
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
     plan-rtt-light --repo "$PWD" --level formal --adapter Intel --window-backend x11 \
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

The formal job is 64 sequential game processes under the repository lock:
audit, behavior, Capture, one fixed RenderDoc replay capture, and Memory. It
settles after behavior and RenderDoc, retains every artifact, and registers an
attempt only after the offline bundle validation passes. Revalidate a registered
attempt without launching the game with:

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
  free on the actual Cargo target filesystem; do not use `/tmp` capacity as a
  build budget. While a stage is running, sample `MemAvailable` every second
  and terminate that stage's isolated process group if RAM falls below 8 GiB.
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
