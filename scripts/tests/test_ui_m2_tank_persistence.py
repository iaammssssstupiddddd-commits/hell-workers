"""Synthetic persistence receipts only, never actual native/release evidence."""
import copy
import hashlib
from pathlib import Path
import tempfile
import unittest

from scripts import ui_m2_tank_persistence as persistence
from scripts import ui_m2_tank_full as full
from scripts.tests.test_ui_m2_tank_full import fixture as full_fixture


def durable_fixture():
    ctx = copy.deepcopy(full_fixture()[0]["samples"][-1]["m2_context"])
    ctx["inventory_holders"] = []
    ctx["items"][0]["stored_in"] = "child-0"
    ctx["bucket_storages"][0]["stored_items"].append("bucket-0")
    all_rows = [*ctx["tanks"],*ctx["bucket_storages"],*ctx["items"]]
    for n,row in enumerate(all_rows):
        row["entity_bits"] = 30 if row["entity"] == "30v1" else 1000+n
        pose = row.setdefault("pose", dict(translation=[float(n),0.,0.]))
        pose.setdefault("rotation",[0.,0.,0.,1.])
        pose.setdefault("scale",[1.,1.,1.])
        if "resource" in row:
            for k in ("belongs_to","stored_in","loaded_in","delivering_to","stored_by_mixer"):
                row.setdefault(k,None)
    lookup = {r["entity"]:"loaded-"+r["entity"] for r in all_rows}
    bits = {r["entity_bits"]:r["entity_bits"]+100000 for r in all_rows}
    def translate(value):
        if isinstance(value,dict):
            return {k:bits[v] if k == "entity_bits" else translate(v) for k,v in value.items()}
        if isinstance(value,list):
            return [translate(v) for v in value]
        return lookup.get(value,value) if isinstance(value,str) else value
    after = translate(ctx)
    rows = [dict(saved_entity=r["entity"],saved_bits=r["entity_bits"],loaded_entity=lookup[r["entity"]],
                 loaded_bits=bits[r["entity_bits"]],kind="Tank" if r in ctx["tanks"] else "BucketStorage" if r in ctx["bucket_storages"] else r["resource"])
            for r in all_rows]
    return ctx,after,rows


