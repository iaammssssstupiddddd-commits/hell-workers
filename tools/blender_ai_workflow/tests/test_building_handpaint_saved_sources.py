"""Retained packed originals, actual byte identity and immutable readback receipts."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import building_handpaint_materials as hand


def datablocks(snapshot):
    images=[]
    for name,row in snapshot['images'].items():
        data=(hand.SOURCE_ROOT/row['path']).read_bytes()
        images.append(SimpleNamespace(name='HW_Handpaint_'+name,size=row['size'],use_fake_user=True,
                                      packed_file=SimpleNamespace(data=data,size=len(data))))
    text=SimpleNamespace(use_fake_user=True,as_string=lambda:json.dumps(snapshot,sort_keys=True))
    return images,{'HW_Handpaint_Sources.json':text}


class SavedSourceTests(unittest.TestCase):
    def test_retained_images_require_raw_packed_bytes_not_available_external_pngs(self):
        snapshot=hand.source_snapshot();images,texts=datablocks(snapshot)
        hand.check_packed_sources(images,texts,snapshot)
        for attack in ('missing-id','duplicate-id','missing-pack','changed-bytes','wrong-size','not-retained','text-change'):
            images,texts=datablocks(snapshot)
            if attack=='missing-id':images.pop()
            elif attack=='duplicate-id':images.append(deepcopy(images[0]))
            elif attack=='missing-pack':images[0].packed_file=None
            elif attack=='changed-bytes':images[0].packed_file.data=b'x'*images[0].packed_file.size
            elif attack=='wrong-size':images[0].size=[1,1]
            elif attack=='not-retained':images[0].use_fake_user=False
            else:texts['HW_Handpaint_Sources.json'].as_string=lambda:'{}'
            with self.subTest(attack=attack),self.assertRaises(ValueError):
                hand.check_packed_sources(images,texts,snapshot)

    def test_readback_is_safe_exact_and_rejects_changed_inputs_or_receipts(self):
        import subprocess
        snapshot=hand.source_snapshot()
        for attack in ('none','blend','inspector','materials','receipt','duplicate','failure'):
            with self.subTest(attack=attack),tempfile.TemporaryDirectory() as temp:
                root=Path(temp);blend=root/'source.blend';blend.write_bytes(b'frozen-blend')
                script=root/'inspector.py';script.write_bytes(b'frozen-inspector')
                result={'contract':'building-handpaint-saved-sources-v1','blend_sha256':hashlib.sha256(blend.read_bytes()).hexdigest(),'sources':snapshot}
                snapshots=[snapshot,snapshot]
                if attack=='materials':snapshots[1]={**snapshot,'manifest_sha256':'changed'}
                def child(argv,**kwargs):
                    self.assertIn('--disable-autoexec',argv)
                    self.assertIn('--background',argv)
                    self.assertEqual(kwargs['env']['BLENDER_SAFE_NO_NETWORK'],'1')
                    self.assertNotIn('--save',argv)
                    if attack=='blend':blend.write_bytes(b'changed')
                    if attack=='inspector':script.write_bytes(b'changed')
                    if attack=='receipt':result['blend_sha256']='0'*64
                    line='HW_HANDPAINT_BLEND_JSON:'+json.dumps(result)+'\n'
                    return SimpleNamespace(returncode=1 if attack=='failure' else 0,stdout=line* (2 if attack=='duplicate' else 1),stderr='')
                with patch.object(hand,'BLEND_INSPECTOR',script),patch.object(hand,'source_snapshot',side_effect=snapshots),patch.object(subprocess,'run',side_effect=child):
                    if attack=='none':self.assertEqual(hand.verify_saved_sources(blend,{'BLENDER_SAFE_NO_NETWORK':'1'}),result)
                    else:
                        with self.assertRaises(ValueError):hand.verify_saved_sources(blend,{'BLENDER_SAFE_NO_NETWORK':'1'})


if __name__=='__main__':unittest.main()
