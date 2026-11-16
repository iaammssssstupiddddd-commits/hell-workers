"""Seeded Spa cannot pass when an occupied worker or its target is lost."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / '.codex/skills/hell-workers-run-native-acceptance/scripts'))
from scripts import ui_refactor_spa_slots as seeded
from scripts.tests import test_ui_spa_planning as source


def fixture(directory):
    save = Path(directory) / 'saves/manual-1.scn.ron'
    save.parent.mkdir()
    raw, entries, client, inputs = source.SpaPlanningTests().slots_fixture(save)
    for sample in raw['samples']:
        for site in sample['spa_context']['sites']:
            site['pose']['scale'] = [1., 1., 1.]
    for entry in entries:
        for site in entry['value']['spa_planning']['context']['sites']:
            site['pose']['scale'] = [1., 1., 1.]
    client['input_ack_policy'] = 'next-exact-observation-v1'
    entries = copy.deepcopy(entries[4:7])
    for entry, case in zip(entries, seeded.CHECKPOINTS, strict=True):
        entry['case'] = case
        entry['value']['spa_planning']['seeded'] = {'owner': 1, 'worker': 400, 'tile': 10}
    actions = [row['spa_action'] for row in inputs if 'spa_action' in row and int(row['step']) > entries[0]['last_step']]
    first = actions[0]
    selection = first['before']['spa_planning']['selection_target']
    entries[0]['value']['spa_planning'].update(selection_target=selection, selection_point=selection['point'])
    for i, action in enumerate(actions):
        action['before']['inspection_target'] = None if i == 0 else 1
        action['after']['inspection_target'] = 1
        if 2 <= i <= 5:
            action['before']['control']['entity'] = 55
            inputs[action['first_step']-1]['observed_hover_entities'] = {action['key']: 55}
    prefix = [{'key': 'menu:SetTimeSpeed(Normal)', 'first_step': 1, 'last_step': 1},
              {'key': 'menu:SetTimeSpeed(Paused)', 'first_step': 2, 'last_step': 2}]
    return raw, entries, client, inputs, prefix + actions


class SeededSpaTests(unittest.TestCase):
    def test_waits_until_initial_site_scale_settles(self):
        self.assertFalse(seeded.settled({'sites': [{'pose': {'scale': [1.13, 1.13, 1.13]}}]}))
        self.assertTrue(seeded.settled({'sites': [{'pose': {'scale': [1., 1., 1.]}}]}))

    def verify(self, raw, entries, client, inputs, actions):
        with patch.object(seeded.spa, 'check_action_inventory', return_value=actions):
            seeded.verify(raw, entries, client, inputs)

    def test_occupied_slot_sequence_and_non_eviction(self):
        with tempfile.TemporaryDirectory() as directory:
            self.verify(*fixture(directory))

    def test_wrong_receipt_worker_and_button_fail(self):
        for defect in ('receipt', 'worker', 'button', 'epoch', 'assignment'):
            with self.subTest(defect=defect), tempfile.TemporaryDirectory() as directory:
                raw, entries, client, inputs, actions = fixture(directory)
                if defect == 'receipt':
                    next(e for e in raw['events'] if e.get('operation') == 'SoulSpaSlotsChange')['owner'] = 'foreign'
                elif defect == 'worker':
                    raw['samples'][40]['spa_context']['souls'][0]['generate_power'] = None
                elif defect == 'button':
                    inputs[actions[4]['first_step']-1]['observed_hover_entities'][actions[4]['key']] = 999
                elif defect == 'epoch':
                    raw['samples'][40]['world_epoch'] = 10
                else:
                    raw['samples'][40]['spa_context']['tiles'][1]['workers'] = ['999v0']
                with self.assertRaises(seeded.native.AcceptanceError):
                    self.verify(raw, entries, client, inputs, actions)


if __name__ == '__main__':
    unittest.main()