def fixture():
    raw,entries,client,inputs,contract = full_fixture()
    raw["scope"] = contract["scope"] = persistence.SCOPE
    contract.update(case=persistence.CASE,persistence_policy=persistence.POLICY)
    before,after,mapping = durable_fixture()
    # Last pre-F5 observation is still the exact continuous Full prefix.
    raw["samples"][-1]["m2_context"] = copy.deepcopy(before)
    old_last = copy.deepcopy(raw["samples"][-1])
    new_last = copy.deepcopy(old_last)
    new_last.update(world_epoch=1,m2_context=copy.deepcopy(after))
    lookup = {r["saved_entity"]:r["loaded_entity"] for r in mapping}
    new_bits = {r["saved_bits"]:r["loaded_bits"] for r in mapping}
    def translate(value):
        if isinstance(value,dict):
            return {k:new_bits.get(v,v) if k == "entity_bits" else translate(v) for k,v in value.items()}
        if isinstance(value,list):
            return [translate(v) for v in value]
        return lookup.get(value,value) if isinstance(value,str) else value
    new_last = translate(new_last)
    new_last["art_feedback"]["roots"][0]["entity"] = "new-render-root"
    for p in new_last["art_feedback"]["roots"][0]["parts"]:
        p["parent"] = "new-render-root"
    baseline = copy.deepcopy(entries[3]["value"])
    baseline["catalog"] = "Closed"
    data = b"synthetic exact save container"
    receipts = []
    keys = ("F5","menu:SelectSaveCatalogSlot { slot: Manual1, session: 7 }","F9",
            "menu:SelectLoadCatalogSlot { slot: Manual1, session: 8 }","menu:ConfirmLoadCatalogSlot { slot: Manual1, session: 8 }")
    def value(time,post=False):
        v = copy.deepcopy(baseline)
        v.update(frame=round(time*100),world_epoch=int(post),cursor=[400.,400.])
        v["m2_planning"].update(world_epoch=int(post),real_seconds=time,virtual_seconds=36.)
        if post:
            v["m2_planning"] = translate(v["m2_planning"])
            v["m2_planning"]["buildings"] = [new_bits[x] for x in baseline["m2_planning"]["buildings"]]
            v["m2_planning"]["building_inventory"] = copy.deepcopy(v["m2_planning"]["buildings"])
            for c in v["m2_planning"]["cells"]:
                c["owner"] = new_bits.get(c["owner"],c["owner"])
            v["m2_planning"]["consumers"] = copy.deepcopy(new_last["art_feedback"])
        return v
    def sample(time,post=False):
        s = copy.deepcopy(new_last if post else old_last)
        s.update(sample_index=len(raw["samples"]),real_seconds=time)
        raw["samples"].append(s)
        return s
    for n,key in enumerate(keys):
        t = (48.,50.,54.,56.,58.)[n]
        a,b = value(t),value(t+.3,n == 4)
        a["controls"][key] = dict(fully_visible=True,interaction="Hovered")
        s = sample(t+.3,n == 4)
        first = len(inputs)+1
        for offset,pressed in enumerate((True,False)):
            keyboard = key in ("F5","F9")
            sent = dict(step=str(len(inputs)+1),nonce=client["nonce"],pid=client["pid"],window=client["window"],world_epoch=int(n == 4),
                        pre_frame=a["frame"],post_frame=b["frame"],pressed=pressed)
            sent.update(dict(key=key) if keyboard else dict(button=1,observed_hover={key:"Pressed" if pressed else "Hovered"}))
            inputs.append(sent)
            event = dict(producer="KeyboardInput" if keyboard else "MouseButtonInput",operation="KeyboardInput" if keyboard else "MouseButtonInput",
                         state="Pressed" if pressed else "Released",window="99v1",sample_index=s["sample_index"],real_seconds=t+.1*(offset+1))
            event.update(dict(key=key,repeat=False) if keyboard else dict(button="Left"))
            raw["events"].append(event)
            legacy = dict(sample_index=event["sample_index"],real_seconds=event["real_seconds"],
                          operation=("Key" if keyboard else "Pointer")+("Press" if pressed else "Release"))
            if keyboard:
                legacy["key"] = key
            raw["events"].append(legacy)
        inputs[first-1]["tank_persistence_action"] = dict(key=key,first_step=first,last_step=len(inputs),before=a,after=b)
        if n in (1,4):
            post = n == 4
            receipt = dict(producer="ordinary-load-commit-v1" if post else "ordinary-save-commit-v1",operation="DurableLoad" if post else "DurableSave",
                           result="Committed",target="Manual slot 1",request_source="Load(NormalCatalog { dialog_session: 8 })" if post else "Manual(ManualCatalog { dialog_session: 7 })",
                           file_sha256=hashlib.sha256(data).hexdigest(),file_bytes=len(data),world_epoch=int(post),
                           sample_index=s["sample_index"],real_seconds=t+.3,m2_context=copy.deepcopy(after if post else before))
            if post:
                receipt.update(previous_epoch=0,m2_entity_remap=mapping)
            raw["events"].append(receipt)
            raw["events"].append(dict(producer="SaveLoadOutcome",operation="Load" if post else "Save",result="Succeeded",target="Manual slot 1",
                                      source=receipt["request_source"],sample_index=s["sample_index"],real_seconds=t+.3))
            receipts.append(receipt)
            sample(t+2,post)
            entries.append(dict(case=persistence.CHECKPOINTS[-1 if post else -2],value=value(t+2,post),last_step=len(inputs)))
    sample(61.,True)
    for n,e in enumerate(raw["events"]):
        e["event_index"] = n
    return raw,entries,client,inputs,contract,data


