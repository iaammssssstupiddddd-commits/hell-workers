"""Run and verify the paired 4x native ventilation solver comparison."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

from validation_storage import require_admission
from verify_ventilation_capture import read, require, verify_run

REPO = Path(__file__).resolve().parents[1]
ROTATIONS = (("reference", "cached", "replay"), ("cached", "replay", "reference"),
             ("replay", "reference", "cached"))


def schedule():
    yield "trajectory", "record", 120, False
    for round_id, modes in enumerate(ROTATIONS, 1):
        for mode in modes:
            yield f"round-{round_id}-{mode}", mode, 60, False
    yield "weather-cycle", "cached", 120, False
    yield "power-cycle", "cached", 60, True


def verify_leg(directory, mode, seconds, outage):
    manifest = read(directory / "manifest.json")
    require(manifest["status"] == "valid" and manifest["source"]["unchanged"], "source or game failure")
    require(manifest["binary"]["instrumentation"] == "capture", "capture build required")
    cases = manifest["cases"]
    require(len(cases) == 2 and {c["size"] for c in cases} == {"small", "large"}, "missing layout")
    stats = {}
    for c in cases:
        require(c["workload"] == "ventilation" and c["render"] == "gpu"
                and (c["seed"], c["souls"], c["familiars"]) == (42, 8, 2), "unexpected fixture")
        run, = (directory / "cases" / c["id"]).glob("run-*")
        env = read(run / "requested-environment.json")
        require(env["HW_VENTILATION_SOLVER"] == mode and env["HW_AIR_OUTAGE"] == str(int(outage)), "run mode")
        data = verify_run(run, 64 if c["size"] == "small" else 1, climate=True, seconds=seconds)
        report = read(run / "data/ventilation.json")
        require(report["mode"] == mode and report["climate_build_ns"] > 0, "missing runtime identity/cold start")
        rows = report["samples"]
        if directory.name.startswith("round-"):
            require(report["checked_steps"] >= 4 * seconds, "missing full-room trajectory comparison")
        weather = []
        power_cpu = []
        flow_changes = []
        powered_states = []
        stage = {k: [] for k in ("prepare_ns", "step_ns", "publish_ns")}
        for prev, row in zip(rows, rows[1:]):
            if row["phase"] != "doors":
                continue
            power = row["powered_fans"]
            require(power in ((0, 32) if outage else (0,)), "unexpected connected fan count")
            if not powered_states or power != powered_states[-1]:
                if powered_states:
                    flow_changes.append(abs(row["exterior_net_abs"]-prev["exterior_net_abs"]))
                    power_cpu.append((row["climate_cpu_ns"] - prev["climate_cpu_ns"]) / 1e6)
                powered_states.append(power)
            if row["climate_tick"] == prev["climate_tick"]:
                continue
            if prev["climate_tick"] % 30 == 0:
                weather.append((row["climate_cpu_ns"] - prev["climate_cpu_ns"]) / 1e6)
                for key, vals in stage.items():
                    vals.append((row[key]-prev[key]) / 1e6)
        require(len(weather) >= seconds * 4 // 30, "incomplete weather cycle")
        if outage:
            require(len([v for v in flow_changes if v > 1e-6]) >= 9, "power toggled without material airflow response")
            require(powered_states[:3] == [32, 0, 32] and len(powered_states) >= 11,
                    f"missing simultaneous supply loss/recovery: {powered_states}")
        data.update(weather_ms=weather, weather_stages_ms={k: statistics.median(v) for k,v in stage.items()},
                    climate_cold_ms=report["climate_build_ns"]/1e6, powered_states=powered_states, power_transition_cpu_ms=power_cpu, checked_steps=report["checked_steps"],
                    rss_kib=report["rss_kib"], peak_rss_kib=report["peak_rss_kib"])
        require(0 < data["rss_kib"] <= data["peak_rss_kib"], "invalid memory counters")
        stats[c["size"]] = data
    return manifest, stats


def run_suite(root):
    require_admission(REPO, [root])
    root.mkdir(parents=True, exist_ok=False)
    tapes = root / "tapes"
    tapes.mkdir()
    for name, mode, seconds, outage in schedule():
        directory = root / name
        command = [sys.executable, "scripts/perf.py", "run", "--workload", "ventilation", "--sizes", "small,large",
                   "--renders", "gpu", "--seed", "42", "--souls", "8", "--familiars", "2", "--repeat", "1",
                   "--preflight-runs", "0", "--warmup-secs", "30", "--measure-secs", str(seconds),
                   "--instrumentation", "capture", "--adapter", "Intel", "--backend", "vulkan",
                   "--window-backend", "x11", "--present-mode", "novsync", "--window-width", "1280",
                   "--window-height", "720", "--window-scale-factor", "1", "--rtt-quality", "high",
                   "--timeout-secs", "300", "--output", str(directory)]
        env = {**os.environ, "HW_AIR_TAPE": str(tapes), "HW_AIR_COMPARE": str(int(name.startswith("round-"))), "HW_AIR_OUTAGE": str(int(outage)), "HW_VENTILATION_SOLVER": mode}
        with (root / f"{name}.log").open("w") as log:
            subprocess.run(command, cwd=REPO, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        verify_leg(directory, mode, seconds, outage)
        if mode == "record":
            hashes = tape_hashes(tapes)
            (root / "trajectory.json").write_text(json.dumps(hashes))
        elif mode == "replay":
            require(tape_hashes(tapes) == read(root / "trajectory.json"), "replay tape changed")


def tape_hashes(directory):
    files = sorted(directory.glob("air-*.bin"))
    require(len(files) == 2 and all(p.stat().st_size > 256 * 24 * 480 for p in files), "missing recording")
    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files}


def verify_suite(root):
    require(tape_hashes(root / "tapes") == read(root / "trajectory.json"), "recording changed")
    signatures = set()
    results = {}
    for name, mode, seconds, outage in schedule():
        manifest, results[name] = verify_leg(root / name, mode, seconds, outage)
        signatures.add((manifest["source"]["fingerprint_start"], manifest["binary"]["sha256"],
                        json.dumps(manifest["actual_adapters"], sort_keys=True)))
    require(len(signatures) == 1, "sessions differ in source, binary, or adapter")
    return {"identity": signatures.pop(), "runs": results}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("run", "verify"))
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    if args.action == "run":
        run_suite(args.root.resolve())
    else:
        print(json.dumps(verify_suite(args.root.resolve()), indent=2))
