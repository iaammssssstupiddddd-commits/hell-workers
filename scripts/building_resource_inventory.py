"""Independent logical resource accounting; descriptor bytes are not GPU allocation.

Process-local asset IDs bind initial/final only. Cross-process plateau uses a
multiset of path/payload descriptions, never equality of ephemeral handle IDs.
"""
from __future__ import annotations

from collections import Counter
import json

CAP = 65_536
FORMATS = {
    "R8Unorm": 1, "Rg8Unorm": 2, "Rgba8Unorm": 4, "Rgba8UnormSrgb": 4,
    "Bgra8Unorm": 4, "Bgra8UnormSrgb": 4, "R16Float": 2, "Rg16Float": 4,
    "Rgba16Float": 8, "R32Float": 4, "Rg32Float": 8, "Rgba32Float": 16,
    "Depth16Unorm": 2, "Depth32Float": 4,
    "Rgb9e5Ufloat": 4,
}
TOTALS = {"mesh_count", "image_count", "material_count", "mesh_vertex_index_payload_bytes",
          "image_cpu_payload_bytes", "images_without_cpu_backing", "declared_texture_payload_bytes_estimate",
          "material_shallow_size", "material_shallow_bytes"}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def integer(value, *, positive=False):
    require(type(value) is int and (0 < value if positive else 0 <= value) and value <= 2**64-1,
            "resource integer invalid")
    return value


def base_entry(entry, extra):
    require(isinstance(entry, dict) and set(entry) == {"id", "path", *extra}, "resource entry keys differ")
    require(isinstance(entry["id"], str) and entry["id"] and
            (entry["path"] is None or isinstance(entry["path"], str) and entry["path"]),
            "resource id/path invalid")


def texture_estimate(entry):
    require(entry["format"] in FORMATS and entry["dimension"] in {"D1", "D2", "D3"},
            "resource texture format/dimension unsupported")
    extent = entry["extent"]
    require(isinstance(extent, list) and len(extent) == 3, "resource texture extent differs")
    w, h, d = [integer(n, positive=True) for n in extent]
    require(max(extent) <= 2**32-1, "resource texture extent overflow")
    mips, samples = integer(entry["mip_levels"], positive=True), integer(entry["samples"], positive=True)
    max_dimension = max(w, h, d if entry["dimension"] == "D3" else 1)
    require(mips <= max_dimension.bit_length() and samples in {1, 2, 4, 8, 16}
            and (entry["dimension"] != "D3" or samples == 1), "resource texture mip/sample invalid")
    result = sum(max(1, w >> n)*max(1, h >> n)*
                 (max(1, d >> n) if entry["dimension"] == "D3" else d)
                 for n in range(mips)) * FORMATS[entry["format"]] * samples
    return integer(result)


def validate(value):
    version = value.get("schema_version") if isinstance(value, dict) else None
    require(type(version) is int and version in (1, 2), "resource inventory version differs")
    require(set(value) == {"schema_version", "contract_id", *TOTALS,
            "meshes", "images", "materials", *({"mesh_uses"} if version == 2 else set())}
            and value["contract_id"] == f"building-resident-payload-inventory-v{version}",
            "resource inventory contract differs")
    for field in TOTALS:
        integer(value[field], positive=field == "material_shallow_size")
    for entries, count in ((value["meshes"], "mesh_count"), (value["images"], "image_count"),
                           (value["materials"], "material_count")):
        require(isinstance(entries, list) and len(entries) <= CAP and len(entries) == value[count],
                "resource count/cap differs")
        require(all(isinstance(e, dict) and isinstance(e.get("id"), str) for e in entries)
                and len({e["id"] for e in entries}) == len(entries), "resource ids missing/duplicated")
    for mesh in value["meshes"]:
        base_entry(mesh, {"vertex_index_payload_bytes"})
        integer(mesh["vertex_index_payload_bytes"])
    for image in value["images"]:
        base_entry(image, {"cpu_payload_bytes", "declared_texture_payload_bytes_estimate", "format",
                           "dimension", "extent", "mip_levels", "samples"})
        if image["cpu_payload_bytes"] is not None:
            integer(image["cpu_payload_bytes"])
        require(type(image["declared_texture_payload_bytes_estimate"]) is int
                and image["declared_texture_payload_bytes_estimate"] == texture_estimate(image),
                "resource texture descriptor estimate differs")
    for material in value["materials"]:
        base_entry(material, set())
    require(value["mesh_vertex_index_payload_bytes"] == sum(e["vertex_index_payload_bytes"] for e in value["meshes"])
            and value["image_cpu_payload_bytes"] == sum(e["cpu_payload_bytes"] for e in value["images"]
                                                       if e["cpu_payload_bytes"] is not None)
            and value["images_without_cpu_backing"] == sum(e["cpu_payload_bytes"] is None for e in value["images"])
            and value["declared_texture_payload_bytes_estimate"] == sum(e["declared_texture_payload_bytes_estimate"]
                                                                       for e in value["images"])
            and value["material_shallow_bytes"] == value["material_count"]*value["material_shallow_size"],
            "resource raw totals differ")
    if version == 2:
        uses = value["mesh_uses"]
        require(isinstance(uses, list) and len(uses) <= CAP, "mesh use cap exceeded")
        seen = set()
        mesh_ids = {m["id"] for m in value["meshes"]}
        for row in uses:
            require(isinstance(row, dict) and set(row) == {"entity", "dimension", "mesh", "site_yard_boundary"}
                    and row["dimension"] in ("2d", "3d") and row["mesh"] in mesh_ids
                    and type(row["site_yard_boundary"]) is bool, "mesh ownership invalid")
            integer(row["entity"])
            key = row["entity"], row["dimension"]
            require(key not in seen, "duplicate mesh owner")
            seen.add(key)
    return value


