"""Explicit natural generation inputs, not a terrain mutation or art approval."""
import json
from pathlib import Path
import tempfile
import unittest
from scripts import building_art_acceptance as art


class WorldSeedTests(unittest.TestCase):
    def value(self):
        return {'kind':'MudMixer','variant':'m2-art-context','mode':'feedback','world_seed':42}

    def test_exact_seed_is_bounded_and_context_only_with_legacy_default(self):
        self.assertEqual(art.world_seed({}),art.static.SEED)
        self.assertEqual(art.world_seed(self.value()),42)
        for seed in (True,'42',-1,2**64,None):
            value=self.value()
            value['world_seed']=seed
            with self.assertRaises(ValueError):
                art.world_seed(value)
        for key,value in (('kind','Tank'),('variant','gallery'),('mode','candidate')):
            changed=self.value()
            changed[key]=value
            with self.assertRaises(ValueError):
                art.world_seed(changed)

    def test_argv_and_child_environment_use_the_exact_declared_seed(self):
        value=self.value()
        value.update(identity={},adapter='Intel',view='normal')
        # Context setup is separately checked by its closed recipe; isolate this input boundary.
        from unittest.mock import patch
        with patch.object(art,'context_settings',return_value=True),patch.object(art,'actor_count',return_value=10):
            argv=art.game_command(Path('/binary'),Path('/job'),value)
        self.assertEqual(argv[argv.index('--perf-seed')+1],'42')
        env=art.game_environment({'HW_PERF_SEED':'999','HELL_WORKERS_WORLDGEN_SEED':'999'},Path('/view'),value,'nonce',Path('/job'))
        self.assertNotIn('HW_PERF_SEED',env)
        self.assertEqual(env['HELL_WORKERS_WORLDGEN_SEED'],'42')
        self.assertEqual(env['HW_BUILDING_ART_FEEDBACK_WORLD_SEED'],'42')
        value['world_seed']=art.static.SEED
        env=art.game_environment(env,Path('/view'),value,'nonce',Path('/job'))
        self.assertNotIn('HW_BUILDING_ART_FEEDBACK_WORLD_SEED',env)
        self.assertEqual(json.loads(env['HW_BUILDING_ART_SESSION'])['mode'],'feedback')

    def test_actual_log_and_single_summary_row_must_agree_and_remain_hash_bound(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'data').mkdir()
            for attack in ('none','missing','duplicate','foreign','summary','header'):
                log='PERF_SCENARIO: seed=42 workload=building-art-static\n'
                summary='seed,samples\n42,10\n'
                if attack=='missing':
                    log=''
                elif attack=='duplicate':
                    log+=log
                elif attack=='foreign':
                    log=log.replace('42','43')
                elif attack=='summary':
                    summary=summary.replace('42','43')
                elif attack=='header':
                    summary='seed,seed\n42,42\n'
                (root/'actual-window.log').write_text(log)
                (root/'data/summary.csv').write_text(summary)
                with self.subTest(attack=attack):
                    if attack=='none':
                        result=art.seed_evidence(root,self.value())
                        self.assertEqual(result['seed'],42)
                        self.assertEqual(result['log_sha256'],art.pipeline.digest(log.encode()))
                    else:
                        with self.assertRaises(ValueError):
                            art.seed_evidence(root,self.value())
