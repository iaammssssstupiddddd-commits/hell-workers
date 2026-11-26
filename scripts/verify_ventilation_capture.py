"""Verify the 4× ventilation layout comparison and summarize each native run."""
import argparse
import csv
import json
import math
from pathlib import Path


def require(value, message):
    if not value:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text())


def phase_stats(rows):
    frames = sorted(row["real_delta_ms"] for row in rows)
    cpu = sorted(row["cpu_ns"] / 1000 for row in rows)
    # The first delta precedes the first process clock reading.
    elapsed_ms = sum(row["real_delta_ms"] for row in rows[1:])
    process_ms = (rows[-1]["process_cpu_ns"] - rows[0]["process_cpu_ns"]) / 1e6

    def percentile(values, ratio):
        return round(values[math.floor((len(values) - 1) * ratio + .5)], 3)

    return {
        "frames": len(rows),
        "seconds": round(sum(frames) / 1000, 3),
        "process_cpu_cores": round(process_ms / elapsed_ms, 3),
        "frame_p50_ms": percentile(frames, .50),
        "frame_p95_ms": percentile(frames, .95),
        "frame_p99_ms": percentile(frames, .99),
        "max_ms": round(frames[-1], 3),
        "over_60hz": sum(ms > 1000 / 60 for ms in frames),
        "over_30hz": sum(ms > 1000 / 30 for ms in frames),
        "over_50ms": sum(ms > 50 for ms in frames),
        "air_p95_us": percentile(cpu, .95),
        "air_p99_us": percentile(cpu, .99),
    }


def verify_run(run, groups, *, dimensions=False, climate=False, seconds=60):
    validation = read(run / "validation.json")
    require(validation["valid"], f"{run}: {validation['reasons']}")
    record = read(run / "data/ventilation.json")
    wanted = {"rooms": 256, "doors": 256, "openings": 512, "groups": groups,
              "rooms_per_group": 256 // groups, "matrix_solves_at_build": groups * 2,
              "transitions": seconds * 8}
    require(all(record[k] == v for k, v in wanted.items()), f"{run}: topology or door count")
    with (run / "data/summary.csv").open() as source:
        summary, = csv.DictReader(source)
    require((int(summary["initial_souls"]), int(summary["initial_familiars"])) == (8, 2),
            f"{run}: actual population")
    with (run / "data/window.csv").open() as source:
        window, = csv.DictReader(source)
    for prefix in ("", "end_"):
        expected = {"window_present": "true", "resolved_window_backend": "x11",
                    "adapter_backend": "vulkan", "physical_width": "1280", "physical_height": "720",
                    "rtt_quality": "high", "effective_present_mode": "immediate"}
        require(all(window[prefix + k] == v for k, v in expected.items())
                and "Intel" in window[prefix + "adapter_name"], f"{run}: actual renderer")
    real, virtual = (float(summary[k]) for k in ("measure_real_secs", "measure_virtual_secs"))
    require(float(summary["virtual_time_speed"]) == 4 and real >= seconds and virtual >= 4 * seconds
            and 3.8 <= virtual / real <= 4.001, f"{run}: observed speed {virtual / real}")
    require(record["build_cpu_ns"] > 0, f"{run}: missing structural solve")
    rows = record["samples"]
    idle = [row for row in rows if row["phase"] == "idle"]
    doors = [row for row in rows if row["phase"] == "doors"]
    require(rows == idle + doors and len(idle) > 1 and len(doors) == int(summary["samples"]),
            f"{run}: incomplete phases")
    require(all(row["speed"] == 4 and row["rebuilds"] == rows[0]["rebuilds"] > 0
                and math.isfinite(row["real_delta_ms"]) and row["real_delta_ms"] > 0
                and row["cpu_ns"] >= 0 for row in rows), f"{run}: unstable measurement")
    require(all(b["calls"] == a["calls"] + 1 and a["transitions"] <= b["transitions"]
                and a["process_cpu_ns"] <= b["process_cpu_ns"] for a, b in zip(rows, rows[1:])),
            f"{run}: missing frames or non-monotone counters")
    require(idle[-1]["transitions"] == 0 and doors[-1]["transitions"] == seconds * 8,
            f"{run}: missing sustained door churn")
    baseline, active = phase_stats(idle), phase_stats(doors)
    require(baseline["seconds"] >= 9.5 and abs(active["seconds"] - real) < .002
            and active["air_p95_us"] > 0, f"{run}: incomplete timing")
    result = {"build_ms": round(record["build_cpu_ns"] / 1e6, 3),
            "observed_speed": round(virtual / real, 4),
            "transition_frames": sum(b["transitions"] > a["transitions"]
                                     for a, b in zip(rows, rows[1:])),
            "idle": baseline, "doors": active}

    if climate:
        require(all(row["climate_zones"] == 256 for row in rows), f"{run}: climate room count")
        intervals = zip(rows[len(idle)-1:-1], doors, strict=True)
        ticks = doors[-1]["climate_tick"] - idle[-1]["climate_tick"]
        require(ticks >= seconds * 4 and math.floor(virtual - 5e-7) <= ticks <= math.ceil(virtual + 5e-7),
                f"{run}: missing climate steps")
        costs = []
        for before, after in intervals:
            ns = after["climate_cpu_ns"] - before["climate_cpu_ns"]
            steps = after["climate_tick"] - before["climate_tick"]
            require(steps in (0, 1) and ns >= 0 and (not steps or ns > 0), f"{run}: invalid climate clocks")
            if ns:
                costs.append(ns / 1e6)
        costs.sort()
        result["climate"] = {"ticks": ticks, "cpu_ms_per_step": round(sum(costs) / ticks, 4),
                             "update_frame_p95_ms": round(costs[round(.95 * (len(costs)-1))], 4),
                             "update_frame_max_ms": round(costs[-1], 4)}
    if dimensions:
        from perf_tool.ventilation_dimensions import verify_dimensions
        result["dimensions"] = verify_dimensions(record)
    return result


def verify(root, *, dimensions=False, climate=False):
    manifest = read(root / "manifest.json")
    require(manifest["status"] == "valid" and manifest["source"]["unchanged"]
            and manifest["binary"]["instrumentation"] == "capture", "invalid session")
    cases = manifest["cases"]
    require(len(cases) == 2 and {c["size"] for c in cases} == {"small", "large"}, "layouts")
    result = {}
    for case in cases:
        require(case["workload"] == ("ventilation-dimensions" if dimensions else "ventilation") and case["render"] == "gpu"
                and (case["seed"], case["souls"], case["familiars"]) == (42, 8, 2),
                "unexpected scenario")
        runs = sorted((root / "cases" / case["id"]).glob("run-*"))
        require(len(runs) == 3, f"{case['size']}: expected three runs")
        result[case["size"]] = [verify_run(run, 64 if case["size"] == "small" else 1, dimensions=dimensions, climate=climate) for run in runs]
    return {"adapters": manifest["actual_adapters"], "layouts": result}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    verify_parser = parser.add_subparsers(dest="command", required=True).add_parser("verify")
    verify_parser.add_argument("session", type=Path)
    verify_parser.add_argument("--dimensions", action="store_true")
    verify_parser.add_argument("--climate", action="store_true")
    args = parser.parse_args()
    print(json.dumps(verify(args.session, dimensions=args.dimensions, climate=args.climate), indent=2))
