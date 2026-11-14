"""Rejection tests for released adjacent comparison, applied with its implementation."""
import copy
from contextlib import ExitStack
from dataclasses import replace
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_cumulative_acceptance as cumulative
from scripts import building_foundation_paired_acceptance as paired
from scripts import building_m6_acceptance as m6


class ReleasedCodecBoundary(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.repo = Path(temporary.name)
        self.codec = self.repo / 'codec'
        self.codec.write_bytes(b'fixed trusted codec')
        self.sha = m6.pipeline.digest(self.codec.read_bytes())
        self.identity_value = {'subject_commit': 'a' * 40, 'source_fingerprint': 'b' * 64}
        for name, mock in [('git_dirty_paths', patch.object(m6.native, 'git_dirty_paths', return_value=[])),
                           ('identity_mock', patch.object(m6.static, 'identity', return_value=self.identity_value))]:
            setattr(self, name, mock.start())
            self.addCleanup(mock.stop)

    def context(self, **kwargs):
        return m6.released_case_context(self.repo, self.codec, 'Intel', **kwargs)

    def test_first_admission_and_pinned_read_are_distinct(self):
        with patch.object(m6, 'inventory', return_value={'bindings': [], 'manifests': []}):
            first = self.context()
            self.assertEqual(first.codec_sha256, self.sha)
            self.assertEqual(self.context(expected_codec_sha256=self.sha), first)

    def test_invalid_saved_sha_never_executes_inventory(self):
        for value in (None, True, False, 0, '', 'a' * 63, 'a' * 65, 'A' * 64, 'g' * 64, ['a' * 64]):
            with self.subTest(value=value), patch.object(m6, 'inventory') as inventory:
                with self.assertRaises(ValueError):
                    self.context(expected_codec_sha256=value)
                inventory.assert_not_called()

    def test_changed_codec_never_executes_inventory(self):
        self.codec.write_bytes(b'foreign executable')
        with patch.object(m6, 'inventory') as inventory:
            with self.assertRaisesRegex(ValueError, 'before execution'):
                self.context(expected_codec_sha256=self.sha)
            inventory.assert_not_called()

    def test_change_during_inventory_is_rejected(self):
        def inventory(*_):
            self.codec.write_bytes(b'changed during trusted call')
            return {}
        with patch.object(m6, 'inventory', side_effect=inventory):
            with self.assertRaisesRegex(ValueError, 'changed while reading'):
                self.context(expected_codec_sha256=self.sha)

    def test_source_change_during_inventory_is_rejected(self):
        self.identity_mock.side_effect = [self.identity_mock.return_value, {'subject_commit': 'c' * 40}]
        with patch.object(m6, 'inventory', return_value={}):
            with self.assertRaisesRegex(ValueError, 'changed while reading'):
                self.context(expected_codec_sha256=self.sha)

    def test_null_context_pin_is_rejected_before_inventory_or_oracle(self):
        with patch.object(m6, 'inventory', return_value={}):
            context = self.context()
        context = replace(context, codec_sha256=None)
        with patch.object(m6, 'inventory') as inventory, patch.object(m6, '_verify_case_artifacts') as oracle:
            with self.assertRaises(ValueError):
                m6.verify_released_case(context, ('capture', 'small', 1, 'candidate'), self.repo, 'd' * 64)
            inventory.assert_not_called()
            oracle.assert_not_called()

    def test_foreign_context_cannot_reach_raw_oracle(self):
        with patch.object(m6, 'inventory', return_value={'bindings': []}):
            context = self.context()
        for field, value in [('identity', {'subject_commit': 'e' * 40}), ('assets', {'bindings': ['foreign']})]:
            with self.subTest(field=field), patch.object(m6, 'inventory', return_value={'bindings': []}), \
                    patch.object(m6, '_verify_case_artifacts') as oracle:
                with self.assertRaisesRegex(ValueError, 'context changed or forged'):
                    m6.verify_released_case(replace(context, **{field: value}),
                                           ('capture', 'small', 1, 'candidate'), self.repo, 'd' * 64)
                oracle.assert_not_called()


class FrozenCumulativeInputs(unittest.TestCase):
    def test_historical_verifier_is_explicit_and_only_the_cumulative_profile_uses_it(self):
        planned = {'profile': paired.PROFILE, 'accepted': False, 'promotion_authority': False,
                   'drivers': {}, 'matrix': [list(case) for case in paired.matrix()],
                   'limits': {'max_delta': dict.fromkeys(paired.METRICS, 0),
                              'max_relative_mad': .1, 'max_gap_seconds': 180, 'reason': 'premeasurement fixture'},
                   'repos': {'current': '/current', 'reference': '/reference'},
                   'job_root': '/current/target/comparison', 'identities': {'reference': {'old': True}},
                   'resource_basis': '/basis', 'resource_basis_sha256': 'fixed'}
        with ExitStack() as stack:
            for obj, name, kwargs in (
                (paired, 'drivers', {'return_value': {}}),
                (paired.native, 'validate_repo', {'side_effect': Path}),
                (paired.static, 'check_root', {}),
                (paired, 'identities', {'return_value': planned['identities']}),
                (paired.static, 'sha256', {'return_value': 'fixed'}),
                (cumulative, 'check_inputs', {}),
            ):
                stack.enter_context(patch.object(obj, name, **kwargs))
            direct = stack.enter_context(patch.object(paired.supplement, 'verify', return_value={'source': {'old': True}}))
            frozen = stack.enter_context(patch.object(cumulative, 'verify_resource_basis', return_value={'source': {'old': True}}))
            paired.check_plan(planned)
            direct.assert_called_once_with(Path('/basis'))
            frozen.assert_not_called()
            direct.reset_mock()
            planned.update(profile=paired.CUMULATIVE_PROFILE, release={})
            with self.assertRaisesRegex(paired.native.AcceptanceError, 'historical resource verifier/profile'):
                paired.check_plan(planned)
            direct.assert_not_called()
            frozen.assert_not_called()
            planned['resource_basis_verifier'] = {'fixture': 'explicit binding'}
            paired.check_plan(planned)
            frozen.assert_called_once_with(planned)
            direct.assert_not_called()
            planned.pop('release')
            planned['profile'] = paired.PROFILE
            with self.assertRaisesRegex(paired.native.AcceptanceError, 'historical resource verifier/profile'):
                paired.check_plan(planned)

    def test_missing_historical_source_fails_before_binding_or_cli_plan(self):
        args = type('Args', (), {'resource_verifier_repo': None})()
        with patch.object(cumulative.historical, 'bind') as bind:
            with self.assertRaisesRegex(ValueError, 'frozen resource verifier source'):
                cumulative.bind_resource_verifier(args, {})
            bind.assert_not_called()
        argv = ['cumulative', 'plan', '--repo', '/current', '--reference-repo', '/reference',
                '--budget', '/budget', '--resource-basis', '/basis', '--job-root', '/root',
                '--output', '/plan', '--codec', '/codec']
        with patch('sys.argv', argv), patch.object(paired, 'plan') as plan:
            with self.assertRaisesRegex(paired.native.AcceptanceError, 'frozen resource verifier source'):
                paired.main(profile=paired.CUMULATIVE_PROFILE)
            plan.assert_not_called()

    def test_saved_context_missing_sha_is_not_initial_admission(self):
        planned = {'repos': {'current': '/current', 'reference': '/reference'}, 'adapter': 'Intel',
                   'release': {'context': {'codec': '/codec'}}}
        args = type('Args', (), {'codec': Path('/codec')})()
        with patch.object(m6, 'released_case_context') as factory:
            with self.assertRaises(KeyError):
                cumulative.plan_inputs(args, planned)
            factory.assert_not_called()

    def test_saved_null_pin_is_forwarded_explicitly_not_omitted(self):
        planned = {'repos': {'current': '/current', 'reference': '/reference'}, 'adapter': 'Intel',
                   'release': {'context': {'codec': '/codec', 'codec_sha256': None}}}
        args = type('Args', (), {'codec': Path('/codec')})()
        with patch.object(m6, 'released_case_context', side_effect=ValueError('strict pin')) as factory:
            with self.assertRaisesRegex(ValueError, 'strict pin'):
                cumulative.plan_inputs(args, planned)
            self.assertIn('expected_codec_sha256', factory.call_args.kwargs)
            self.assertIsNone(factory.call_args.kwargs['expected_codec_sha256'])

    def test_codec_change_prevents_candidate_inverse_projection(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'codec'
            path.write_bytes(b'foreign')
            context = type('Context', (), {'codec': str(path), 'codec_sha256': '0' * 64})()
            with patch.object(m6.pipeline, 'candidate_projection') as projection:
                with self.assertRaisesRegex(ValueError, 'before projection'):
                    cumulative.release_paths(context, Path(directory))
                projection.assert_not_called()

    def test_finite_additions_preserve_every_common_file(self):
        before = {'wall/core': 'original', 'door/core': 'g7'}
        after = before | {'released/new': 'candidate-adopted'}
        self.assertEqual(cumulative.asset_delta(before, after, {'released/new'}), ['released/new'])
        variants = [({'door/core': 'g7', 'released/new': 'candidate-adopted'}, {'released/new'}),
                    (after | {'door/core': 'changed'}, {'released/new'}),
                    (after | {'unknown/file': 'unreviewed'}, {'released/new'}),
                    (after, {'released/new', 'missing/file'}),
                    (after | {'.building-pipeline.lock': 'nonempty'}, {'released/new'})]
        for changed, allowed in variants:
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                cumulative.asset_delta(before, changed, allowed)
        lock = '.building-pipeline.lock'
        self.assertEqual(cumulative.asset_delta(before, after | {lock: m6.pipeline.digest(b'')}, {'released/new'}),
                         [lock, 'released/new'])

    def test_unknown_profile_cannot_select_a_custom_policy(self):
        for value in ({}, {'profile': None}, {'profile': True}, {'profile': '/tmp/plugin.py'}, {'profile': 'candidate'}):
            with self.subTest(value=value), patch.object(paired, 'cumulative_policy') as policy:
                with self.assertRaises(paired.native.AcceptanceError):
                    paired.entrypoint(value)
                policy.assert_not_called()
        with patch.object(paired, 'cumulative_policy') as policy:
            self.assertEqual(paired.entrypoint({'profile': paired.PROFILE}), Path(paired.__file__).resolve())
            policy.assert_not_called()

    def test_logical_projection_rejects_real_domain_changes(self):
        expected = cumulative.logical_oracle.expected_records(4)
        initial = {'records': expected, 'target_count': 36, 'souls': 15, 'completion_effects': 0}
        original = {'initial': initial}
        # Complete source-specific oracles must run first. Presentation evidence
        # remains separately hashed; this helper only binds identical game data.
        self.assertEqual(len(cumulative.logical_digest(original, 'small')), 64)
        for key, value in [('state', {'stored_water': 1, 'capacity': 50}), ('anchor', [0, 0]),
                           ('ordinal', True), ('kind', 'Bridge')]:
            changed = copy.deepcopy(original)
            changed['initial']['records'][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                cumulative.logical_digest(changed, 'small')
        for key, value in [('target_count', 35), ('souls', 14), ('completion_effects', 1)]:
            changed = copy.deepcopy(original)
            changed['initial'][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                cumulative.logical_digest(changed, 'small')

    def test_current_release_command_is_single_case_not_full_matrix(self):
        stage = {'binaries': {'reference': {'path': '/fixed/reference'}, 'current': {'path': '/fixed/current'}}}
        planned = {'job_root': '/tmp/comparison', 'adapter': 'Intel',
                   'release': {'context': {'assets': {'bindings': [{'identity': {'authority': 'release_approved'}}]}}}}
        current = cumulative.case_command(planned, stage, ('capture', 'small', 2, 'current'))
        self.assertIn('HW_BUILDING_ASSET_RELEASES=' + json.dumps(planned['release']['context']['assets']['bindings']), current)
        self.assertNotIn('--building-static-single-leg', current)
        self.assertEqual(current[current.index('--building-m6-mode') + 1], 'candidate')
        self.assertEqual(current[current.index('--repeat') + 1], '1')
        self.assertEqual(current[current.index('--binary') + 1], '/fixed/current')
        self.assertEqual(current[current.index('--output') + 1],
                         '/tmp/comparison/capture-small-2-current/capture-small-2-candidate')
        reference = cumulative.case_command(planned, stage, ('capture', 'small', 2, 'reference'))
        self.assertIn('--building-static-single-leg', reference)
        self.assertNotIn('--building-m6-mode', reference)
        self.assertFalse(any(token.startswith('HW_BUILDING_ASSET_RELEASES=') for token in reference))


if __name__ == '__main__':
    unittest.main()

class OperationReleasedOracle(unittest.TestCase):
    def test_scoped_context_keeps_complete_raw_oracle_and_all_case_calls(self):
        context = m6.ReleasedCaseContext('/repo', '/codec', 'a'*64, 'Intel', {}, {}, m6.resident.dependencies())
        planned = {'release': {'context': cumulative.context_value(context)}}
        stage = {'binaries': {'current': {'sha256': 'b'*64}}}
        sidecar = {'layout_sha256': 'original', 'initial': {
            'records': cumulative.logical_oracle.expected_records(4), 'target_count': 36,
            'souls': 15, 'completion_effects': 0}}
        def read(path):
            if path.name == 'requested-environment.json':
                return {'HW_BUILDING_M6_MODE': 'candidate', 'RUST_LOG': 'info,wgpu=error'}
            return sidecar
        row = {'binary_sha256': 'b'*64, 'session_sha256': 'c'*64, 'adapter': 'Intel', 'metrics': {}}
        with patch.object(paired, '_operation_context', return_value=context), \
             patch.object(m6, 'released_case_context', side_effect=AssertionError('no nested re-admission')), \
             patch.object(m6, '_verify_case_artifacts', return_value=row) as oracle, \
             patch.object(cumulative.native, 'read_json', side_effect=read), \
             patch.object(paired.inventory, 'boundaries', return_value={}):
            for instrument in ('capture', 'memory'):
                for repeat in (1, 2, 3):
                    checked, _, _ = cumulative.case_artifacts(planned, stage,
                        (instrument, 'small', repeat, 'current'), Path('/repo/target/native-acceptance/case'))
                    self.assertEqual(checked['presentation_sha256'], 'original')
            self.assertEqual(oracle.call_count, 6)
            oracle.side_effect = ValueError('corrupt raw')
            with self.assertRaisesRegex(ValueError, 'corrupt raw'):
                cumulative.case_artifacts(planned, stage, ('capture', 'small', 1, 'current'),
                                          Path('/repo/target/native-acceptance/case'))

    def test_private_artifact_oracle_still_rejects_mode_owner_and_binary(self):
        context = m6.ReleasedCaseContext('/repo', '/codec', 'a'*64, 'Intel', {}, {}, m6.resident.dependencies())
        good = ('capture', 'small', 1, 'candidate')
        for item, session, binary in ((('capture', 'small', 1, 'legacy-control'), '/repo/target/native-acceptance/a', 'b'),
                                      (good, '/foreign/a', 'b'),
                                      (good, '/repo/target/native-acceptance/a', 'wrong')):
            with patch.object(m6, '_verify_case_artifacts', return_value={'binary_sha256': 'b'}), self.assertRaises(ValueError):
                m6._verify_released_case_artifacts(context, item, Path(session), binary)
