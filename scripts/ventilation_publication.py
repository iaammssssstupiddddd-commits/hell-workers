"""Compare full and component-local publication using the same 4x HVAC solver."""
import argparse
import fcntl
import json
from pathlib import Path
import math
import statistics
import struct
import subprocess
import sys

from validation_storage import require_admission
from verify_ventilation_capture import read, require, verify_run

REPO = Path(__file__).resolve().parents[1]
STAGES = ("climate_cpu_ns", "input_ns", "prepare_ns", "step_ns", "publish_ns")


def schedule(profile):
    # Cover all 16 weather phases in the first pair, then reverse the order.
    rounds = (120, 60, 60) if profile == "capture" else (60,)
    for n, seconds in enumerate(rounds):
        for arm in (("full", "local") if n % 2 == 0 else ("local", "full")):
            yield f"round-{n+1}-{arm}", arm, seconds


def events(samples, size, arm, seconds, profile):
    air_groups = 1 if size == "large" else 64
    observations = {k: [] for k in ("steady", "weather", "one", "outage", "combined")}
    phases = set()
    for before, after in zip(samples, samples[1:]):
        if after["phase"] != "doors":
            continue
        def delta(key):
            return after[key] - before[key]
        weather = before["wind"] != after["wind"]
        if weather:
            tick = after["climate_tick"] - int(delta("climate_tick") > 0)
            phase = (tick // 30) % 16
            require(abs(after["wind"] - math.cos(phase*math.tau/16)) < 1e-12, "weather phase disagrees with wind")
            phases.add(phase)
        power = before["powered_fans"] != after["powered_fans"]
        one = before["high_fans"] != after["high_fans"]
        require(after["powered_fans"] in (0, 32) and after["high_fans"] in (0, 1), "invalid fan states")
        if sum((weather, power, one)) > 1:
            kind = "combined"
        elif weather:
            kind = "weather"
        elif power:
            kind = "outage"
        elif one:
            kind = "one"
        else:
            kind = "steady"
        groups, rooms, vents = (delta(k) for k in ("groups_changed", "rooms_written", "vents_written"))
        if kind == "steady":
            require((groups, rooms, vents) == (0, 0, 0), "unexplained ventilation write")
            if not delta("climate_tick"):
                continue
        else:
            require(groups > 0 and rooms > 0 and vents > 0, f"{kind} did not publish")
            if arm == "full":
                require((rooms, vents) == (256, 512), "full publication control")
            else:
                require((rooms, vents) == (groups * 256 // air_groups,
                                           groups * 512 // air_groups), "component size mismatch")
            if kind == "weather":
                require(groups == air_groups, "global weather did not update all connected buildings")
            if kind == "one":
                require(groups == 1 and abs(delta("inspected_net")) > 1e-7, "single fan failed to change airflow")
            if kind == "outage":
                require(groups == (16 if size == "small" else 1), "32 fans did not change their connected buildings")
        observations[kind].append({k: delta(k) for k in (*STAGES, "rooms_written", "vents_written",
                                                          "publish_allocations", "publish_bytes")})
    for k in ("weather", "one", "outage"):
        require(len(observations[k]) >= seconds * 4 // 30 - 1, f"incomplete isolated {k} transitions")
    require(len(observations["steady"]) > seconds * 3, "insufficient ordinary ticks")
    if seconds == 120:
        require(len(phases) == 16, "incomplete 16-phase weather cycle")
    measured = [r for r in samples if r["phase"] == "doors"]
    inspection_count = measured[-1]["inspections"] - measured[0]["inspections"]
    require(inspection_count > 0, "working vent inspector was never refreshed")
    counts = ("rooms_written", "vents_written", "publish_allocations", "publish_bytes")
    stats = {}
    for name, rows in observations.items():
        if not rows:
            continue
        item = {"n": len(rows), "max_cpu_ms": max(r["climate_cpu_ns"] for r in rows) / 1e6}
        item.update({k.removesuffix("_ns") + "_ms": statistics.median(r[k] for r in rows) / 1e6 for k in STAGES})
        item.update({k: statistics.median(r[k] for r in rows) for k in counts})
        stats[name] = item
    if profile == "memory" and arm == "full":
        require(stats["one"]["publish_allocations"] >= 512, "allocation scope did not observe eager strings")
    return {"events": stats, "weather_phases": sorted(phases), "inspections": inspection_count,
            "inspection_cpu_ms": (measured[-1]["inspection_ns"]-measured[0]["inspection_ns"]) / 1e6}


def verify_leg(directory, profile, arm, seconds):
    m = read(directory / "manifest.json")
    require(m["status"] == "valid" and m["source"]["unchanged"] and m["binary"]["instrumentation"] == profile,
            "invalid source, binary or game")
    require(len(m["cases"]) == 2 and {c["size"] for c in m["cases"]} == {"small", "large"}, "missing layout")
    results = {}
    for case in m["cases"]:
        size = case["size"]
        require((case["seed"], case["souls"], case["familiars"], case["render"], case["workload"]) ==
                (42, 8, 2, "gpu", "ventilation"), "wrong gameplay fixture")
        run, = (directory / "cases" / case["id"]).glob("run-*")
        env = read(run / "requested-environment.json")
        require(env["HW_AIR_PUBLICATION"] == arm and env["HW_AIR_OUTAGE"] == "mixed", "unexpected control")
        data = verify_run(run, 64 if size == "small" else 1, climate=True, seconds=seconds)
        report = read(run / "data/ventilation.json")
        require(report["mode"] == "cached" and report["full_publication"] == (arm == "full") and report["mixed"],
                "wrong measured arm")
        data.update(events(report["samples"], size, arm, seconds, profile))
        if profile == "memory":
            allocator = read(run / "profile-artifact.json")["allocation_memory"]
            require(allocator["accounting_errors"] == 0 and allocator["peak_live_bytes"] > 0, "invalid allocator")
            data["peak_live_bytes"] = allocator["peak_live_bytes"]
        results[size] = data
    signature = (m["source"]["fingerprint_start"], m["binary"]["sha256"], json.dumps(m["actual_adapters"], sort_keys=True))
    return signature, results


def compare_traces(baseline, candidate):
    room = struct.Struct("<6d4B")
    vents = struct.Struct("<512d")
    with baseline.open("rb") as left, candidate.open("rb") as right:
        for tick in range(130, 330):
            header = struct.pack("<Q", tick)
            require(left.read(8) == right.read(8) == header, f"missing paired tick {tick}")
            for zone in range(256):
                a, b = room.unpack(left.read(room.size)), room.unpack(right.read(room.size))
                require(a[6:] == b[6:] and all(math.isfinite(x) and math.isfinite(y) and abs(x-y) <= 1e-9 * (1+abs(x))
                        for x, y in zip(a[:6], b[:6])), f"paired climate, control or dilution mismatch at {tick}/{zone}")
            a, b = vents.unpack(left.read(vents.size)), vents.unpack(right.read(vents.size))
            require(all(math.isfinite(x) and math.isfinite(y) and abs(x-y) <= 1e-9 * (1+abs(x)) for x, y in zip(a, b)),
                    f"paired opening flow mismatch at {tick}")
        require(not left.read(1) and not right.read(1), "trailing trace")


def paired_paths(directory):
    manifest = read(directory / "manifest.json")
    return {c["size"]: next((directory / "cases" / c["id"]).glob("run-*/data/publication-trace.bin"))
            for c in manifest["cases"]}


def compare_pair(root, prefix):
    full, local = paired_paths(root / (prefix+"full")), paired_paths(root / (prefix+"local"))
    for size in ("small", "large"):
        compare_traces(full[size], local[size])


def verify(root, profile):
    signatures = set()
    results = {}
    for name, arm, seconds in schedule(profile):
        sig, results[name] = verify_leg(root / name, profile, arm, seconds)
        signatures.add(sig)
    for name, arm, _ in schedule(profile):
        if arm == "local":
            compare_pair(root, name.removesuffix("local"))
    require(len(signatures) == 1, "comparison changed executable, source or adapter")
    return {"identity": signatures.pop(), "runs": results}


def run_suite(root, profile):
    require_admission(REPO, [root])
    skill_dir = REPO / ".codex/skills/hell-workers-run-native-acceptance/scripts"
    sys.path.insert(0, str(skill_dir))
    import native_acceptance as native
    with native.LOCK_PATH.open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        resources = native.resource_snapshot(REPO, require_launcher=True)
        require(resources["status"] == "ready", str(resources["failures"]))
        env = native.cargo_environment(REPO)
        root.mkdir(parents=True, exist_ok=False)
        (root / "admission.json").write_text(json.dumps(resources))
        for name, arm, seconds in schedule(profile):
            gate = native.stage_start_resource_gate(name)
            (root / f"{name}-admission.json").write_text(json.dumps(gate))
            require(gate["status"] == "ready", str(gate["failures"]))
            command = [sys.executable, "scripts/perf.py", "run", "--workload", "ventilation", "--sizes", "small,large",
                       "--renders", "gpu", "--seed", "42", "--souls", "8", "--familiars", "2", "--repeat", "1",
                       "--preflight-runs", "0", "--warmup-secs", "30", "--measure-secs", str(seconds),
                       "--instrumentation", profile, "--adapter", "Intel", "--backend", "vulkan", "--window-backend",
                       "x11", "--present-mode", "novsync", "--window-width", "1280", "--window-height", "720",
                       "--window-scale-factor", "1", "--rtt-quality", "high", "--timeout-secs", "300",
                       "--output", str(root / name)]
            with (root / f"{name}.log").open("w") as log:
                subprocess.run(command, cwd=REPO, env={**env, "HW_AIR_PUBLICATION": arm, "HW_AIR_OUTAGE": "mixed",
                               "HW_AIR_COMPARE": "0", "HW_VENTILATION_SOLVER": "cached"}, stdout=log,
                               stderr=subprocess.STDOUT, check=True)
            verify_leg(root / name, profile, arm, seconds)
            prefix = name.removesuffix(arm)
            if (root / (prefix+"local")).is_dir() and (root / (prefix+"full")).is_dir():
                compare_pair(root, prefix)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("run", "verify"))
    parser.add_argument("profile", choices=("capture", "memory"))
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    if args.action == "run":
        run_suite(args.root.resolve(), args.profile)
    else:
        print(json.dumps(verify(args.root.resolve(), args.profile), indent=2))
