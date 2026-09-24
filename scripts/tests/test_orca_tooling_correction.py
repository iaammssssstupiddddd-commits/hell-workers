from __future__ import annotations

import copy
import json
import unittest
from unittest.mock import patch

from scripts import orca_tooling_correction as correction
from scripts.tests import test_orca_git_integrate as fixtures
from scripts.tests.test_orca_dispatch import REQUEST, COORDINATOR

L = correction.loop


class ToolingCorrectionTests(unittest.TestCase):
    def setUp(self):
        self.i = fixtures.IntegrationTests()
        self.i.setUp()
        self.addCleanup(self.i.doCleanups)
        self.i.enable()
        f = self.i.fixture
        f.register()
        original = f.completed
        documentation = self._testMethodName == 'test_review_cited_documentation_correction_reenters_validation'

        def rejected(ticket, slot, attempt):
            result = original(ticket, slot, attempt)
            if ticket['id'].startswith('integration-'):
                record = L.parse_review(result['body'], result['session'])
                record.pop('reviewer_session')
                finding = ({'id': 'D1', 'message': 'docs/contract.md is stale',
                            'acceptance': 'Update docs/contract.md to match the implementation'}
                           if documentation else
                           {'id': 'T1', 'message': 'Tooling contract differs', 'acceptance': 'Contract matches'})
                record.update(verdict='changes_requested', blocking_findings=[finding])
                result['body'] = 'Summary. Review. Complete.\nORCA_REVIEW_JSON: ' + json.dumps(record)
            return result

        with patch.object(L, 'completion', side_effect=rejected):
            for _ in range(60):
                data = f.tick()
                if data['integration']['phase'] == 'correction_required':
                    break
            else:
                self.fail('fixture did not reject integration')
        # This unit adapter mocks mailbox delivery; represent its completed ACKs.
        for item in data['attempts'].values():
            item['completion_acknowledged'] = True
        L.save(data)
        self.before = copy.deepcopy(data)
        correction_repo = f.primary
        if documentation:
            correction_repo = f.root / 'documentation-correction'
            f.run_git(f.primary, 'worktree', 'add', '-qb', 'documentation-correction',
                      str(correction_repo), data['integration']['receipt']['head'])
        correction_path = correction_repo / ('docs/contract.md' if documentation else 'scripts/contract.py')
        correction_path.parent.mkdir(exist_ok=True)
        correction_path.write_text('corrected contract\n')
        f.run_git(correction_repo, 'add', str(correction_path.relative_to(correction_repo)))
        f.run_git(correction_repo, 'commit', '-qm', 'fix coordinator-owned contract')
        self.spec = {'loop_sha256': L.inspection_digest(data),
                     'commit': L.roles.git(correction_repo, 'rev-parse', 'HEAD'),
                     'reason': 'Correct the host-only test contract without extending worker scope',
                     'validation': data['integration']['validation']}
        if documentation:
            self.spec['category'] = 'documentation'

    def test_new_head_requires_fresh_validation_review_preserves_run_and_attempts(self):
        result = correction.correct(REQUEST, COORDINATOR, self.spec)
        data = L.load(REQUEST)
        self.assertEqual(data['attempts'], self.before['attempts'])
        self.assertEqual(data['run'], self.before['run'])
        self.assertEqual(data['lanes'], self.before['lanes'])
        self.assertNotIn('review', data['integration'])
        self.assertEqual(data['integration']['phase'], 'validating')
        self.assertNotEqual(result['head'], self.before['integration']['receipt']['head'])
        self.assertEqual(data['integration']['receipt']['previous'], self.before['integration']['receipt'])
        self.assertEqual((self.i.repo / 'src/content.txt').read_text(), 'revision 1')
        with self.assertRaisesRegex(ValueError, 'without replay'):
            correction.correct(REQUEST, COORDINATOR, self.spec)
        completed = self.i.fixture.finish()
        self.assertEqual(completed['phase'], 'approved', completed.get('reason'))
        self.assertEqual(completed['integration']['review_ticket']['generation'], 2)

    def test_stale_unacknowledged_or_dirty_subject_is_refused(self):
        with self.assertRaisesRegex(ValueError, 'exact settled'):
            correction.correct(REQUEST, COORDINATOR, {**self.spec, 'loop_sha256': '0' * 64})
        data = copy.deepcopy(self.before)
        next(iter(data['attempts'].values()))['completion_acknowledged'] = False
        L.save(data)
        with self.assertRaisesRegex(ValueError, 'exact settled'):
            correction.correct(REQUEST, COORDINATOR, {**self.spec, 'loop_sha256': L.inspection_digest(data)})
        L.save(self.before)
        (self.i.repo / 'src/content.txt').write_text('external change')
        with self.assertRaises(ValueError):
            correction.correct(REQUEST, COORDINATOR, self.spec)
        self.assertEqual((self.i.repo / 'src/content.txt').read_text(), 'external change')

    def test_game_changes_are_not_tooling_corrections(self):
        f = self.i.fixture
        (f.primary / 'src/content.txt').write_text('not tooling')
        f.run_git(f.primary, 'add', 'src/content.txt')
        f.run_git(f.primary, 'commit', '-qm', 'outside scope')
        self.spec['commit'] = L.roles.git(f.primary, 'rev-parse', 'HEAD')
        with self.assertRaisesRegex(ValueError, 'only host tooling'):
            correction.correct(REQUEST, COORDINATOR, self.spec)
        self.assertEqual(L.roles.git(self.i.repo, 'rev-parse', 'HEAD'), self.before['integration']['receipt']['head'])

    def test_review_cited_documentation_correction_reenters_validation(self):
        result = correction.correct(REQUEST, COORDINATOR, self.spec)
        data = L.load(REQUEST)
        self.assertEqual(data['integration']['phase'], 'validating')
        self.assertNotEqual(result['head'], self.before['integration']['receipt']['head'])
        completed = self.i.fixture.finish()
        self.assertEqual(completed['phase'], 'approved', completed.get('reason'))

    def test_interrupted_correction_is_not_replayed_or_approved(self):
        with patch.object(L.integration, 'finish', side_effect=OSError('interrupted checkout')):
            with self.assertRaises(OSError):
                correction.correct(REQUEST, COORDINATOR, self.spec)
        self.assertEqual(L.load(REQUEST), self.before)
        with self.assertRaisesRegex(ValueError, 'without replay'):
            correction.correct(REQUEST, COORDINATOR, self.spec)
