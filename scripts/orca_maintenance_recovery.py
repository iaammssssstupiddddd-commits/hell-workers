"""Host-only tooling upgrade of a paused loop; preserve Run, source and sessions.

Not an approval or a Task result. Settled attempts remain immutable. A stopped,
unarmed bootstrap may be adopted only as resumable provider history, never as
successful work. Interrupted maintenance stops for inspection, without replay.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import subprocess
from contextlib import ExitStack
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop

B, S, R = loop.bindings, loop.STORAGE, loop.roles


def require_quiescent(data: dict, fleet: dict) -> None:
    if (data['phase'] != 'paused' or data.get('integration', {}).get('phase') != 'planned'
            or any(lane['phase'] != 'paused' for lane in data['lanes'].values())
            or data['inbox']['operation'] is not None):
        raise ValueError('maintenance requires paused lanes and untouched integration')
    rows = fleet.get('workers')
    if (fleet.get('scope', {}).get('run') != data['run']['context']['id']
            or fleet.get('page', {}).get('hasMore') is not False or not isinstance(rows, list)
            or {row.get('dispatchId') for row in rows} != set(data['attempts'])):
        raise ValueError('Run contains unknown attempts or an incomplete inventory')
    for row in rows:
        attempt = data['attempts'][row['dispatchId']]
        if (row.get('dispatchStatus') != 'completed' or row.get('workerState') != 'succeeded'
                or attempt.get('released') is not True or attempt.get('outcome') != 'completed'):
            raise ValueError('every known attempt must be settled and released')


def unarmed_proof(role: dict, attempt: dict, ticket: dict, closed: dict) -> dict:
    last = role['last']
    directory = loop.dispatch.task_bridge.root() / B.identity(last['orca_bridge'])
    journal = S.read_private_json(directory / 'journal.json', {})
    identity = S.read_private_json(directory / 'identity.json', {})
    receipt = closed.get('result', {}).get('close', {})
    if (last.get('phase') not in {'starting', 'unknown'} or last['key'] in role['tasks']
            or attempt.get('phase') != 'unknown' or attempt.get('task_id') is not None
            or attempt.get('dispatch_id') is not None or attempt.get('bridge_id') != last['orca_bridge']
            or attempt.get('terminal') != last.get('terminal')
            or closed.get('ok') is not True or receipt.get('handle') != last['terminal']
            or receipt.get('ptyKilled') is not True
            or closed.get('_meta', {}).get('runtimeId') != identity.get('runtime')
            or identity.get('terminal') != last['terminal'] or identity.get('repo') != ticket['repo']
            or journal.get('authority') is not None or journal.get('operations') != {}
            or journal.get('phase') not in {'bootstrap', 'closed', 'unknown'}
            or (directory / 'arm.json').exists() or (directory / 'arm.json').is_symlink()):
        raise ValueError('bootstrap recovery needs exact close proof and no Dispatch authority')
    pid = identity.get('pid')
    if type(pid) is not int or pid <= 0:
        raise ValueError('missing launcher process identity')
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        pass
    else:
        raise ValueError('bootstrap launcher is still alive')
    lease_id = identity['runtime'] + identity['terminal'] + identity['incarnation']
    with loop.acquire_host('workspace-' + hashlib.sha256(lease_id.encode()).hexdigest(), inherit=False):
        pass
    return {'identity': identity, 'journal': journal, 'terminal_close': closed}


def review_ticket(ticket: dict, source: str, evidence: dict) -> dict:
    expected = {'ticket': 'review-' + loop.checkpoints.task_key(ticket)[:40],
                'base': ticket['assignment_base'], 'head': ticket['base'],
                'source_sha256': source, 'validation_evidence': evidence['id']}
    return {'schema': 1, 'id': expected['ticket'], 'repo': ticket['repo'], 'branch': ticket['branch'],
            'base': ticket['base'], 'review_base': expected['base'], 'generation': ticket['generation'],
            'read_only': True, 'allowed_directories': [], 'source_sha256': source,
            'validation_evidence': evidence['id'],
            'prompt': 'Review the actual diff and documented contract without editing or testing. '
                      'This is a fresh review after a tooling-only maintenance upgrade; no prior verdict is accepted. '
                      'Report approved or changes_requested and blocking_findings with id, message, acceptance. '
                      'Use Python json.dumps, not manual quotation, for a single final ORCA_REVIEW_JSON: line '
                      'in the worker_done body. The exact subject fields are: ' + json.dumps(expected) +
                      '. Add verdict and blocking_findings only. A completed review is outcome succeeded even '
                      'when changes are requested. Review the code, not the earlier agent summary.'}


def recover(request: str, spec: dict, cli: Path) -> dict:
    if set(spec) != {'loop_sha256', 'new_base', 'sources', 'bootstrap_closes'}:
        raise ValueError('maintenance specification fields differ')
    operation = B.digest({'request': request, 'spec': spec})
    path = S.checked_directory(loop.root() / 'maintenance') / f'{operation}.json'
    if path.exists():
        raise ValueError('maintenance already recorded; inspect it, never replay mutations')
    with ExitStack() as leases:
        for name in ('ui-coordinator', 'workspace-' + B.digest({'driver': request}), loop.LOCK,
                     'worker-a', 'worker-b', 'reviewer'):
            leases.enter_context(loop.acquire_host(name, inherit=False))
        data = loop.load(request)
        if B.digest(data) != spec['loop_sha256']:
            raise ValueError('loop changed before maintenance')
        ui = loop.dispatch.ui_coordinator.read_registered_state(request)
        if ui['phase'] != 'exited' or ui['exit_code'] != 0 or ui['terminal'] != data['terminal']:
            raise ValueError('coordinator must exit normally before maintenance')
        loop.dispatch.checked_run(cli, data['terminal'], data['run']['context'])
        fleet = loop.dispatch.run_cli(cli, ['orchestration', 'worker-list', '--run', data['run']['context']['id']], 'maintenance-fleet')
        require_quiescent(data, fleet)
        # Use the normal mailbox verification/release accounting, not raw ACK.
        for _ in range(3):
            loop.mail.poll(data, loop.save)
            if loop.mail.drained(data):
                break
        if not loop.mail.drained(data):
            raise ValueError('unresolved messages require a coordinator decision')
        before = copy.deepcopy(data)
        target = data['integration']['target']
        new_base = spec['new_base']
        if R.git(Path(target['repo']), 'rev-parse', '--verify', new_base + '^{commit}') != new_base:
            raise ValueError('full tooling commit SHA required')
        names = R.git(Path(target['repo']), 'diff', '--name-only', target['base'], new_base).splitlines()
        if (not names or any(not name.startswith('scripts/') or name.startswith('scripts/tests/fixtures/') for name in names)
                or R.git(Path(target['repo']), 'merge-base', target['base'], new_base) != target['base']):
            raise ValueError('maintenance may update tooling only, not worker fixtures')
        repos = {slot: lane['ticket']['repo'] for slot, lane in data['lanes'].items()}
        repos['integration'] = target['repo']
        if set(spec['sources']) != set(repos) or set(spec['bootstrap_closes']) - set(data['lanes']):
            raise ValueError('exact workspace inventory required')
        journal = {'schema': 1, 'phase': 'prepared', 'request_id': request, 'spec': spec,
                   'before': {'loop': before, 'roles': {}}, 'sources': {}, 'proofs': {}, 'after': {}}
        for slot, value in repos.items():
            repo = Path(value)
            leases.enter_context(loop.acquire_host(R.workspace_slot(repo), inherit=False))
            source = R.fingerprint(repo)
            if source != spec['sources'][slot]:
                raise ValueError('workspace changed before maintenance')
            patch = loop.checkpoints.git(repo, 'diff', '--binary', 'HEAD')
            if R.git(repo, 'ls-files', '--others', '--exclude-standard') or R.git(repo, 'diff', '--cached', '--name-only'):
                raise ValueError('untracked or staged source requires separate preservation')
            journal['sources'][slot] = {'head': R.git(repo, 'rev-parse', 'HEAD'), 'sha256': source, 'patch': patch}
            if slot == 'integration':
                if patch or journal['sources'][slot]['head'] != target['base']:
                    raise ValueError('integration subject changed')
                continue
            lane, ticket = data['lanes'][slot], data['lanes'][slot]['ticket']
            role = B.read_state(slot, R.provider_for(ticket, slot), allow_pending=True)
            key = loop.checkpoints.task_key(ticket)
            if role['last']['key'] != key:
                raise ValueError('role belongs to another task')
            leases.enter_context(loop.acquire_host('workspace-' + key, inherit=False))
            snapshot = B.session_snapshot(R.prepare_runtime(f'{slot}/tasks/{key}'), role['provider'], repo)
            journal['before']['roles'][slot] = role
            journal['sources'][slot]['session'] = snapshot
            if slot in spec['bootstrap_closes']:
                attempt = S.read_private_json(loop.dispatch.dispatch_path(request, ticket), {})
                journal['proofs'][slot] = unarmed_proof(role, attempt, ticket, spec['bootstrap_closes'][slot])
                R.worker_scope(ticket, initial=False)
                if journal['sources'][slot]['head'] != ticket['base']:
                    raise ValueError('unarmed worker HEAD changed')
            else:
                if patch or role['last']['exit_code'] != 0 or role['last']['process_exited'] is not True:
                    raise ValueError('reviewed lane needs a clean exited checkpoint')
                R.require_completed_bridge(role['last'], repo)
                if any(role['tasks'][key][name] != val for name, val in snapshot.items()):
                    raise ValueError('worker history changed')
                if (not lane.get('reason', '').startswith('invalid review verdict:')
                        or R.fingerprint(repo) != lane['review_ticket']['source_sha256']):
                    raise ValueError('only an unchanged invalid-format review can be retried')
        S.write_ledger(path, journal)
        for slot, value in repos.items():
            repo, original = Path(value), journal['sources'][slot]
            if R.fingerprint(repo) != original['sha256']:
                raise ValueError('source changed after preservation')
            if original['patch']:
                # Exact inverse of the preserved bootstrap-only patch, never reset/clean.
                loop.checkpoints.git(repo, 'apply', '--reverse', '--check', '-', body=original['patch'] + '\n')
                loop.checkpoints.git(repo, 'apply', '--reverse', '-', body=original['patch'] + '\n')
            R.git(repo, 'merge', '--no-edit', new_base)
            if R.git(repo, 'status', '--porcelain'):
                raise ValueError('tooling merge did not leave a clean workspace')
        target['base'] = new_base
        data['integration']['source'] = R.fingerprint(Path(target['repo']))
        for slot, lane in data['lanes'].items():
            repo = Path(lane['ticket']['repo'])
            ticket = {**lane['ticket'], 'base': R.git(repo, 'rev-parse', 'HEAD'), 'assignment_base': new_base,
                      'generation': lane['ticket'].get('generation', 0) + 2}
            role = copy.deepcopy(journal['before']['roles'][slot])
            key, snapshot = role['last']['key'], journal['sources'][slot]['session']
            source = R.fingerprint(repo)
            role['tasks'][key] = {'key': key, 'ticket_sha256': B.digest(ticket),
                'subject': loop.checkpoints.subject(ticket), 'origin': str(repo), 'source_sha256': source, **snapshot}
            if slot in spec['bootstrap_closes']:
                role['last'].update(phase='recorded', process_exited=True, exit_code=None, maintenance_exit=str(path))
            role['maintenance_receipt'] = str(path)
            journal['after'][slot] = {'role': role, 'ticket': ticket}
            lane.update(ticket=ticket, source=source, phase='planned', failed=False, reason=None,
                        session=snapshot['session_id'],
                        follow_up='Continue this same assignment after a stopped tooling repair. Prior bootstrap edits '
                                  'were preserved and reverted, not accepted as work. Wait for the live Dispatch. '
                                  'Complete only the assigned scope, without tests or Git commands.')
            lane['history'].append({'from': 'paused', 'to': 'planned', 'maintenance': str(path)})
            for name in ('review', 'review_ticket', 'checkpoint', 'evidence', 'attempt', 'failure_reason'):
                lane.pop(name, None)
        # Publish an exact receipt before the resumable role records reference it.
        journal['phase'] = 'complete'
        S.write_ledger(path, journal)
        for slot, value in journal['after'].items():
            B.save_state(value['role'])
            assignment_path = B.state_path(slot).parent / 'assignments' / f"{value['role']['last']['key']}.json"
            S.write_ledger(assignment_path, loop.checkpoints.assignment(value['ticket'], slot))
        data.update(phase='paused', reason='maintenance complete; fresh review and coordinator restart required')
        data['maintenance_receipt'] = str(path)
        data['spec_sha256'] = B.digest({'lanes': [{'slot': slot, 'ticket': lane['ticket'], 'validation': lane['validation']}
                                                for slot, lane in data['lanes'].items()],
                                       'integration': {'target': target, 'validation': data['integration']['validation']}})
        loop.save(data)
    # Validation must acquire its ordinary role/workspace/heavy leases independently.
    for slot, lane in data['lanes'].items():
        if slot not in spec['bootstrap_closes']:
            config = lane['validation']
            evidence = loop.checkpoints.validate(lane['ticket'], slot, config['argv'], config['help_reason'], config['help_decision'])
            if evidence['exit_code'] != 0:
                raise ValueError('post-maintenance validation failed')
            lane.update(phase='review_pending', checkpoint={'next_ticket': lane['ticket']},
                        review_ticket=review_ticket(lane['ticket'], lane['source'], evidence), evidence=evidence)
    with loop.acquire_host(loop.LOCK, inherit=False):
        observed = loop.load(request)
        if observed.get('maintenance_receipt') != str(path) or observed['phase'] != 'paused':
            raise ValueError('loop changed during post-maintenance validation')
        data.update(phase='active', reason=None)
        loop.save(data)
    return {'receipt': str(path), 'run': data['run']['context'], 'phase': 'active'}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request-id', required=True)
    parser.add_argument('--spec', type=Path, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(recover(args.request_id, S.read_private_json(args.spec, {}), loop.dispatch.intake.default_orca_cli())))
        return 0
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Maintenance refused: {error}\n')


if __name__ == '__main__':
    raise SystemExit(main())
