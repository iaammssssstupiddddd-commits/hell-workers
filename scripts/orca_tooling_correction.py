"""Coordinator-owned tooling correction of a rejected integration candidate.

Preserve the rejected subject and Run. No approval is inherited by the new head:
ordinary combined validation and the fixed reviewer must inspect it again.
Interrupted corrections are inspection-only; never replay Git writes blindly.
"""
from __future__ import annotations

import argparse
import copy
import json
import re
from contextlib import ExitStack
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop


def correct(request: str, terminal: str, spec: dict) -> dict:
    if (set(spec) != {'loop_sha256', 'commit', 'reason', 'validation'}
            or not isinstance(spec['reason'], str) or not spec['reason'].strip()
            or len(spec['reason']) > 2000 or not re.fullmatch(r'[a-f0-9]{40}', spec['commit'])):
        raise ValueError('exact correction commit, loop digest and bounded rationale required')
    loop.checked_validation(spec['validation'])
    operation = loop.bindings.digest({'request': request, 'spec': spec})
    path = loop.STORAGE.checked_directory(loop.root() / 'tooling-corrections') / f'{operation}.json'
    with ExitStack() as leases:
        leases.enter_context(loop.acquire_host(loop.LOCK, inherit=False))
        if path.exists() or path.is_symlink():
            raise ValueError('correction already recorded; inspect without replay')
        data = loop.load(request)
        final = data.get('integration', {})
        if (loop.bindings.digest(data) != spec['loop_sha256'] or data['terminal'] != terminal
                or data['phase'] != 'active' or final.get('phase') != 'correction_required'
                or final['review']['verdict'] != 'changes_requested'
                or not final['review']['blocking_findings'] or not loop.mail.drained(data)
                or any(item.get('released') is not True or item.get('completion_acknowledged') is not True
                       for item in data['attempts'].values())
                or any(lane['phase'] != 'approved' for lane in data['lanes'].values())
                or len(final.get('tooling_corrections', [])) >= 3):
            raise ValueError('exact settled rejected integration required')
        loop.dispatch.checked_coordinator(request, terminal)
        if not loop.active_issue(data):
            raise ValueError('issue is no longer active')
        target = final['target']
        repo = Path(target['repo'])
        for name in ('worker-a', 'worker-b', loop.roles.workspace_slot(repo)):
            leases.enter_context(loop.acquire_host(name, inherit=False))
        previous = final['receipt']
        loop.integration.check_receipt(previous)
        sealed = loop.roles.read_review_receipt(final['review']['receipt_id'])
        if (sealed['record'] != {key: value for key, value in final['review'].items() if key != 'receipt_id'}
                or sealed['ticket_sha256'] != loop.bindings.digest(final['review_ticket'])):
            raise ValueError('rejected review differs from its sealed receipt')
        loop.integration.check_approvals(target, previous['approvals'])
        git = loop.integration.git
        commit = spec['commit']
        if (git(repo, 'rev-parse', '--verify', commit + '^{commit}') != commit
                or git(repo, 'merge-base', target['base'], commit) != target['base']):
            raise ValueError('correction must descend from the original tooling base')
        names = git(repo, 'diff', '--no-renames', '--name-only', target['base'], commit).splitlines()
        if (not names or any(not name.startswith('scripts/') or name.startswith('scripts/tests/fixtures/')
                             for name in names)):
            raise ValueError('only host tooling corrections are allowed; preserve worker and game code')
        head = previous['head']
        tree = git(repo, 'merge-tree', '--write-tree', '--no-messages', head, commit)
        if not re.fullmatch(r'[a-f0-9]{40}', tree) or tree == git(repo, 'rev-parse', head + '^{tree}'):
            raise ValueError('correction needs a clean changed tree')
        candidate = git(repo, 'commit-tree', tree, '-p', head, '-p', commit,
                        body=f'fix: correct rejected integration tooling\n\nOrca-Correction: {operation}\n')
        receipt = {'schema': 1, 'operation': operation, 'phase': 'prepared', 'target': target,
                   'approvals': previous['approvals'], 'previous': previous, 'start': head,
                   'generation': previous['generation'] + 1, 'source_before': previous['source_after'],
                   'commits': [{'head': candidate, 'parents': [head, commit], 'tree': tree}],
                   'head': candidate, 'coordinator_correction': spec}
        journal = {'schema': 1, 'phase': 'prepared', 'before': copy.deepcopy(data), 'spec': spec}
        loop.STORAGE.write_ledger(path, journal)
        receipt_path = loop.integration.root() / f'{operation}.json'
        loop.STORAGE.write_ledger(receipt_path, receipt)
        # Keep scheduler/worker/target leases through checkout and publication.
        # finish independently rechecks fixed-reviewer receipts before ref update.
        receipt = loop.integration.finish(receipt, receipt_path)
        replacement = {'target': target, 'validation': spec['validation'], 'phase': 'validating',
                       'reason': None, 'source': receipt['source_after'], 'receipt': receipt,
                       'history': final.get('history', []),
                       'tooling_corrections': [*final.get('tooling_corrections', []), str(path)]}
        data['integration'] = replacement
        loop.save(data)
        loop.STORAGE.write_ledger(path, {**journal, 'phase': 'complete', 'head': receipt['head']})
        return {'phase': 'validating', 'head': receipt['head'], 'receipt': str(path), 'run': data['run']}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request-id', required=True)
    parser.add_argument('--coordinator', required=True)
    parser.add_argument('--spec', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(correct(args.request_id, args.coordinator,
                             loop.STORAGE.read_private_json(args.spec, {}))))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
