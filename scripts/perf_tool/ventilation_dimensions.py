"""Verify the six interleaved native transport conditions against a separate zone balance."""
import math
from statistics import mean

MODES = {
    "airflow": (),
    "temperature": (0,),
    "humidity": (1,),
    "pollution": (2,),
    "humidity_pollution": (1, 2),
    "all": (0, 1, 2),
}


def require(value, detail):
    if not value:
        raise ValueError(f"invalid ventilation dimensions capture: {detail}")


def reference(links, field, count=256, steps=40):
    """Solve each implicit step iteratively, checking the total mass/heat budget."""
    initial = (20., 7., .10)[field]
    outdoor = (12., 4., .02)[field]
    wall = .12 if field == 0 else 0.
    capacity = 6. if field == 0 else 4.
    source = [(0.18, 0.04, 0.015)[field] * (1 + (i % 7) / 7) for i in range(count)]
    neighbors = [[] for _ in range(count)]
    exterior_in = [0.] * count
    exterior_out = [0.] * count
    diagonal = [capacity] * count
    for link in links:
        a, b, forward, reverse = (link[k] for k in ("a", "b", "forward", "reverse"))
        require(0 <= a < count and (b is None or 0 <= b < count and a != b)
                and all(math.isfinite(q) and q >= 0 for q in (forward, reverse)), "flow endpoints")
        diagonal[a] += forward
        if b is None:
            exterior_in[a] += reverse
            exterior_out[a] += forward
        else:
            neighbors[a].append((b, reverse + wall))
            neighbors[b].append((a, forward + wall))
            diagonal[b] += reverse
    for i, connected in enumerate(neighbors):
        require(len(connected) <= 4, "uniform square room faces")
        walls_out = (4 - len(connected)) * wall
        exterior_in[i] += walls_out
        exterior_out[i] += walls_out
        diagonal[i] += 4 * wall
    values = [initial] * count
    for _ in range(steps):
        previous = values.copy()
        rhs = [capacity * v + s + q * outdoor for v, s, q in zip(previous, source, exterior_in)]
        for _ in range(100):
            change = 0.
            for i, row in enumerate(neighbors):
                updated = (rhs[i] + sum(q * values[j] for j, q in row)) / diagonal[i]
                change = max(change, abs(updated - values[i]))
                values[i] = updated
            if change < 1e-12:
                break
        else:
            raise ValueError("independent implicit transport failed to converge")
        change_total = capacity * (sum(values) - sum(previous))
        boundary_total = sum(source) + outdoor * sum(exterior_in) - sum(
            rate * v for rate, v in zip(exterior_out, values)
        )
        require(abs(change_total - boundary_total) < 1e-8, "conservation")
    return values


def distribution(values):
    ordered = sorted(values)
    def p(ratio):
        return ordered[math.floor((len(ordered) - 1) * ratio + .5)]
    return {"p50": p(.5), "p95": p(.95), "p99": p(.99), "max": ordered[-1]}


def verify_dimensions(record):
    experiment = record["transport"]
    require(experiment["ticks"] == 240 and experiment["modes"] == list(MODES)
            and len(experiment["links"]) == 512 and len(experiment["final_values"]) == len(MODES)
            and all(x > 0 for x in experiment["factor_links"]) and experiment["setup_ns"] > 0,
            "geometry or step timing")
    adjacency = [set() for _ in range(256)]
    exterior = 0
    for link in experiment["links"]:
        a, b = link["a"], link["b"]
        require(0 <= a < 256, "invalid zone")
        if b is None:
            exterior += 1
        else:
            require(0 <= b < 256 and b != a and b not in adjacency[a], "duplicate shared face")
            adjacency[a].add(b)
            adjacency[b].add(a)
    require(exterior == {64: 256, 1: 32}[record["groups"]], "outdoor grille count")
    unseen = set(range(256))
    sizes = []
    while unseen:
        stack = [unseen.pop()]
        size = 0
        while stack:
            zone = stack.pop()
            size += 1
            new = adjacency[zone] & unseen
            unseen.difference_update(new)
            stack.extend(new)
        sizes.append(size)
    require(sizes == [record["rooms_per_group"]] * record["groups"], "thermal room graph")
    references = [reference(experiment["links"], f) for f in range(3)]
    for field_ids, final in zip(MODES.values(), experiment["final_values"], strict=True):
        require(len(final) == len(field_ids), "unexpected field")
        for field, actual in zip(field_ids, final):
            require(len(actual) == 256 and all(math.isfinite(v) and v >= 0
                    and abs(v - ref) < 1e-8 for v, ref in zip(actual, references[field])),
                    "final per-room transport differs from the independent solve")
    rows = [r for r in record["samples"] if r["phase"] == "doors"]
    frames = {k: [] for k in MODES}
    process_ns = dict.fromkeys(MODES, 0)
    costs = {k: [] for k in MODES}
    total = 0
    for index, row in enumerate(rows):
        observation = row["transport"]
        block = min(total, 239) // 8
        mode = list(MODES)[(block + block // len(MODES)) % len(MODES)]
        ticks = observation["ticks"]
        require(observation["mode"] == mode and ticks in (0, 1), "mode sequence")
        if ticks:
            require(observation["cpu_ns"] > 0, "unmeasured integration")
            costs[mode].append(observation["cpu_ns"] / 1000.)
            total += ticks
        else:
            require(observation["cpu_ns"] == 0, "extra integration")
        # Time<Real>.delta is the preceding frame. Attribute the next interval to this work.
        if index + 1 < len(rows):
            following = rows[index + 1]
            frames[mode].append(following["real_delta_ms"])
            process_ns[mode] += following["process_cpu_ns"] - row["process_cpu_ns"]
    require(total == 240, "complete integration")
    result = {}
    for mode in MODES:
        elapsed_ms = sum(frames[mode])
        require(len(costs[mode]) == 40 and 9500 < elapsed_ms < 10500, "unbalanced exposure")
        result[mode] = {
            "ticks": len(costs[mode]), "tick_us": distribution(costs[mode]),
            "mean_tick_us": mean(costs[mode]), "frame_ms": distribution(frames[mode]),
            "frames": len(frames[mode]), "over_33ms": sum(x > 1000/30 for x in frames[mode]),
            "over_50ms": sum(x > 50 for x in frames[mode]), "seconds": elapsed_ms / 1000.,
            "process_cores": process_ns[mode] / 1e6 / elapsed_ms,
        }
    return {"setup_ms": experiment["setup_ns"] / 1e6,
            "factor_links": experiment["factor_links"], "modes": result}
