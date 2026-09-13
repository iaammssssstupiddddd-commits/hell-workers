from __future__ import annotations

import hashlib
import io
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from contextlib import redirect_stdout, redirect_stderr
from pathlib import Path
from unittest.mock import patch

from scripts import dev, dev_tools, install_dev_tools


class ToolResolutionTests(unittest.TestCase):
    def test_cargo_home_precedes_path_and_wrong_version_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo_bin, path_bin = root / 'cargo/bin', root / 'path'
            for folder, version in ((cargo_bin, '0.1.0'), (path_bin, '0.20.2')):
                folder.mkdir(parents=True)
                binary = folder / 'cargo-deny'
                binary.write_text(f'#!{sys.executable}\nprint("cargo-deny {version}")\n')
                binary.chmod(0o755)
            environment = {**os.environ, 'CARGO_HOME': str(root / 'cargo'), 'PATH': str(path_bin)}
            self.assertEqual(dev_tools.resolve_tool('cargo-deny', environment), str(cargo_bin / 'cargo-deny'))
            with self.assertRaisesRegex(RuntimeError, 'expected 0.20.2'):
                dev_tools.probe_tool('cargo-deny', '0.20.2', environment)
            (cargo_bin / 'cargo-deny').unlink()
            self.assertEqual(dev_tools.probe_tool('cargo-deny', '0.20.2', environment), str(path_bin / 'cargo-deny'))

    def test_missing_tool_fails_with_install_help(self) -> None:
        with patch.object(dev_tools, 'resolve_tool', return_value=None):
            with self.assertRaisesRegex(RuntimeError, 'install_dev_tools.py'):
                dev_tools.probe_tool('ruff', '0.16.7', {})

    def test_ruff_config_mismatch_fails_before_execution(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'ruff.toml').write_text('required-version = "==0.0.0"\n')
            manifest = dev_tools.load_manifest(dev.REPO_ROOT)
            with patch.object(dev_tools, 'load_manifest', return_value=manifest), patch.object(dev_tools, 'probe_tool') as probe:
                with self.assertRaisesRegex(RuntimeError, 'differs'):
                    dev_tools.preflight(root, {}, ['ruff'])
                probe.assert_not_called()


class DriverTests(unittest.TestCase):
    def test_diagnosis_environment_disables_install_without_creating_temp(self) -> None:
        with patch.object(dev, 'cargo_environment', return_value={}) as cargo_env:
            self.assertEqual(dev.quality_environment()['RUSTUP_AUTO_INSTALL'], '0')
            self.assertFalse(cargo_env.call_args.kwargs['create_temp_dir'])

    def test_missing_toolchain_and_quality_tools_are_diagnosed_without_install(self) -> None:
        with patch.object(dev, 'cargo_environment', return_value={}), patch.object(dev.shutil, 'which', return_value='/fixture/tool'), patch.object(dev.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1, '', 'toolchain missing')) as run, patch.object(dev_tools, 'preflight', side_effect=RuntimeError('quality tool missing')), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            self.assertEqual(dev.doctor(), 1)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.args[0], ['rustc', '-vV'])
            self.assertEqual(run.call_args.kwargs['env']['RUSTUP_AUTO_INSTALL'], '0')

    def test_exact_probed_binaries_and_environment_are_executed(self) -> None:
        environment = {'CARGO_HOME': '/persistent/cargo', 'PATH': '/persistent/bin'}
        binaries = {'cargo-deny': '/persistent/cargo/bin/cargo-deny'}
        with patch.object(dev, 'quality_environment', return_value=environment), patch.object(dev_tools, 'preflight', return_value=binaries) as preflight, patch.object(dev, 'acquire_activity') as acquire, patch.object(dev.subprocess, 'run') as run:
            dev.run_quality_tools(deps=True, offline=True)
            self.assertIs(preflight.call_args.args[1], environment)
            self.assertIs(run.call_args.kwargs['env'], environment)
            command = run.call_args.args[0]
            self.assertEqual(command[0], binaries['cargo-deny'])
            self.assertLess(command.index('--offline'), command.index('check'))
            self.assertIn('--locked', command)
            acquire.return_value.close.assert_called_once()

    def test_tool_or_network_failure_propagates_exit_and_releases_activity(self) -> None:
        for arguments in (['lint'], ['deps'], ['deps', '--offline']):
            with self.subTest(arguments=arguments), patch.object(dev, 'quality_environment', return_value={}), patch.object(dev_tools, 'preflight', return_value={name: '/tool/' + name for name in dev_tools.TOOL_NAMES}), patch.object(dev, 'acquire_activity') as acquire, patch.object(dev.subprocess, 'run', side_effect=subprocess.CalledProcessError(7, ['tool'])):
                self.assertEqual(dev.main(arguments), 7)
                if arguments[0] == 'deps':
                    acquire.return_value.close.assert_called_once()

    def test_verify_stops_before_rust_when_preflight_fails(self) -> None:
        with patch.object(dev_tools, 'preflight', side_effect=RuntimeError('missing tool')), patch.object(dev, 'quality_environment', return_value={}), patch.object(dev, 'run_command') as run, redirect_stderr(io.StringIO()):
            self.assertEqual(dev.main(['verify']), 1)
            run.assert_not_called()


class InstallerTests(unittest.TestCase):
    @staticmethod
    def archive() -> bytes:
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode='w:gz') as bundle:
            for name, value in (('../../ruff', b'binary'), ('../../unrelated', b'escape')):
                info = tarfile.TarInfo(name)
                info.size = len(value)
                bundle.addfile(info, io.BytesIO(value))
        return output.getvalue()

    def test_checksum_failure_preserves_installed_binary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            (destination / 'ruff').write_bytes(b'existing')
            with patch.object(install_dev_tools.urllib.request, 'urlopen', return_value=io.BytesIO(self.archive())):
                with self.assertRaisesRegex(RuntimeError, 'SHA-256 mismatch'):
                    install_dev_tools.install_archive('ruff', {'url': 'https://fixture.invalid', 'sha256': '0' * 64}, destination)
            self.assertEqual((destination / 'ruff').read_bytes(), b'existing')
            self.assertEqual(len(list(destination.iterdir())), 1)

    def test_extracts_only_named_binary_without_following_archive_paths(self) -> None:
        archive = self.archive()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / 'bin'
            with patch.object(install_dev_tools.urllib.request, 'urlopen', return_value=io.BytesIO(archive)):
                install_dev_tools.install_archive('ruff', {'url': 'https://fixture.invalid', 'sha256': hashlib.sha256(archive).hexdigest(), 'version': '0.16.7'}, destination)
            self.assertEqual((destination / 'ruff').read_bytes(), b'binary')
            self.assertTrue(os.access(destination / 'ruff', os.X_OK))
            self.assertEqual(sorted(p.relative_to(root).as_posix() for p in root.rglob('*')), ['bin', 'bin/ruff'])


if __name__ == '__main__':
    unittest.main()
