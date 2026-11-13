"""Adjacent old-foundation versus actual nine-kind released static evidence.

Private implementation draft. Apply only after M5 canonical adoption closes the
old live-verifier source dependencies; no acceptance authority is provided here.
"""
from __future__ import annotations

from dataclasses import asdict
import json
from pathlib import Path

if __package__:
    from . import building_foundation_paired_acceptance as paired
    from . import building_m6_acceptance as m6
    from . import building_historical_resource_verifier as historical
    from .perf_tool.artifact_readers import building_m6 as release_oracle
    from .perf_tool.artifact_readers import building_art_static as logical_oracle
else:
    import building_foundation_paired_acceptance as paired
    import building_m6_acceptance as m6
    import building_historical_resource_verifier as historical
    from perf_tool.artifact_readers import building_m6 as release_oracle
    from perf_tool.artifact_readers import building_art_static as logical_oracle

native, pipeline, static = paired.native, m6.pipeline, paired.static
PROFILE = paired.CUMULATIVE_PROFILE
# Closed inputs used by the existing static/performance producer. No custom
# environment/callback can be supplied in a plan or receipt.
EXCLUDED_ENV = (
    'HW_BUILDING_ART_SESSION', 'HW_BUILDING_GROUP_ART_SESSION', 'HW_BUILDING_RELEASE_GALLERY_SESSION',
    'HW_BUILDING_M6_MODE', 'HW_BUILDING_M6_CANDIDATES', 'HW_BUILDING_GROUP_COMPARISON',
    'HW_BUILDING_ASSET_RELEASES', 'HW_BUILDING_ORDINARY_ART_SESSION',
    'HW_BUILDING_PRODUCTION_OBSERVATIONS', 'HW_M2_ACCEPTANCE_PROBE',
    'HW_BRIDGE_ACCEPTANCE_PROBE', 'HW_NATIVE_UI_ROOT',
)


def dependencies():
    modules = (m6, release_oracle, logical_oracle, m6.art, pipeline, historical)
    return {Path(module.__file__).name: static.sha256(Path(module.__file__)) for module in modules} | {
        Path(__file__).name: static.sha256(Path(__file__)),
        'numeric_verifiers': pipeline.numeric_verifier_inventory(),
    }


def release_paths(context, asset_root):
    """Derive a finite allowlist from verified exact released identities."""
    pipeline.require(pipeline.digest(Path(context.codec).read_bytes()) == context.codec_sha256,
                     'frozen release codec changed before projection')
    paths = set()
    pipeline.require([b['identity']['kind'] for b in context.assets['bindings']]
                     == list(pipeline.RELEASE_GROUPS['m5']), 'exact eight released kinds required')
    for binding, manifest in zip(context.assets['bindings'], context.assets['manifests'], strict=True):
        ident = binding['identity']
        kind, generation = ident['kind'], ident['generation']
        slug = pipeline.KINDS[kind]['slug']
        pointer_path = f'authority/building-{slug}.json'
        journal_path = f'transactions/building-{slug}-{generation}.json'
        pointer = pipeline.read(asset_root / pointer_path)
        journal = pipeline.read(asset_root / journal_path)
        plan = journal['plan']
        projection = plan['projection']
        pipeline.require(pipeline.digest(Path(context.codec).read_bytes()) == context.codec_sha256,
                         'frozen release codec changed before candidate projection')
        candidate, text = pipeline.candidate_projection(manifest, Path(context.codec))
        pipeline.require(pipeline.digest(Path(context.codec).read_bytes()) == context.codec_sha256,
                         'frozen release codec changed during candidate projection')
        pipeline.require(set(journal) == {'schema_version', 'state', 'new', 'plan_sha256', 'plan',
                         'previous', 'previous_locator', 'evidence', 'approval'}
            and type(journal['schema_version']) is int and journal['schema_version'] == 1
            and journal['state'] == 'applied' and pointer == journal['new']
            and pipeline.exact_json(pointer['identity'], ident)
            and pipeline.exact_json(manifest['identity'], ident)
            and plan['kind'] == kind and plan['operation'] == 'promote_building_asset_set'
            and type(plan['schema_version']) is int and plan['schema_version'] == 1
            and pipeline.exact_json(plan['candidate_identity'], candidate['identity'])
            and plan['candidate_sha256'] == pipeline.digest(text.encode())
            and plan['codec_sha256'] == context.codec_sha256
            and plan['previous'] is None and journal['previous'] is None
            and plan['previous_locator'] is None and journal['previous_locator'] is None,
            'actual first-introduction release transaction differs')
        pipeline.require(pointer['plan_sha256'] == journal['plan_sha256']
            == pipeline.digest(pipeline.canonical(plan))
            and pointer['evidence_sha256'] == pipeline.digest(pipeline.canonical(journal['evidence']))
            and pointer['approval_sha256'] == pipeline.digest(pipeline.canonical(journal['approval']))
            and projection['manifest'].encode() == (asset_root / binding['locator']).read_bytes()
            and projection['receipt'].encode() == pipeline.file_bytes(asset_root, manifest['receipt']),
            'release receipt/projection binding differs')
        pipeline.check_release_approval(journal['approval'], candidate['identity'], pointer['evidence_sha256'])
        paths.update((pointer_path, journal_path, binding['locator'], manifest['receipt']['path']))
        paths.update(record['path'] for record in manifest['artifacts'])
        prefix = f'building_sets/{slug}/{generation}/provenance'
        paths.update(f'{prefix}/{name}' for name in ('source', 'geometry.json', 'art-approval.json', 'numeric-approval.json'))
    return paths


