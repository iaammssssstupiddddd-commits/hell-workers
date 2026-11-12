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
    require(isinstance(value, dict) and set(value) == {"schema_version", "contract_id", *TOTALS,
            "meshes", "images", "materials"} and type(value["schema_version"]) is int
            and value["schema_version"] == 1 and value["contract_id"] == "building-resident-payload-inventory-v1",
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
