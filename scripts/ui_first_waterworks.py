"""Exercise the same generated world at two sites through ordinary window input."""
import native_acceptance as native

CASE = "first-waterworks"
ROUTES = ("river", "inland")
STAGES = ("offer", "site", "placement", "blueprint", "inspected", "paid")
CHECKPOINTS = tuple(f"waterworks-{route}-{stage}" for route in ROUTES for stage in STAGES) + ("waterworks-inland-restored",)


def check_observation(checkpoint, value):
    state = value["waterworks"]
    stage = checkpoint.rsplit("-", 1)[-1]
    controls = value["controls"]
    if stage == "placement" or stage == "blueprint":
        todo = controls.get("text:contract-todo", {})
        native.require(todo.get("fully_visible") and value["workspace_page"] == "World"
                       and "水1杯（5単位）" in todo["text"]
                       and todo["text"].startswith("済" if stage == "blueprint" else "□"),
                       "map ToDo missing, clipped, or not tracking the site")
    else:
        card = controls.get("text:contract-card", {})
        native.require(card.get("fully_visible"), "contract text clipped")
    if stage == "restored":
        native.require(state["stage"] == "paid" and state["reward"] in (20, 40), "paid state missing after reload")
        return
    native.require(state["stage"] == ("site" if stage == "placement" else stage), f"expected {stage}, observed {state['stage']}")
    if stage == "offer":
        native.require(state["accepted"] is None and state["water"] == 0, "expected pristine world")
    else:
        native.require(abs(state["deadline"] - state["accepted"] - 43200) < 0.01,
                       "contract deadline changed")
        native.require(state["designation"] == (1 if "-river-" in checkpoint and stage not in ("site", "placement") else 0), "unexpected target")
    if stage in ("blueprint", "inspected", "paid"):
        route = checkpoint.split("-")[1]
        native.require(state["site"] == state["sites"][ROUTES.index(route)]["grid"], "wrong actual site")
    if stage in ("inspected", "paid"):
        expected = 40 if state["inspected"] <= state["deadline"] else 20
        native.require(state["accepted"] <= state["inspected"] <= state["seconds"]
                       and state["reward"] == expected and state["water"] >= 5 and state["river_delivery"], "delivery or reward differs")
    if stage == "paid":
        native.require("menu:ClaimWaterworks" not in value["controls"] and abs(state["credited"] - state["reward"]) < 0.001, "claim balance or eligibility differs")


def wait(driver, predicate, timeout=20):
    driver.last = driver.wait(predicate, after=driver.last["frame"], timeout=timeout)


def open_card(driver):
    if "tab:Contracts" not in driver.last["controls"]:
        driver.click("menu:Workspace(OpenEntities)")
    driver.click("tab:Contracts")


def click_site(driver, index, part):
    state = driver.last
    point = state["waterworks"]["sites"][index][part]
    driver.send(point=tuple(round(x * state["scale_factor"]) for x in point))
    native.require(not driver.last["pointer_over_ui"] and not driver.last["world_input_captured"],
                   "placement point obscured")
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)