def asset_delta(reference, current, permitted):
    pipeline.require(set(reference) <= set(current), 'baseline asset removed')
    pipeline.require(all(current[path] == value for path, value in reference.items()), 'common baseline asset replaced')
    extras = set(current) - set(reference)
    # The pipeline lock is a known empty coordination file, never a directory
    # exclusion or an input/authority exception.
    lock = '.building-pipeline.lock'
    expected = set(permitted)
    if lock in extras:
        pipeline.require(current[lock] == pipeline.digest(b''), 'asset pipeline lock contains data')
        expected.add(lock)
    pipeline.require(extras == expected, 'unknown or missing exact release asset addition')
    return sorted(extras)


def _plan_inputs(args, planned):
    repo, reference = (Path(planned['repos'][source]) for source in ('current', 'reference'))
    if 'release' in planned:
        # Saved context is never initial admission: absent/null/invalid SHA must
        # fail before the codec can be invoked by the snapshot factory.
        expected = planned['release']['context']['codec_sha256']
        context = m6.released_case_context(repo, Path(args.codec).resolve(), planned['adapter'],
                                           expected_codec_sha256=expected)
    else:
        context = m6.released_case_context(repo, Path(args.codec).resolve(), planned['adapter'])
    pipeline.require(context.identity == planned['identities']['current'], 'released context source differs')
    before, after = m6.file_inventory(reference / 'assets'), m6.file_inventory(repo / 'assets')
    additions = asset_delta(before, after, release_paths(context, repo / 'assets'))
    return ({'context': asdict(context), 'reference_assets': before, 'current_assets': after,
             'admitted_additions': additions}, context)


def plan_inputs(args, planned):
    return _plan_inputs(args, planned)[0]


def context_value(context):
    pipeline.require(type(context) is m6.ReleasedCaseContext, 'foreign release context type')
    return asdict(context)


def check_inputs(planned):
    value = planned['release']
    # Recreate every identity, byte inventory, pointer, journal and dependency;
    # never infer release authority from a performance result.
    class Inputs:
        codec = Path(value['context']['codec'])
    actual, context = _plan_inputs(Inputs(), planned)
    pipeline.require(pipeline.exact_json(value, actual), 'frozen released asset boundary changed')
    return context


def bind_resource_verifier(args, planned):
    pipeline.require(args.resource_verifier_repo is not None, 'frozen resource verifier source required')
    return historical.bind(args.resource_verifier_repo, Path(planned['resource_basis']),
                           Path(planned['repos']['current']))


def verify_resource_basis(planned):
    return historical.verify(planned['resource_basis_verifier'], Path(planned['resource_basis']),
                             Path(planned['repos']['current']))