def boundaries(value):
    require(isinstance(value, dict) and set(value) == {"schema_version", "contract_id", "initial_boundary",
            "final_boundary", "gpu_allocation_measurement", "initial", "final"}
            and type(value["schema_version"]) is int and value["schema_version"] == 1
            and value["contract_id"] == "building-resource-boundaries-v1"
            and value["initial_boundary"] == "ready-before-warmup"
            and value["final_boundary"] == "post-measure-flush"
            and value["gpu_allocation_measurement"] is False, "resource boundaries missing or invalid")
    initial, final = validate(value["initial"]), validate(value["final"])
    require(initial == final, "resource handles/counts/payload drifted during measurement")
    return initial


def plateau(small, medium):
    validate(small)
    validate(medium)
    require(all(small[key] == medium[key] for key in TOTALS), "N/4N shared resource totals differ")
    for key in ("meshes", "images", "materials"):
        def descriptions(entries):
            return Counter(json.dumps({k: v for k, v in entry.items() if k != "id"}, sort_keys=True,
                                      separators=(",", ":")) for entry in entries)
        require(descriptions(small[key]) == descriptions(medium[key]),
                "N/4N resource payload/path multiset differs")
    return {key: small[key] for key in sorted(TOTALS)}


def legacy_boundary_growth(small, medium):
    """Observe, never approve, the historical Site/Yard quad duplication.

    Only the old-reference supplement/paired leg may use this exception. Current
    sources must still pass the unmodified global plateau above. Total resident
    counts and bytes remain in raw results and budget comparisons.
    """
    def partition(value, expected):
        validate(value)
        require(value["schema_version"] == 2, "baseline ownership evidence missing")
        uses = value["mesh_uses"]
        boundaries = [r for r in uses if r["site_yard_boundary"]]
        require(len(boundaries) == expected and all(r["dimension"] == "2d" for r in boundaries),
                "baseline Site/Yard fixture boundary count differs")
        ids = {r["mesh"] for r in boundaries}
        require(len(ids) == expected, "historical per-boundary mesh identity differs")
        require(not any(r["mesh"] in ids and not r["site_yard_boundary"] for r in uses),
                "baseline boundary mesh has a foreign consumer")
        meshes = [m for m in value["meshes"] if m["id"] in ids]
        require(len(meshes) == expected and all(m["path"] is None and m["vertex_index_payload_bytes"] == 152
                                              for m in meshes), "baseline boundary quad payload differs")
        result = {k: v for k, v in value.items() if k != "mesh_uses"}
        result.update(schema_version=1, contract_id="building-resident-payload-inventory-v1",
                      meshes=[m for m in value["meshes"] if m["id"] not in ids],
                      mesh_count=value["mesh_count"]-expected,
                      mesh_vertex_index_payload_bytes=value["mesh_vertex_index_payload_bytes"]-152*expected)
        return result
    # Also usable between repeated same-size observations, with exact 7/25
    # fixture identities checked by the session verifier before this function.
    counts = [sum(r["site_yard_boundary"] for r in v.get("mesh_uses", [])) for v in (small, medium)]
    require(all(n in (7, 25) for n in counts), "unknown baseline boundary scale")
    shared = plateau(partition(small, counts[0]), partition(medium, counts[1]))
    return {"global_shared_mesh_plateau": False, "known_failure": "per-boundary duplicated Site/Yard quad",
            "boundary_instances": counts, "boundary_mesh_bytes": [n*152 for n in counts],
            "remaining_resource_plateau": shared, "accepted": False}


def require_boundary_scale(value, size):
    validate(value)
    require(size in ("small", "medium") and value["schema_version"] == 2,
            "boundary scale ownership unavailable")
    expected = {"small": 7, "medium": 25}[size]
    require(sum(row["site_yard_boundary"] for row in value["mesh_uses"]) == expected,
            "boundary ownership differs from verified fixture size")
