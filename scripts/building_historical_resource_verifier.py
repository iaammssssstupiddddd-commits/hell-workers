"""Read-only execution of the original resource verifier, never a new baseline.

The closed cumulative profile alone uses this source-only checkout. Current
measurement identities stay separate; no catalog substitution or launch argv
from an evidence file is executed.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys

if __package__:
    from . import building_resource_inventory_acceptance as original
else:
    import building_resource_inventory_acceptance as original

native, static = original.native, original.static
PROFILE = 'building-cumulative-historical-resource-verifier-v1'
COMMIT = '29e58ebfbc052c35c1a2eed6f6fa172989276b1b'
ENTRY = 'scripts/building_resource_inventory_acceptance.py'
MODULES = {
    'building_resource_inventory_acceptance.py': ENTRY,
    'building_art_static_acceptance.py': 'scripts/building_art_static_acceptance.py',
    'building_resource_inventory.py': 'scripts/building_resource_inventory.py',
    'building_native_executable.py': 'scripts/building_native_executable.py',
    'native_acceptance.py': '.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py',
}
CODE = (
    'import json,sys; from pathlib import Path; '
    "sys.path.insert(0,str(Path(sys.argv[1])/'scripts')); "
    'import building_resource_inventory_acceptance as frozen; '
    'print(json.dumps(frozen.verify(Path(sys.argv[2])),sort_keys=True,allow_nan=False))'
)


def common_dir(repo):
    value = subprocess.run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'],
                           cwd=repo, capture_output=True, text=True, timeout=10, check=True)
    return Path(value.stdout.strip()).resolve(strict=True)


def ignored_files(repo):
    value = subprocess.run(['git', 'ls-files', '--others', '--ignored', '--exclude-standard', '-z'],
                           cwd=repo, capture_output=True, timeout=10, check=True)
    return value.stdout.split(b'\0')[:-1]


def frozen_file(path, repo):
    native.require(path.is_file() and not path.is_symlink() and path.resolve() == path
                   and path.is_relative_to(repo) and not path.stat().st_mode & 0o222,
                   'historical verifier requires canonical read-only regular files')
    return static.sha256(path)


def bind(repo, basis, current):
    repo, basis, current = Path(repo), Path(basis), Path(current)
    native.require(all(p.is_absolute() and p.resolve() == p for p in (repo, basis, current))
                   and repo.is_relative_to(current / 'target') and repo != current,
                   'historical verifier path/ownership differs')
    native.require(native.validate_repo(repo) == repo and native.git_subject(repo) == COMMIT
                   and not native.git_dirty_paths(repo) and not repo.stat().st_mode & 0o222
                   and common_dir(repo) == common_dir(current)
                   and not ignored_files(repo) and not (repo / 'target').exists(),
                   'historical verifier commit, clean state or Git ownership differs')
    manifest = native.read_json(basis / 'manifest.json')
    plan = manifest['plan']
    native.require(Path(plan['job_root']) == basis and plan['profile'] == original.PROFILE,
                   'historical resource basis root/scope differs')
    drivers = plan['drivers']
    native.require(set(drivers) == set(MODULES) | {'primary_verifier_harness'},
                   'historical resource verifier module inventory differs')
    for name, relative in MODULES.items():
        native.require(frozen_file(repo / relative, repo) == drivers[name],
                       'historical resource verifier module changed: ' + name)
    # The native module owns its catalog. Requiring its actual executing bytes
    # to match avoids interpreting a historical catalog with another revision.
    native.require(static.sha256(Path(native.__file__)) == drivers['native_acceptance.py'],
                   'historical resource native catalog reader revision differs')
    for relative in native.NATIVE_HARNESS_FILES:
        frozen_file(repo / relative, repo)
    native.require(native.native_harness_fingerprint(repo) == drivers['primary_verifier_harness'],
                   'historical resource verifier catalog changed')
    return {'profile': PROFILE, 'repo': str(repo), 'commit': COMMIT, 'entrypoint': ENTRY,
            'entry_sha256': drivers['building_resource_inventory_acceptance.py'],
            'drivers': drivers, 'harness_fingerprint': drivers['primary_verifier_harness'],
            'basis_root': str(basis), 'basis_manifest_sha256': static.sha256(basis / 'manifest.json')}


def response(text):
    def pairs(items):
        result = {}
        for key, value in items:
            native.require(key not in result, 'duplicate historical verifier response key')
            result[key] = value
        return result

    def nonfinite(_):
        raise ValueError('nonfinite historical verifier response')

    native.require(len(text.encode()) <= 65536, 'historical verifier response exceeds bound')
    result = json.loads(text, object_pairs_hook=pairs, parse_constant=nonfinite)
    native.require(isinstance(result, dict), 'historical verifier response is not an object')
    return result


def verify(binding, basis, current):
    native.require(isinstance(binding, dict), 'historical resource verifier binding required')
    repo = Path(binding['repo'])
    native.require(bind(repo, basis, current) == binding,
                   'historical resource verifier binding changed')
    env = {key: value for key, value in os.environ.items()
           if key not in {'PYTHONPATH', 'PYTHONHOME', 'PYTHONSTARTUP'}}
    env['PYTHONDONTWRITEBYTECODE'] = '1'
    env['GIT_OPTIONAL_LOCKS'] = '0'
    try:
        completed = subprocess.run([sys.executable, '-I', '-B', '-c', CODE, str(repo), str(basis)],
                                   cwd=repo, env=env, stdin=subprocess.DEVNULL,
                                   capture_output=True, text=True, timeout=180, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise native.AcceptanceError('historical resource verifier could not complete') from error
    native.require(completed.returncode == 0,
                   'historical resource verifier failed: ' + completed.stderr[-1500:])
    result = response(completed.stdout)
    native.require(bind(repo, basis, current) == binding,
                   'historical verifier/basis changed during verification')
    manifest = native.read_json(Path(basis) / 'manifest.json')
    native.require(set(result) == {'status', 'profile', 'source', 'capture_runs', 'parent_timing_runs',
                   'resource_only', 'accepted', 'promotion_authority', 'performance_budget_evidence', 'plateau'}
                   and result['status'] == 'pass' and result['profile'] == original.PROFILE
                   and json.dumps(result['source'], sort_keys=True, allow_nan=False)
                       == json.dumps(manifest['plan']['identity'], sort_keys=True, allow_nan=False)
                   and type(result['capture_runs']) is int and result['capture_runs'] == 2
                   and type(result['parent_timing_runs']) is int and result['parent_timing_runs'] == 12
                   and result['resource_only'] is True
                   and all(result[key] is False for key in ('accepted', 'promotion_authority', 'performance_budget_evidence'))
                   and json.dumps(result['plateau'], sort_keys=True, allow_nan=False)
                       == json.dumps(manifest['plateau'], sort_keys=True, allow_nan=False)
                   and result['plateau']['global_shared_mesh_plateau'] is False
                   and result['plateau']['accepted'] is False,
                   'historical verifier response scope or raw result differs')
    return result