def case_command(planned, stage, case):
    instrument, size, repeat, source = case
    root = Path(planned['job_root']) / paired.label(case)
    prefix = ['env', *(token for key in EXCLUDED_ENV for token in ('-u', key)), 'RUST_LOG=info,wgpu=error']
    if source == 'reference':
        argv = static.command(root, instrument, size, 15 if size == 'small' else 60, planned['adapter'])
        argv[argv.index('--repeat') + 1] = '1'
        argv.append('--building-static-single-leg')
    else:
        context = planned['release']['context']
        prefix.append('HW_BUILDING_ASSET_RELEASES=' + json.dumps(context['assets']['bindings']))
        argv = m6.command(root, (instrument, size, repeat, 'candidate'), planned['adapter'])
    return [*prefix, *argv, '--skip-build', '--binary', stage['binaries'][source]['path']]


def logical_digest(sidecar, size):
    """Only called after the complete source-specific raw oracle has passed."""
    copies = {'small': 4, 'medium': 16}[size]
    expected = logical_oracle.expected_records(copies)
    initial = sidecar['initial']
    rows = initial['records']
    pipeline.require(len(rows) == len(expected)
        and initial['target_count'] == copies * 9 and initial['souls'] == copies // 4 * 15
        and initial['completion_effects'] == 0, 'logical population/fixture differs')
    for row, logical in zip(rows, expected, strict=True):
        pipeline.require(pipeline.exact_json({key: row[key] for key in logical}, logical), 'cross-source logical record differs')
    return pipeline.digest(pipeline.canonical({'records': expected, 'target_count': copies * 9,
        'souls': copies // 4 * 15, 'completion_effects': 0}))


def case_artifacts(planned, stage, case, parent):
    instrument, size, repeat, source = case
    case_id = static.Case('building-art-static', size, 'gpu', static.SEED, 15 if size == 'small' else 60, 0).identifier
    if source == 'reference':
        checked = static.verify_session(parent, instrument, size, 15 if size == 'small' else 60,
                                       planned['identities'][source], planned['adapter'], repeats=1)
        session = parent / f'{instrument}-{size}'
    else:
        frozen = planned['release']['context']
        context = paired._operation_context(planned, frozen)
        scoped = context is not None
        if not scoped:
            context = m6.released_case_context(Path(frozen['repo']), Path(frozen['codec']), frozen['adapter'],
                                               expected_codec_sha256=frozen['codec_sha256'])
        pipeline.require(pipeline.exact_json(context_value(context), frozen), 'release-only case context differs')
        item = (instrument, size, repeat, 'candidate')
        session = parent / m6.label(item)
        verifier = m6._verify_released_case_artifacts if scoped else m6.verify_released_case
        row = verifier(context, item, session, stage['binaries'][source]['sha256'])
        checked = {'binary_sha256': row['binary_sha256'], 'session_sha256': row['session_sha256'],
                   'observations': [{'adapter': row['adapter'], 'metrics': row['metrics']}]}
    run = session / 'cases' / case_id / 'run-001'
    requested = native.read_json(run / 'requested-environment.json')
    # fixed_environment records the M6 CLI mode, not inherited release env.
    # The enclosing engine checks exact ledger command (including env -u and
    # the full explicit ReleaseApproved payload); the complete release oracle
    # separately binds all eight observed pool/part identities. Do not invent a
    # requested-environment field or alter the old measurement harness.
    pipeline.require(requested.get('RUST_LOG', 'info,wgpu=error') == 'info,wgpu=error',
                     'cumulative renderer logging differs')
    if source == 'current':
        pipeline.require(requested.get('HW_BUILDING_M6_MODE') == 'candidate'
            and 'HW_BUILDING_M6_CANDIDATES' not in requested
            and 'HW_BUILDING_GROUP_COMPARISON' not in requested,
            'released case mode or specialized view differs')
    else:
        pipeline.require(all(key not in requested for key in (
            'HW_BUILDING_M6_MODE', 'HW_BUILDING_M6_CANDIDATES', 'HW_BUILDING_GROUP_COMPARISON')),
            'plain reference acquired a specialized view')
    sidecar = native.read_json(run / 'data/building_art_static.json')
    checked['presentation_sha256'] = sidecar['layout_sha256']
    checked['observations'][0]['layout_sha256'] = logical_digest(sidecar, size)
    raw = run / 'data/building_resource_inventory.json'
    observed = paired.inventory.boundaries(native.read_json(raw))
    return checked, raw, observed


if __name__ == '__main__':
    paired.main(profile=PROFILE)