def catalog(driver, action, slot="Manual1"):
    def matching(v):
        return [k for k in v["controls"] if k.startswith("menu:" + action) and slot in k]
    wait(driver, matching)
    key, = matching(driver.last)
    driver.click(key)


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
    driver.key("F5")
    catalog(driver, "SelectSaveCatalogSlot")
    wait(driver, lambda v: v["catalog"] == "Closed" and "Save:Succeeded" in v["save_outcomes"])
    for index, route in enumerate(ROUTES):
        if index:
            epoch = driver.last["world_epoch"]
            driver.key("F9")
            catalog(driver, "SelectLoadCatalogSlot")
            catalog(driver, "ConfirmLoadCatalogSlot")
            wait(driver, lambda v: v["catalog"] == "Closed" and v["world_epoch"] == epoch + 1, 60)
        open_card(driver)
        if not index:
            driver.click("menu:ToggleContractDistrict")
            wait(driver, lambda v: v["waterworks"]["overlay"])
        driver.capture(f"waterworks-{route}-offer")
        driver.click("menu:SetTimeSpeed(Normal)")
        driver.click("menu:AcceptWaterworks")
        driver.capture(f"waterworks-{route}-site")
        if index:
            driver.click("menu:Workspace(Close)")
            driver.click("menu:ToggleArchitect")
            driver.click("menu:SelectBuild(Tank)")
        else:
            driver.click("menu:PlaceWaterworks")
        wait(driver, lambda v: "text:contract-todo" in v["controls"])
        driver.capture(f"waterworks-{route}-placement")
        click_site(driver, index, "body")
        wait(driver, lambda v: v["waterworks"]["pending"])
        click_site(driver, index, "companion")
        wait(driver, lambda v: v["waterworks"]["stage"] == "blueprint")
        driver.capture(f"waterworks-{route}-blueprint")
        if index:
            driver.key("Escape")
        open_card(driver)
        driver.click("menu:SetTimeSpeed(Super)")
        wait(driver, lambda v: v["waterworks"]["stage"] == "inspected", 155)
        driver.click("menu:SetTimeSpeed(Normal)")
        driver.capture(f"waterworks-{route}-inspected")
        driver.click("menu:ClaimWaterworks")
        driver.capture(f"waterworks-{route}-paid")

    driver.click("menu:SetTimeSpeed(Paused)")
    driver.key("F5")
    catalog(driver, "SelectSaveCatalogSlot", "Manual2")
    wait(driver, lambda v: v["catalog"] == "Closed" and "Save:Succeeded" in v["save_outcomes"])
    epoch = driver.last["world_epoch"]
    driver.key("F9")
    catalog(driver, "SelectLoadCatalogSlot", "Manual2")
    catalog(driver, "ConfirmLoadCatalogSlot", "Manual2")
    wait(driver, lambda v: v["world_epoch"] == epoch + 1 and v["catalog"] == "Closed", 60)
    open_card(driver)
    driver.capture("waterworks-inland-restored")


def check_sequence(entries, events, client):
    for entry in entries:
        check_observation(entry["case"], entry["value"])
    for index, route in enumerate(ROUTES):
        group = entries[index * len(STAGES):(index + 1) * len(STAGES)]
        values = [entry["value"] for entry in group]
        states = [v["waterworks"] for v in values]
        native.require([e["case"] for e in group] == [f"waterworks-{route}-{s}" for s in STAGES]
                       and all(v["world_epoch"] == values[0]["world_epoch"] for v in values)
                       and len({s["accepted"] for s in states[1:]}) == 1
                       and states[3]["target"] is not None and states[3]["target"] == states[-1]["completed_blueprint"]
                       and states[-1]["reward"] == states[-2]["reward"]
                       and states[-1]["inspected"] == states[-2]["inspected"],
                       "world, deadline or payment changed during route")
        keys = ("ToggleArchitect", "SelectBuild(Tank)") if index else ("PlaceWaterworks",)
        if index:
            native.require(not any(e.get("observed_hover", {}).get("menu:PlaceWaterworks") == "Pressed"
                                   and e["world_epoch"] == values[0]["world_epoch"] for e in events),
                           "ordinary construction used the contract shortcut")
        for key in ("AcceptWaterworks", *keys, "ClaimWaterworks"):
            native.require(any(e.get("pressed") and e.get("button") == 1 and
                               e.get("observed_hover", {}).get("menu:" + key) == "Pressed" and
                               e["world_epoch"] == values[0]["world_epoch"] and e["pid"] == client["pid"]
                               for e in events), "missing actual press for " + key)
    paid, restored = (entry["value"]["waterworks"] for entry in entries[-2:])
    native.require(all(paid[key] == restored[key] for key in ("accepted", "deadline", "inspected", "reward", "designation"))
                   and "menu:ClaimWaterworks" not in entries[-1]["value"]["controls"],
                   "restored paid contract differs")
    first, second = entries[0]["value"], entries[len(STAGES)]["value"]
    native.require(second["world_epoch"] == first["world_epoch"] + 1
                   and first["waterworks"]["sites"][0]["grid"] != second["waterworks"]["sites"][1]["grid"],
                   "two distinct sites across a real reload required")
