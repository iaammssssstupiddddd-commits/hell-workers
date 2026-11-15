"""Boundary tests; the child response/Git fixtures are not native evidence."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_historical_resource_verifier as historical


class HistoricalResourceVerifier(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.current = Path(temporary.name).resolve() / 'primary'
        self.repo = self.current / 'target' / 'frozen'
        self.basis = self.current / 'basis'
        self.repo.mkdir(parents=True)
        self.basis.mkdir()
        real = Path(__file__).resolve().parents[2]
        for relative in set(historical.MODULES.values()) | set(historical.native.NATIVE_HARNESS_FILES):
            destination = self.repo / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(real / relative, destination)
            destination.chmod(0o444)
        self.repo.chmod(0o555)
        drivers = {name: historical.static.sha256(self.repo / relative)
                   for name, relative in historical.MODULES.items()}
        drivers['primary_verifier_harness'] = historical.native.native_harness_fingerprint(self.repo)
        self.manifest = {'plan': {'profile': historical.original.PROFILE,
            'job_root': str(self.basis), 'drivers': drivers, 'identity': {'subject_commit': 'fixture'}},
            'plateau': {'global_shared_mesh_plateau': False, 'accepted': False}}
        self.write_manifest()
        self.result = {'status': 'pass', 'profile': historical.original.PROFILE,
            'source': self.manifest['plan']['identity'], 'capture_runs': 2,
            'parent_timing_runs': 12, 'resource_only': True, 'accepted': False,
            'promotion_authority': False, 'performance_budget_evidence': False,
            'plateau': self.manifest['plateau']}
        for name, mock in (
            ('validate_repo', patch.object(historical.native, 'validate_repo', side_effect=lambda p: Path(p))),
            ('subject', patch.object(historical.native, 'git_subject', return_value=historical.COMMIT)),
            ('dirty', patch.object(historical.native, 'git_dirty_paths', return_value=[])),
            ('common', patch.object(historical, 'common_dir', return_value=self.current)),
            ('ignored', patch.object(historical, 'ignored_files', return_value=[])),
        ):
            setattr(self, name, mock.start())
            self.addCleanup(mock.stop)
        self.binding = historical.bind(self.repo, self.basis, self.current)

    def write_manifest(self):
        (self.basis / 'manifest.json').write_text(json.dumps(self.manifest))

    def completed(self, result=None, code=0):
        return subprocess.CompletedProcess([], code, json.dumps(self.result if result is None else result), 'failure')

    def verify(self):
        return historical.verify(self.binding, self.basis, self.current)

    def test_exact_isolated_verifier_only_and_original_nonacceptance(self):
        with patch.object(historical.subprocess, 'run', return_value=self.completed()) as child:
            self.assertEqual(self.verify(), self.result)
        argv = child.call_args.args[0]
        self.assertEqual(argv[1:4], ['-I', '-B', '-c'])
        self.assertEqual(argv[4], historical.CODE)
        self.assertEqual(argv[5:], [str(self.repo), str(self.basis)])
        self.assertEqual(child.call_args.kwargs['cwd'], self.repo)
        self.assertEqual(child.call_args.kwargs['stdin'], subprocess.DEVNULL)
        self.assertNotIn('PYTHONPATH', child.call_args.kwargs['env'])
        self.assertEqual(child.call_args.kwargs['env']['GIT_OPTIONAL_LOCKS'], '0')
        self.assertFalse(self.result['accepted'])
        self.assertFalse(self.result['plateau']['global_shared_mesh_plateau'])

    def test_foreign_commit_dirty_git_owner_or_writable_tree_never_executes(self):
        for mock, value in ((self.subject, '0' * 40), (self.dirty, ['changed.py']),
                            (self.ignored, [b'scripts/__pycache__/foreign.pyc']),
                            (self.common, None)):
            old = mock.return_value
            if mock is self.common:
                mock.side_effect = [self.current, self.current / 'foreign']
            else:
                mock.return_value = value
            with patch.object(historical.subprocess, 'run') as child:
                with self.assertRaises(historical.native.AcceptanceError):
                    self.verify()
                child.assert_not_called()
            mock.return_value = old
            mock.side_effect = None
        self.repo.chmod(0o755)
        with patch.object(historical.subprocess, 'run') as child:
            with self.assertRaises(historical.native.AcceptanceError):
                self.verify()
            child.assert_not_called()

    def test_each_execution_module_and_historical_catalog_tamper_rejected(self):
        catalog = next(p for p in historical.native.NATIVE_HARNESS_FILES
                       if p not in historical.MODULES.values())
        for relative in [*historical.MODULES.values(), catalog]:
            path = self.repo / relative
            saved = path.read_bytes()
            path.chmod(0o644)
            path.write_bytes(saved + b'\n# changed\n')
            path.chmod(0o444)
            with patch.object(historical.subprocess, 'run') as child:
                with self.assertRaises(historical.native.AcceptanceError):
                    self.verify()
                child.assert_not_called()
            path.chmod(0o644)
            path.write_bytes(saved)
            path.chmod(0o444)

    def test_binding_entry_path_basis_driver_and_manifest_changes_rejected(self):
        for key, value in [('entrypoint', 'arbitrary.py'), ('repo', str(self.current)),
                           ('basis_root', str(self.current)), ('commit', '0' * 40),
                           ('entry_sha256', '0' * 64), ('drivers', {}),
                           ('basis_manifest_sha256', '0' * 64)]:
            with self.subTest(key=key), patch.object(historical.subprocess, 'run') as child:
                bad = copy.deepcopy(self.binding)
                bad[key] = value
                with self.assertRaises(historical.native.AcceptanceError):
                    historical.verify(bad, self.basis, self.current)
                child.assert_not_called()
        self.manifest['plan']['job_root'] = str(self.current)
        self.write_manifest()
        with self.assertRaises(historical.native.AcceptanceError):
            self.verify()

    def test_symlink_and_writable_module_are_not_frozen(self):
        path = self.repo / historical.ENTRY
        path.chmod(0o644)
        with self.assertRaises(historical.native.AcceptanceError):
            self.verify()
        path.chmod(0o444)
        saved = path.read_bytes()
        path.unlink()
        elsewhere = self.current / 'elsewhere.py'
        elsewhere.write_bytes(saved)
        elsewhere.chmod(0o444)
        path.symlink_to(elsewhere)
        with self.assertRaises(historical.native.AcceptanceError):
            self.verify()

    def test_child_failure_timeout_and_invalid_response_rejected(self):
        invalid = [self.completed(code=1), subprocess.CompletedProcess([], 0, '{}', ''),
                   subprocess.CompletedProcess([], 0, 'x' * 65537, ''),
                   subprocess.CompletedProcess([], 0, '{"status":"pass","status":"pass"}', ''),
                   subprocess.CompletedProcess([], 0, 'NaN', '')]
        for completed in invalid:
            with patch.object(historical.subprocess, 'run', return_value=completed):
                with self.assertRaises((historical.native.AcceptanceError, ValueError)):
                    self.verify()
        with patch.object(historical.subprocess, 'run', side_effect=subprocess.TimeoutExpired('fixed', 180)):
            with self.assertRaises(historical.native.AcceptanceError):
                self.verify()

    def test_response_cannot_promote_scope_or_erase_old_failure(self):
        for key, value in [('accepted', True), ('promotion_authority', True),
                           ('performance_budget_evidence', True), ('resource_only', False),
                           ('capture_runs', True), ('parent_timing_runs', 11),
                           ('source', {}), ('plateau', {'global_shared_mesh_plateau': True, 'accepted': True})]:
            bad = copy.deepcopy(self.result)
            bad[key] = value
            with self.subTest(key=key), patch.object(historical.subprocess, 'run', return_value=self.completed(bad)):
                with self.assertRaises(historical.native.AcceptanceError):
                    self.verify()

    def test_basis_mutation_during_child_is_rejected(self):
        def child(*_, **__):
            self.manifest['changed_during_verification'] = True
            self.write_manifest()
            return self.completed()
        with patch.object(historical.subprocess, 'run', side_effect=child):
            with self.assertRaises(historical.native.AcceptanceError):
                self.verify()

    def test_module_mutation_during_child_is_rejected(self):
        def child(*_, **__):
            path = self.repo / historical.ENTRY
            path.chmod(0o644)
            path.write_bytes(path.read_bytes() + b'\n# changed during verification\n')
            path.chmod(0o444)
            return self.completed()
        with patch.object(historical.subprocess, 'run', side_effect=child):
            with self.assertRaises(historical.native.AcceptanceError):
                self.verify()


if __name__ == '__main__':
    unittest.main()
