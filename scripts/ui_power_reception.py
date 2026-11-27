"""Award and commission the nominated panel using actual window input."""
import native_acceptance as native
from scripts.ui_first_waterworks import catalog, open_card, wait

CASE = "first-power-reception"
CHECKPOINTS = tuple("reception-" + stage for stage in ("offer", "award", "blueprint", "built", "live", "restored"))


def check_observation(checkpoint, value):
    state = value["reception"]
    panel = state["panel"]
    stage = checkpoint.removeprefix("reception-")
    if stage == "offer":
        native.require(state["accepted"] is None and "menu:AcceptReception" in value["controls"], "missing valid tender")
    elif stage == "award":
        native.require(state["accepted"] is not None and panel is None, "award did not precede construction")
    elif stage == "blueprint":
        native.require(panel["blueprint"] and not panel["live"], "missing actual pending installation")
    elif stage == "built":
        native.require(panel["built"] and not panel["reported"] and state["received"] is None, "installation automatically reported")
    else:
        native.require(panel["built"] and panel["reported"] and panel["live"] and
                       state["received"] >= max(state["accepted"], state["scheduled"]), "invalid first reception")
        card = value["controls"].get("text:contract-card", {})
        native.require("使用 0 W" in card.get("text", "") and "初回受電" in card["text"], "missing receipt on contract")


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
    open_card(driver)
    driver.click("menu:SelectReceptionContract(true)")
    driver.click("menu:SetTimeSpeed(Normal)")
    wait(driver, lambda v: "menu:AcceptReception" in v["controls"], 45)
    driver.capture(CHECKPOINTS[0])
    driver.click("menu:AcceptReception")
    wait(driver, lambda v: v["reception"]["accepted"] is not None)
    driver.capture(CHECKPOINTS[1])
    driver.click("menu:PlaceReceivingPanel")
    wait(driver, lambda v: v["workspace_page"] == "World" and v["reception"]["screen"])
    driver.send(point=tuple(round(x * driver.last["scale_factor"]) for x in driver.last["reception"]["screen"]))
    native.require(not driver.last["pointer_over_ui"], "panel site obscured by UI")
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)
    wait(driver, lambda v: (v["reception"]["panel"] or {}).get("blueprint"))
    driver.capture(CHECKPOINTS[2])
    open_card(driver)
    driver.click("menu:SetTimeSpeed(Super)")
    wait(driver, lambda v: (v["reception"]["panel"] or {}).get("built"), 155)
    driver.capture(CHECKPOINTS[3])
    button, = [k for k in driver.last["controls"] if k.startswith("menu:ReportReceivingPanel")]
    driver.click(button)
    wait(driver, lambda v: v["reception"]["received"] is not None, 110)
    driver.click("menu:SetTimeSpeed(Paused)")
    driver.capture(CHECKPOINTS[4])
    driver.key("F5")
    catalog(driver, "SelectSaveCatalogSlot")
    wait(driver, lambda v: "Save:Succeeded" in v["save_outcomes"] and v["catalog"] == "Closed")
    epoch = driver.last["world_epoch"]
    driver.key("F9")
    catalog(driver, "SelectLoadCatalogSlot")
    catalog(driver, "ConfirmLoadCatalogSlot")
    wait(driver, lambda v: v["world_epoch"] == epoch + 1 and v["catalog"] == "Closed", 60)
    open_card(driver)
    driver.click("menu:SelectReceptionContract(true)")
    driver.capture(CHECKPOINTS[5])


def check_sequence(entries, events, client):
    for e in entries:
        check_observation(e["case"], e["value"])
    states = [e["value"]["reception"] for e in entries]
    native.require(states[2]["panel"]["id"] != states[3]["panel"]["id"] and
                   all(s["panel"]["designation"] == 1 for s in states[2:]),
                   "construction did not transfer the nominated blueprint")
    wood = states[3]["wood"]
    native.require(states[3]["receipt_error"] is None and len(wood) == 2
                   and all(r[0] == states[2]["panel"]["id"] for r in wood)
                   and len({r[1] for r in wood}) == 2 and sorted(r[2:] for r in wood) == [[0, 1], [1, 2]],
                   "missing two consumed pieces of wood for the nominated blueprint")
    native.require(all(entry["value"]["paused"] for entry in entries[-2:]) and
                   all(states[-1][k] == states[-2][k] for k in ("accepted", "scheduled", "deadline", "received", "balance"))
                   and entries[-1]["value"]["world_epoch"] == entries[-2]["value"]["world_epoch"] + 1,
                   "reloaded award or receipt changed")
    for action in ("AcceptReception", "PlaceReceivingPanel", "ReportReceivingPanel"):
        native.require(any(e.get("pressed") and e.get("button") == 1 and e["pid"] == client["pid"] and
                           any(k.startswith("menu:" + action) and v == "Pressed" for k, v in e.get("observed_hover", {}).items())
                           for e in events), "missing OS press for " + action)
