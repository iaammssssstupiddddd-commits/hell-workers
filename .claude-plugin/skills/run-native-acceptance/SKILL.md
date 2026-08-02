---
name: run-native-acceptance
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
  --audit /tmp/example/audit \
  --capture /tmp/example/capture \
  --memory /tmp/example/memory \
  --adapter Intel --backend vulkan --window-backend x11 --min-runs 3
```

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
  helper holds `/tmp/hell-workers-native-acceptance.lock` across the whole recipe.
- Use two Cargo jobs when `MemAvailable` is at least 12 GiB and one job below
  that. Refuse to start below 8 GiB available RAM, 15 GiB workspace free space,
  or 1 GiB `/tmp` free space.
- Keep `CARGO_INCREMENTAL=0` for profiling. Do not create per-feature target
  directories, copy the roughly 748 MiB binary into `/tmp`, enable profiling
  incremental state, add a new compiler cache, or run `cargo clean` routinely.
- Keep only small manifests, CSVs, and logs under `/tmp`. It is tmpfs on the
  standard workstation, so never place binaries or full Tracy traces there.
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
