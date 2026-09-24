from __future__ import annotations

import copy
import unittest
from unittest.mock import patch

from scripts import orca_maintenance_recovery as recovery
from scripts.tests import test_orca_review_loop as fixtures

L = recovery.loop


class MaintenanceRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.f = fixtures.ReviewLoopTests()
        self.f.setUp()
        self.addCleanup(self.f.doCleanups)
        f = self.f
        target = f.root / 'integration'
        f.run_git(f.primary, 'worktree', 'add', '-qb', 'hw-42-integration', str(target))
        f.spec['integration'] = {'target': {'repo': str(target), 'branch': 'hw-42-integration', 'base': f.ticket['base']},
                                 'validation': f.spec['lanes'][0]['validation']}
        f.register()
        for _ in range(8):
            f.tick()
        with patch.object(L, 'completion', return_value={'outcome': 'completed', 'body': 'bad JSON',
                                                       'session': f.sessions['reviewer']}):
            f.tick()
        data = f.tick()
        self.assertEqual(data['phase'], 'paused')
        data['run'] = {'phase': 'ready', 'context': {'id': 'run_fixture', 'consumer_generation': 1}}
        L.save(data)
        (f.primary / 'scripts/fix.py').write_text('# guarded maintenance\n')
        f.run_git(f.primary, 'add', 'scripts/fix.py')
        f.run_git(f.primary, 'commit', '-qm', 'fix tooling')
        self.before = data
        self.spec = {'loop_sha256': recovery.B.digest(data), 'new_base': L.roles.git(f.primary, 'rev-parse', 'HEAD'),
                     'sources': {'worker-a': L.roles.fingerprint(f.repo), 'integration': L.roles.fingerprint(target)},
                     'bootstrap_closes': {}}
        self.fleet = {'scope': {'run': 'run_fixture'}, 'page': {'hasMore': False},
                      'workers': [{'dispatchId': key, 'dispatchStatus': 'completed', 'workerState': 'succeeded'}
                                  for key in data['attempts']]}
        for obj, name, value in (
            (L.dispatch, 'checked_run', data['run']['context']),
            (L.dispatch, 'run_cli', self.fleet),
            (L.dispatch.ui_coordinator, 'read_registered_state', {'phase': 'exited', 'exit_code': 0, 'terminal': data['terminal']}),
        ):
            p = patch.object(obj, name, return_value=value)
            p.start()
            self.addCleanup(p.stop)

    def recover(self):
        return recovery.recover(self.before['request_id'], self.spec, self.f.cli)

    def test_preserves_run_session_and_source_and_requires_fresh_review(self):
        result = self.recover()
        data = L.load(self.before['request_id'])
        self.assertEqual(data['run'], self.before['run'])
        lane = data['lanes']['worker-a']
        self.assertEqual(lane['session'], self.before['lanes']['worker-a']['session'])
        self.assertEqual(lane['phase'], 'review_pending')
        self.assertEqual(lane['review_ticket']['review_base'], self.spec['new_base'])
        self.assertEqual((self.f.repo / 'src/content.txt').read_text(), 'revision 0')
        self.assertTrue((self.f.repo / 'scripts/fix.py').is_file())
        self.assertNotIn('review', lane)
        receipt = recovery.S.read_private_json(recovery.Path(result['receipt']), {})
        self.assertEqual(receipt['before']['loop']['attempts'], self.before['attempts'])
        with self.assertRaisesRegex(ValueError, 'never replay'):
            self.recover()

    def test_external_source_change_is_refused_before_git_mutation(self):
        (self.f.repo / 'src/content.txt').write_text('external work')
        with self.assertRaisesRegex(ValueError, 'workspace changed'):
            self.recover()
        self.assertEqual((self.f.repo / 'src/content.txt').read_text(), 'external work')
        self.assertFalse((self.f.repo / 'scripts/fix.py').exists())

    def test_unknown_or_unreleased_attempts_are_not_recoverable(self):
        for mutate in (
            lambda fleet: fleet['workers'].append({'dispatchId': 'ctx_unknown'}),
            lambda fleet: fleet['workers'][0].update(dispatchStatus='dispatched'),
            lambda fleet: fleet['page'].update(hasMore=True),
        ):
            with self.subTest(mutate=mutate):
                fleet = copy.deepcopy(self.fleet)
                mutate(fleet)
                with self.assertRaises(ValueError):
                    recovery.require_quiescent(self.before, fleet)

    def test_non_tooling_commit_is_refused(self):
        (self.f.primary / 'src/content.txt').write_text('out of scope')
        self.f.run_git(self.f.primary, 'add', 'src/content.txt')
        self.f.run_git(self.f.primary, 'commit', '-qm', 'not tooling')
        self.spec['new_base'] = L.roles.git(self.f.primary, 'rev-parse', 'HEAD')
        with self.assertRaisesRegex(ValueError, 'tooling only'):
            self.recover()

    def test_review_request_has_fresh_exact_subject_and_no_inferred_verdict(self):
        ticket = {**self.f.ticket, 'assignment_base': self.spec['new_base'], 'generation': 2}
        review = recovery.review_ticket(ticket, 'a' * 64, {'id': 'b' * 64})
        self.assertEqual(review['source_sha256'], 'a' * 64)
        self.assertEqual(review['validation_evidence'], 'b' * 64)
        self.assertNotIn('verdict', review)
        self.assertIn('json.dumps', review['prompt'])

    def test_unarmed_exit_requires_close_and_dead_process_and_no_authority(self):
        directory = self.f.root / 'bootstrap'
        directory.mkdir()
        role = {'tasks': {}, 'last': {'key': 'unarmed', 'phase': 'starting',
                'orca_bridge': 'bridge', 'terminal': 'term_fixture'}}
        attempt = {'phase': 'unknown', 'bridge_id': 'bridge', 'terminal': 'term_fixture'}
        identity = {'pid': 999999, 'runtime': 'runtime_fixture', 'terminal': 'term_fixture',
                    'incarnation': 'incarnation_fixture', 'repo': self.f.ticket['repo']}
        journal = {'phase': 'bootstrap', 'authority': None, 'operations': {}}
        closed = {'ok': True, '_meta': {'runtimeId': 'runtime_fixture'},
                  'result': {'close': {'handle': 'term_fixture', 'ptyKilled': True}}}
        with patch.object(L.dispatch.task_bridge, 'root', return_value=self.f.root), \
             patch.object(recovery.B, 'identity', return_value='bootstrap'), \
             patch.object(recovery.S, 'read_private_json', side_effect=lambda p, _: identity if p.name == 'identity.json' else journal), \
             patch.object(recovery.os, 'kill', side_effect=ProcessLookupError):
            proof = recovery.unarmed_proof(role, attempt, self.f.ticket, closed)
            self.assertEqual(proof['terminal_close'], closed)
            with patch.object(recovery.os, 'kill', return_value=None):
                with self.assertRaisesRegex(ValueError, 'still alive'):
                    recovery.unarmed_proof(role, attempt, self.f.ticket, closed)
            for invalid in ({'ok': False}, {'ok': True, 'result': {'close': {'ptyKilled': False}}}):
                with self.assertRaises(ValueError):
                    recovery.unarmed_proof(role, attempt, self.f.ticket, invalid)
            journal['authority'] = {'dispatch_id': 'ctx_live'}
            with self.assertRaises(ValueError):
                recovery.unarmed_proof(role, attempt, self.f.ticket, closed)


if __name__ == '__main__':
    unittest.main()