class TankPersistenceTests(unittest.TestCase):
    def verify(self,args):
        raw,entries,client,inputs,contract,data = args
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/"saves/manual-1.scn.ron"
            path.parent.mkdir()
            path.write_bytes(data)
            persistence.verify(raw,entries,client,inputs,contract,save_path=path)

    def test_complete_typed_bijection_preserves_all_durable_values_without_mutation(self):
        before,after,rows = durable_fixture()
        original = copy.deepcopy((before,after,rows))
        self.assertEqual(len(persistence.mapped_durable(before,after,rows)),58)
        self.assertEqual((before,after,rows),original)

    def test_missing_many_to_one_wrong_kind_bits_and_foreign_maps_rejected(self):
        for mutate in (
            lambda a,b,r:r.pop(),
            lambda a,b,r:r[1].update(loaded_entity=r[0]["loaded_entity"]),
            lambda a,b,r:r[0].update(kind="Water"),
            lambda a,b,r:r[0].update(saved_bits=True),
            lambda a,b,r:r[0].update(saved_entity="foreign"),
            lambda a,b,r:r[0].update(loaded_entity=r[0]["saved_entity"]),
        ):
            args = durable_fixture()
            mutate(*args)
            with self.assertRaises((ValueError,RuntimeError)):
                persistence.mapped_durable(*args)

    def test_item_substitution_relations_pending_and_pose_mismatch_rejected(self):
        for mutate in (
            lambda a,b,r:b["items"][0].update(resource="BucketWater"),
            lambda a,b,r:b["items"][0].update(stored_in=None),
            lambda a,b,r:b["bucket_storages"][0]["stored_items"].pop(),
            lambda a,b,r:b["bucket_storages"][0].update(pending_blueprint="foreign"),
            lambda a,b,r:b["items"][-1]["pose"]["translation"].__setitem__(0,999.),
            lambda a,b,r:b["items"][-1].update(loaded_in="foreign"),
            lambda a,b,r:b["items"][-1].update(entity="substitute"),
        ):
            args = durable_fixture()
            mutate(*args)
            with self.assertRaises((ValueError,RuntimeError)):
                persistence.mapped_durable(*args)

    def test_explicit_profile_full_prefix_and_every_persistence_tail_sample(self):
        self.verify(fixture())

    def test_old_full_entry_does_not_accept_persistence_scope(self):
        raw,entries,client,inputs,contract,_ = fixture()
        with self.assertRaises((ValueError,RuntimeError)):
            full.verify(raw,entries[:4],client,inputs[:entries[3]["last_step"]],contract)

    def test_transient_tail_epoch_inventory_bytes_or_input_corruption_rejected(self):
        for mutate in (
            lambda r,e,c,i,a,d:r["samples"][-1].update(world_epoch=0),
            lambda r,e,c,i,a,d:r["samples"][-1].update(world_epoch=True),
            lambda r,e,c,i,a,d:r["samples"][-1]["m2_context"]["tanks"][0]["stored_items"].pop(),
            lambda r,e,c,i,a,d:r["samples"][-1].update(virtual_seconds=37.),
            lambda r,e,c,i,a,d:next(x for x in r["events"] if x.get("operation")=="DurableLoad").update(file_sha256="0"*64),
            lambda r,e,c,i,a,d:next(x for x in r["events"] if x.get("operation")=="DurableLoad").update(request_source="Load(NormalCatalog { dialog_session: 9 })"),
            lambda r,e,c,i,a,d:next(x for x in r["events"] if x.get("operation")=="DurableLoad").update(real_seconds=999.),
            lambda r,e,c,i,a,d:i[-1].update(pid=c["pid"]+1),
            lambda r,e,c,i,a,d:next(x for x in r["events"] if x.get("key")=="F5").update(repeat=True),
            lambda r,e,c,i,a,d:r["samples"][-2]["art_feedback"]["roots"][0]["parts"][1].update(visible=False),
            lambda r,e,c,i,a,d:next(x for x in r["events"] if x.get("producer")=="SaveLoadOutcome").update(result="Failed"),
            lambda r,e,c,i,a,d:next(x for x in r["events"] if x.get("operation")=="DurableLoad").update(previous_epoch=True),
            lambda r,e,c,i,a,d:e[-1]["value"].update(world_epoch=True),
            lambda r,e,c,i,a,d:i[-1].update(pre_frame=True),
        ):
            args = fixture()
            mutate(*args)
            with self.subTest(mutate=mutate),self.assertRaises((ValueError,RuntimeError)):
                self.verify(args)

    def test_tail_motion_cannot_be_hidden_after_last_action(self):
        raw,entries,client,inputs,_,_ = fixture()
        inputs.append(dict(step=str(len(inputs)+1),nonce=client["nonce"],pid=client["pid"],window=client["window"],world_epoch=0,
                           pre_frame=5900,post_frame=5910,point=[400,400],observed_cursor=[400.,400.]))
        with self.assertRaisesRegex((ValueError,RuntimeError),"motion after final"):
            persistence.check_inputs(raw,entries,client,inputs)

    def test_bool_epoch_is_rejected_at_the_exact_ui_and_action_boundaries(self):
        args = fixture()
        args[1][-1]["value"]["world_epoch"] = True
        with self.assertRaisesRegex((ValueError,RuntimeError),"capture catalog differs"):
            self.verify(args)
        args = fixture()
        action = next(r["tank_persistence_action"] for r in args[3] if "tank_persistence_action" in r)
        action["before"]["world_epoch"] = False
        with self.assertRaisesRegex((ValueError,RuntimeError),"action owner/clock/epoch differs"):
            persistence.check_inputs(args[0],args[1],args[2],args[3])

    def test_extra_tail_producer_does_not_hide_in_the_f5_boundary_sample(self):
        args = fixture()
        event = next(e for e in args[0]["events"] if e.get("key")=="F5")
        foreign = dict(event,producer="ordinary-tank-take-branch-v1",operation="TankTake")
        args[0]["events"].insert(foreign["event_index"],foreign)
        for n,row in enumerate(args[0]["events"]):
            row["event_index"] = n
        with self.assertRaisesRegex((ValueError,RuntimeError),"unexpected tail producer"):
            self.verify(args)

    def test_consistent_self_owned_replacement_cannot_replace_committed_storage(self):
        args = fixture()
        save = next(e for e in args[0]["events"] if e.get("operation")=="DurableSave")
        load = next(e for e in args[0]["events"] if e.get("operation")=="DurableLoad")
        old = save["m2_context"]["bucket_storages"][0]["entity"]
        new = "replacement-storage"
        save["m2_context"]["bucket_storages"][0]["entity"] = new
        for item in save["m2_context"]["items"]:
            if item.get("stored_in") == old:
                item["stored_in"] = new
        next(r for r in load["m2_entity_remap"] if r["saved_entity"] == old)["saved_entity"] = new
        # These same-owner inventories and their remap are internally consistent,
        # but not the storages created by the ordinary placement receipt.
        persistence.mapped_durable(save["m2_context"],load["m2_context"],load["m2_entity_remap"])
        with self.assertRaisesRegex((ValueError,RuntimeError),"original committed companions replaced"):
            self.verify(args)

    def test_legacy_edges_require_exact_schema_and_unique_real_input_pair(self):
        for kind in ("missing","duplicate","key","schema","foreign"):
            args = fixture()
            events = args[0]["events"]
            edge = next(e for e in events if e.get("operation")=="KeyPress")
            if kind == "missing":
                events.remove(edge)
            elif kind == "duplicate":
                events.append(copy.deepcopy(edge))
            elif kind == "key":
                edge["key"] = "Escape"
            elif kind == "schema":
                edge["owner"] = "foreign"
            else:
                edge["operation"] = "InjectedGameplay"
            for n,e in enumerate(events):
                e["event_index"] = n
            with self.subTest(kind=kind),self.assertRaises((ValueError,RuntimeError)):
                self.verify(args)
