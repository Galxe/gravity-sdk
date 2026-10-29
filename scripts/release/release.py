#!/usr/bin/env python3
"""Fail-closed release controls. All network operations go through authenticated gh."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

REPOSITORY = 'Galxe/gravity-sdk'
WORKFLOW = REPOSITORY + '/.github/workflows/release.yml'
BINARIES = ('gravity_node', 'gravity_cli')
ASSETS = (*BINARIES, *(name + '.sha256' for name in BINARIES),
          'release-manifest.json', 'binary-provenance.jsonl',
          'manifest-provenance.jsonl', 'image-provenance.jsonl')
TAG = re.compile(r'v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?')
SHA = re.compile(r'[0-9a-f]{40}')
DIGEST = re.compile(r'sha256:[0-9a-f]{64}')
MAX_GLIBC = (2, 35)  # Preserve the selected Ubuntu 22.04 build ABI ceiling.


def require(condition, message):
    if not condition:
        raise ValueError(message)


def gh(*args):
    return subprocess.check_output(['gh', *args], text=True).strip()


def api(path):
    return json.loads(gh('api', path))


def pages(path):
    return [item for page in json.loads(gh('api', '--paginate', '--slurp', path)) for item in page]


def validate_identity(tag, commit):
    require(TAG.fullmatch(tag), 'invalid release tag')
    require(SHA.fullmatch(commit), 'source commit must be a lowercase full SHA')


def check_abi(directory):
    """Inspect required ELF symbol versions without executing build outputs."""
    for binary in BINARIES:
        output = subprocess.check_output(['readelf', '--version-info', str(directory / binary)], text=True)
        versions = [tuple(map(int, value.split('.'))) for value in re.findall(r'\bGLIBC_([0-9]+\.[0-9]+(?:\.[0-9]+)?)\b', output)]
        require(versions, 'cannot establish required GLIBC ABI: ' + binary)
        require(max(versions) <= MAX_GLIBC, 'required GLIBC exceeds 2.35: ' + binary)


def check_policy(immutable, rulesets, environment, deployment_policies):
    """Accept a deliberately narrow, auditable policy shape; unknown shapes fail."""
    require(immutable.get('enabled') is True, 'immutable releases must be enabled')
    protected = False
    for rule in rulesets:
        refs = rule.get('conditions', {}).get('ref_name', {})
        if (rule.get('target') == 'tag' and rule.get('enforcement') == 'active'
                and rule.get('bypass_actors') == [] and refs.get('exclude') == []
                and 'refs/tags/v*' in refs.get('include', [])):
            types = {item.get('type') for item in rule.get('rules', [])}
            if {'update', 'deletion'} <= types:
                protected = True
    require(protected, 'active non-bypass update/deletion tag rules for refs/tags/v* required')
    require(environment.get('name') == 'release', 'release environment required')
    require(environment.get('can_admins_bypass') is False, 'environment administrator bypass must be disabled')
    reviewers = [rule for rule in environment.get('protection_rules', [])
                 if rule.get('type') == 'required_reviewers']
    require(any(rule.get('reviewers') and rule.get('prevent_self_review') is True
                for rule in reviewers), 'independent required release reviewer required')
    require(environment.get('deployment_branch_policy') == {
        'protected_branches': False, 'custom_branch_policies': True},
        'release environment must use explicit tag restrictions')
    require(deployment_policies and all(p.get('type') == 'tag' and p.get('name') == 'v*'
                                       for p in deployment_policies),
            'release environment must allow only v* tags')


def audit_policy():
    prefix = 'repos/' + REPOSITORY
    summaries = pages(prefix + '/rulesets?includes_parents=true&per_page=100')
    rulesets = [api(prefix + '/rulesets/' + str(rule['id'])) for rule in summaries]
    policies = api(prefix + '/environments/release/deployment-branch-policies')
    # Deliberately reject truncated/malformed results rather than miss a broad policy.
    require(policies.get('total_count') == len(policies.get('branch_policies', [])),
            'incomplete deployment policy response')
    check_policy(api(prefix + '/immutable-releases'), rulesets,
                 api(prefix + '/environments/release'), policies['branch_policies'])


def assert_new(tag):
    releases = pages('repos/' + REPOSITORY + '/releases?per_page=100')
    require(not any(release.get('tag_name') == tag for release in releases),
            'release already exists (including drafts); choose a new version or investigate the failed draft')


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def make_manifest(directory, tag, commit, image_digest, run_id, run_attempt):
    validate_identity(tag, commit)
    require(DIGEST.fullmatch(image_digest), 'invalid OCI digest')
    require(str(run_id).isdigit() and str(run_attempt).isdigit(), 'invalid workflow run identity')
    artifacts = {}
    for binary in BINARIES:
        path = directory / binary
        require(path.is_file() and path.stat().st_size > 0 and not path.is_symlink(),
                'missing or empty binary: ' + binary)
        digest = sha256(path)
        artifacts[binary] = {'sha256': digest}
        (directory / (binary + '.sha256')).write_text(f'{digest}  {binary}\n')
    manifest = {'schema_version': 1, 'repository': REPOSITORY, 'tag': tag,
                'source_commit': commit, 'workflow': WORKFLOW,
                'run_id': str(run_id), 'run_attempt': str(run_attempt),
                'artifacts': artifacts,
                'image': {'name': 'ghcr.io/galxe/gravity_node', 'digest': image_digest}}
    (directory / 'release-manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    return manifest


def verify_assets(directory, tag, commit):
    validate_identity(tag, commit)
    require({p.name for p in directory.iterdir()} == set(ASSETS), 'release asset inventory is incomplete or unexpected')
    for name in ASSETS:
        path = directory / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size > 0,
                'missing, empty or linked asset: ' + name)
    manifest = json.loads((directory / 'release-manifest.json').read_text())
    require(manifest.get('schema_version') == 1 and manifest.get('repository') == REPOSITORY
            and manifest.get('workflow') == WORKFLOW and manifest.get('tag') == tag
            and manifest.get('source_commit') == commit, 'manifest source identity mismatch')
    require(manifest.get('image', {}).get('name') == 'ghcr.io/galxe/gravity_node'
            and DIGEST.fullmatch(manifest['image'].get('digest', '')), 'invalid manifest image')
    for binary in BINARIES:
        digest = sha256(directory / binary)
        require(manifest.get('artifacts', {}).get(binary, {}).get('sha256') == digest,
                'manifest digest mismatch: ' + binary)
        require((directory / (binary + '.sha256')).read_text() == f'{digest}  {binary}\n',
                'checksum sidecar mismatch: ' + binary)
    policy = ['--repo', REPOSITORY, '--signer-workflow', WORKFLOW,
              '--signer-digest', commit, '--source-ref', 'refs/tags/' + tag,
              '--source-digest', commit, '--deny-self-hosted-runners']
    for subject, bundle in [*( (str(directory / binary), 'binary-provenance.jsonl') for binary in BINARIES),
                            (str(directory / 'release-manifest.json'), 'manifest-provenance.jsonl'),
                            ('oci://' + manifest['image']['name'] + '@' + manifest['image']['digest'],
                             'image-provenance.jsonl')]:
        gh('attestation', 'verify', subject, '--bundle', str(directory / bundle), *policy)


def publish(directory, tag, commit):
    # Policy read uses a short-lived, administration-read App token, while all
    # writes and artifact verification use the ordinary job GITHUB_TOKEN.
    publication_token = os.environ.get('GH_TOKEN')
    policy_token = os.environ.get('RELEASE_POLICY_TOKEN')
    require(publication_token and policy_token, 'publication and policy-read identities required')
    try:
        os.environ['GH_TOKEN'] = policy_token
        audit_policy()
    finally:
        os.environ['GH_TOKEN'] = publication_token
    assert_new(tag)
    verify_assets(directory, tag, commit)
    classification = ['--prerelease'] if '-' in tag else []
    gh('release', 'create', tag, '--repo', REPOSITORY, '--verify-tag', '--target', commit,
       *classification, '--draft', '--title', tag, '--notes',
       'Built from ' + commit + '. See signed release-manifest.json for artifact and OCI digests.')
    # No upload overwrite option exists in this path. Any interruption leaves a
    # draft for operator investigation; reruns cannot rewrite it.
    gh('release', 'upload', tag, '--repo', REPOSITORY, *(str(directory / name) for name in ASSETS))
    with tempfile.TemporaryDirectory(prefix='release-check-') as temporary:
        gh('release', 'download', tag, '--repo', REPOSITORY, '--dir', temporary)
        downloaded = Path(temporary)
        require({p.name for p in downloaded.iterdir()} == set(ASSETS), 'remote asset inventory mismatch')
        for name in ASSETS:
            require(sha256(downloaded / name) == sha256(directory / name),
                    'uploaded bytes differ: ' + name)
    gh('release', 'edit', tag, '--repo', REPOSITORY, '--draft=false')
    release = api('repos/' + REPOSITORY + '/releases/tags/' + tag)
    require(release.get('immutable') is True and release.get('draft') is False,
            'published release immutability not confirmed; stop and alert the release owner')
    require(release.get('prerelease') is ('-' in tag), 'published release classification mismatch')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('audit-policy')
    abi = sub.add_parser('check-abi')
    abi.add_argument('--directory', required=True, type=Path)
    check = sub.add_parser('assert-new')
    check.add_argument('--tag', required=True)
    for command in ('manifest', 'publish'):
        cli = sub.add_parser(command)
        cli.add_argument('--directory', required=True, type=Path)
        cli.add_argument('--tag', required=True)
        cli.add_argument('--commit', required=True)
        if command == 'manifest':
            cli.add_argument('--image-digest', required=True)
            cli.add_argument('--run-id', required=True)
            cli.add_argument('--run-attempt', required=True)
    args = parser.parse_args()
    if args.command == 'audit-policy':
        audit_policy()
    elif args.command == 'check-abi':
        check_abi(args.directory)
    elif args.command == 'assert-new':
        require(TAG.fullmatch(args.tag), 'invalid release tag')
        assert_new(args.tag)
    elif args.command == 'manifest':
        make_manifest(args.directory, args.tag, args.commit, args.image_digest, args.run_id, args.run_attempt)
    else:
        publish(args.directory, args.tag, args.commit)


if __name__ == '__main__':
    main()
