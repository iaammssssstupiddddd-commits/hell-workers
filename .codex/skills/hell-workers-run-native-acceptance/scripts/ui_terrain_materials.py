"""Visible terrain material/prepass smoke and exact Scene resize observations."""
import hashlib
import math

import native_acceptance as native

CHECKPOINTS = ("terrain-initial", "terrain-resized", "terrain-restored")
SIZES = {"terrain-initial": [1920, 1080], "terrain-resized": [1280, 720], "terrain-restored": [1920, 1080]}


def sample_srgb(rgb, width, height, x, y, channel):
    x, y = min(width-1, max(0, x)), min(height-1, max(0, y))
    left, top = math.floor(x), math.floor(y)
    linear = 0.0
    for px, wx in ((left, 1-(x-left)), (min(left+1, width-1), x-left)):
        for py, wy in ((top, 1-(y-top)), (min(top+1, height-1), y-top)):
            value = rgb[(py*width+px)*3+channel]/255
            linear += wx*wy*(value/12.92 if value <= 0.04045 else ((value+0.055)/1.055)**2.4)
    return 255*(12.92*linear if linear <= 0.0031308 else 1.055*linear**(1/2.4)-0.055)


def check_observation(case, value):
    evidence = value["terrain_materials"]
    native.require(value["viewport"] == SIZES[case], "terrain client dimensions differ")
    native.require(value["paused"] and value["dpi_mode"] == "native"
                   and value["scale_factor"] == value["base_scale_factor"] > 0, "terrain needs actual DPI and paused simulation")
    native.require(evidence["camera_bound"] and evidence["composite_bound"], "Scene output binding differs")
    scale = evidence["quality_scale"]
    native.require(math.isfinite(scale) and scale > 0
                   and evidence["size"] == [max(1, math.floor(side * scale + 0.5)) for side in value["viewport"]]
                   and math.isclose(evidence["target_scale"], value["scale_factor"] * scale, rel_tol=1e-6)
                   and math.isclose(evidence["camera_scale"], evidence["target_scale"], rel_tol=1e-6),
                   "Scene dimensions or DPI scale differ")
    gpu = evidence["gpu"]
    native.require(gpu["scene"] == evidence["scene"] and gpu["size"] == evidence["size"], "stale GPU Scene image")
    native.require(gpu["resident"] == [True] * 4 and not gpu["errors"], "terrain main/prepass pipeline is unavailable")
    native.require("Intel" in gpu["adapter"] and gpu["backend"] == "Vulkan", "terrain adapter/backend differs")
    patches = evidence["patches"]
    native.require([patch["lod"] for patch in patches] == [0, 1, 2], "terrain LOD coverage differs")
    last_right = -1
    for patch in patches:
        native.require(patch["visible"], "terrain patch hidden")
        for rect, bounds in ((patch["scene_rect"], evidence["size"]), (patch["client_rect"], value["viewport"])):
            native.require(all(math.isfinite(number) for number in rect)
                           and 0 <= rect[0] < rect[2] <= bounds[0] and 0 <= rect[1] < rect[3] <= bounds[1], "terrain patch outside target")
        native.require(patch["scene_rect"][0] > last_right, "terrain patches overlap")
        last_right = patch["scene_rect"][2]
    readback = evidence["readback"]
    native.require(readback["scene"] == evidence["scene"] and readback["size"] == evidence["size"], "stale Scene readback")
    native.require(readback["opaque_pixels"] == [64, 64, 64], "terrain patch rendered clear pixels")


def ready(value):
    evidence = value.get("terrain_materials")
    return bool(evidence and evidence.get("readback")
                and evidence["readback"]["scene"] == evidence["scene"]
                and evidence["gpu"].get("scene") == evidence["scene"])


def add_capture(directory, item):
    filename = item["value"]["terrain_materials"]["readback"]["file"]
    path = directory / filename
    native.require(path.parent == directory and path.is_file(), "terrain readback file missing")
    item["scene_screenshot"] = filename
    item["scene_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()


def verify_sequence(directory, entries, events):
    native.require([item["case"] for item in entries] == list(CHECKPOINTS), "terrain checkpoints differ")
    native.require([event.get("resize") for event in events] == [[1280, 720], [1920, 1080]], "terrain resize sequence differs")
    scenes = []
    for index, item in enumerate(entries):
        value = item["value"]
        check_observation(item["case"], value)
        evidence = value["terrain_materials"]
        scenes.append(evidence["scene"])
        native.require(item["last_step"] == index, "terrain checkpoint lacks resize acknowledgement")
        if index:
            previous = entries[index-1]["value"]
            native.require(value["frame"] > previous["frame"] and evidence["gpu"]["frame"] > previous["terrain_materials"]["gpu"]["frame"], "terrain reuses an old frame")
        path = directory / item["scene_screenshot"]
        native.require(path.parent == directory and path.name == evidence["readback"]["file"]
                       and hashlib.sha256(path.read_bytes()).hexdigest() == item["scene_sha256"], "terrain readback changed")
        width, height, rgb = native.decode_png_rgb(path.read_bytes())
        native.require([width, height] == evidence["size"], "terrain image dimensions differ")
        client_width, client_height, client_rgb = native.decode_png_rgb((directory / item["screenshot"]).read_bytes())
        native.require([client_width, client_height] == value["viewport"], "terrain client image dimensions differ")
        for patch in evidence["patches"]:
            x0, y0, x1, y1 = patch["scene_rect"]
            x, y = int((x0+x1)/2), int((y0+y1)/2)
            native.require(x+8 <= width and y+8 <= height, "terrain sample outside image")
            samples = [max(rgb[((y+dy)*width+x+dx)*3:((y+dy)*width+x+dx)*3+3]) for dy in range(8) for dx in range(8)]
            native.require(min(samples) > 2, "terrain readback contains only clear background")
            left, top, right, bottom = patch["client_rect"]
            cx, cy = int((left+right)/2), int((top+bottom)/2)
            client_samples = [max(client_rgb[((cy+dy)*client_width+cx+dx)*3:((cy+dy)*client_width+cx+dx)*3+3])
                              for dy in range(8) for dx in range(8)]
            native.require(min(client_samples) > 2, "terrain composite is missing from the actual client")
            expected, actual = [0.0]*3, [0.0]*3
            for dy in range(8):
                for dx in range(8):
                    sx = (cx+dx+0.5)/client_width*width-0.5
                    sy = (((cy+dy+0.5)/client_height-0.5)/evidence["vertical_compensation"]+0.5)*height-0.5
                    for channel in range(3):
                        expected[channel] += sample_srgb(rgb, width, height, sx, sy, channel)/64
                        actual[channel] += client_rgb[((cy+dy)*client_width+cx+dx)*3+channel]/64
            native.require(max(abs(a-b) for a, b in zip(expected, actual)) <= 8,
                           "terrain client colors differ from the current Scene output")
    native.require(len(set(scenes)) == 3, "resize did not recreate Scene target")


def exercise(driver):
    driver.last = driver.wait(ready, after=driver.last["frame"], timeout=60)
    driver.capture("terrain-initial")
    for case in CHECKPOINTS[1:]:
        size = SIZES[case]
        driver.input.resize(str(len(driver.events)+1), driver.nonce, *size)
        driver.last = driver.wait(lambda value: value["viewport"] == size and ready(value), after=driver.last["frame"], timeout=60)
        native.require(list(driver.input.client_size()) == size, "window manager rejected resize")
        driver.events[-1]["post_frame"] = driver.last["frame"]
        native.atomic_write_json(driver.root / "events.json", {"events": driver.events})
        driver.capture(case)
