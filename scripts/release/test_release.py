"""Behavior tests use real staged bytes and a stateful, offline GitHub boundary."""
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release

COMMIT = 'a' * 40
TAG = 'v1.11.0'
IMAGE_DIGEST = 'sha256:' + 'b' * 64


def valid_policy():
    return ({'enabled': True}, [{
        'target': 'tag', 'enforcement': 'active', 'bypass_actors': [],
        'conditions': {'ref_name': {'include': ['refs/tags/v*'], 'exclude': []}},
        'rules': [{'type': 'update'}, {'type': 'deletion'}],
    }], {
        'name': 'release', 'can_admins_bypass': False,
        'protection_rules': [{'type': 'required_reviewers', 'prevent_self_review': True,
                              'reviewers': [{'type': 'Team', 'reviewer': {'id': 1}}]}],
        'deployment_branch_policy': {'protected_branches': False, 'custom_branch_policies': True},
    }, [{'name': 'v*', 'type': 'tag'}])


class PolicyTests(unittest.TestCase):
    def test_built_binary_glibc_requirements_are_bounded(self):
        for version, accepted in [('2.17', True), ('2.35', True), ('2.36', False), ('3.0', False)]:
            with self.subTest(version=version), patch.object(
                    release.subprocess, 'check_output', return_value='Name: GLIBC_' + version):
                if accepted:
                    release.check_abi(Path('/unused'))
                else:
                    with self.assertRaisesRegex(ValueError, 'GLIBC exceeds'):
                        release.check_abi(Path('/unused'))

    def test_unknown_binary_abi_fails_closed(self):
        with patch.object(release.subprocess, 'check_output', return_value='no version information'):
            with self.assertRaisesRegex(ValueError, 'cannot establish'):
                release.check_abi(Path('/unused'))

    def test_accepts_complete_policy(self):
        release.check_policy(*valid_policy())

    def test_hidden_bypass_list_reports_permission_boundary_without_accepting_policy(self):
        policy = valid_policy()
        policy[1][0].pop('bypass_actors')
        with self.assertRaisesRegex(ValueError, 'Administration read is insufficient'):
            release.check_policy(*policy)

    def test_each_missing_boundary_fails(self):
        mutations = [
            lambda p: p[0].update(enabled=False),
            lambda p: p[0].clear(),
            lambda p: p[1].clear(),
            lambda p: p[1][0].update(enforcement='evaluate'),
            lambda p: p[1][0].update(target='branch'),
            lambda p: p[1][0].update(bypass_actors=[{'actor_type': 'RepositoryRole', 'actor_id': 5}]),
            lambda p: p[1][0].pop('bypass_actors'),
            lambda p: p[1][0]['conditions']['ref_name'].update(exclude=['refs/tags/v1*']),
            lambda p: p[1][0]['conditions']['ref_name'].update(include=['refs/tags/v2*']),
            lambda p: p[1][0]['rules'].pop(),
            lambda p: p[2].update(can_admins_bypass=True),
            lambda p: p[2].pop('can_admins_bypass'),
            lambda p: p[2].update(name='default'),
            lambda p: p[2]['protection_rules'].clear(),
            lambda p: p[2]['protection_rules'][0].update(prevent_self_review=False),
            lambda p: p[2]['protection_rules'][0].update(reviewers=[]),
            lambda p: p[2].update(deployment_branch_policy=None),
            lambda p: p[3].append({'name': '*', 'type': 'branch'}),
            lambda p: p[3].clear(),
            lambda p: p[3][0].update(name='*'),
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(boundary=index):
                policy = deepcopy(valid_policy())
                mutate(policy)
                with self.assertRaises(ValueError):
                    release.check_policy(*policy)

    def test_audit_refuses_partial_policy_response(self):
        with patch.object(release, 'pages', return_value=[]), patch.object(
                release, 'api', return_value={'total_count': 31, 'branch_policies': []}):
            with self.assertRaisesRegex(ValueError, 'incomplete'):
                release.audit_policy()

    def test_audit_propagates_api_permission_failure(self):
        with patch.object(release, 'pages', side_effect=subprocess.CalledProcessError(1, 'gh')):
            with self.assertRaises(subprocess.CalledProcessError):
                release.audit_policy()

    def test_full_tags_and_commits_only(self):
        for tag in ('v1.2.3', 'v1.2.3-rc.1', 'v1.2.3-hotfix1'):
            release.validate_identity(tag, COMMIT)
        for tag in ('v1.2.3/other', 'v1.2.3 trailing', 'v1.2', 'main', 'v1.2.3\n'):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_identity(tag, COMMIT)
        for commit in ('a' * 39, 'A' * 40, 'main', ''):
            with self.assertRaises(ValueError):
                release.validate_identity(TAG, commit)


class GitHubFixture:
    """Mimic remote release state, complete downloads and signed proof rejection."""
    def __init__(self, assets, tag=TAG):
        self.assets = assets
        self.tag = tag
        self.calls = []
        self.exists = False
        self.uploaded = False
        self.published = False
        self.corrupt_download = False
        self.extra_download = False
        self.upload_fails = False
        self.signature_fails = False
        self.immutable = True

    def __call__(self, *args):
        self.calls.append(args)
        if args[:3] == ('api', '--paginate', '--slurp'):
            return json.dumps([[{'tag_name': self.tag, 'draft': True}] if self.exists else []])
        if args[:2] == ('attestation', 'verify'):
            for flag, value in [('--repo', release.REPOSITORY), ('--signer-workflow', release.WORKFLOW),
                                ('--signer-digest', COMMIT), ('--source-ref', 'refs/tags/' + self.tag),
                                ('--source-digest', COMMIT)]:
                assert args[args.index(flag) + 1] == value
            assert '--deny-self-hosted-runners' in args
            if self.signature_fails:
                raise subprocess.CalledProcessError(1, 'gh attestation verify')
            return '{}'
        if args[:2] == ('release', 'create'):
            assert not self.exists
            assert '--draft' in args and '--verify-tag' in args
            assert ('--prerelease' in args) == ('-' in self.tag)
            self.exists = True
            return 'draft-url'
        if args[:2] == ('release', 'upload'):
            assert self.exists and not self.published
            assert '--clobber' not in args
            if self.upload_fails:
                raise subprocess.CalledProcessError(1, 'gh release upload')
            self.uploaded = True
            return ''
        if args[:2] == ('release', 'download'):
            assert self.uploaded and not self.published
            target = Path(args[args.index('--dir') + 1])
            for path in self.assets.iterdir():
                shutil.copyfile(path, target / path.name)
            if self.corrupt_download:
                (target / 'gravity_node').write_bytes(b'tampered')
            if self.extra_download:
                (target / 'unexpected').write_text('extra')
            return ''
        if args[:2] == ('release', 'edit'):
            assert self.uploaded and not self.published
            self.published = True
            return ''
        if args[0] == 'api' and '/releases/tags/' in args[1]:
            return json.dumps({'draft': False, 'immutable': self.immutable, 'prerelease': '-' in self.tag})
        raise AssertionError(args)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        (self.directory / 'gravity_node').write_bytes(b'node production bytes\x00\xff' * 100)
        (self.directory / 'gravity_cli').write_bytes(b'cli production bytes\x00\xfe' * 100)
        release.make_manifest(self.directory, TAG, COMMIT, IMAGE_DIGEST, 123, 1)
        for bundle in ('binary-provenance.jsonl', 'manifest-provenance.jsonl', 'image-provenance.jsonl'):
            (self.directory / bundle).write_text('{"test_bundle":true}\n')
        self.remote = GitHubFixture(self.directory)
        self.env = patch.dict(os.environ, {'GH_TOKEN': 'test-publisher', 'RELEASE_POLICY_TOKEN': 'test-policy'})
        self.env.start()
        self.addCleanup(self.env.stop)

    def publish(self):
        with patch.object(release, 'gh', side_effect=self.remote), patch.object(release, 'audit_policy') as audit:
            release.publish(self.directory, TAG, COMMIT)
            audit.assert_called_once()
        self.assertEqual(os.environ['GH_TOKEN'], 'test-publisher')

    def test_manifest_hashes_actual_bytes_and_binds_release(self):
        manifest = json.loads((self.directory / 'release-manifest.json').read_text())
        self.assertEqual(manifest['source_commit'], COMMIT)
        self.assertEqual(manifest['tag'], TAG)
        self.assertEqual(manifest['workflow'], release.WORKFLOW)
        self.assertEqual(manifest['image']['digest'], IMAGE_DIGEST)
        for binary in release.BINARIES:
            expected = hashlib.sha256((self.directory / binary).read_bytes()).hexdigest()
            self.assertEqual(manifest['artifacts'][binary]['sha256'], expected)

    def test_publishes_only_after_full_download_and_all_four_proofs(self):
        self.publish()
        self.assertTrue(self.remote.published)
        self.assertEqual(sum(call[:2] == ('attestation', 'verify') for call in self.remote.calls), 4)
        self.assertEqual([call[:2] for call in self.remote.calls if call[0] == 'release'],
                         [('release', 'create'), ('release', 'upload'),
                          ('release', 'download'), ('release', 'edit')])

    def test_prerelease_suffix_is_preserved_during_publication(self):
        candidate = 'v1.11.0-rc.1'
        release.make_manifest(self.directory, candidate, COMMIT, IMAGE_DIGEST, 123, 1)
        self.remote = GitHubFixture(self.directory, candidate)
        with patch.object(release, 'gh', side_effect=self.remote), patch.object(release, 'audit_policy'):
            release.publish(self.directory, candidate, COMMIT)
        self.assertTrue(self.remote.published)
        create = next(call for call in self.remote.calls if call[:2] == ('release', 'create'))
        self.assertIn('--prerelease', create)

    def test_existing_draft_or_release_cannot_be_modified(self):
        self.remote.exists = True
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.publish()
        self.assertFalse(any(call[0] == 'release' for call in self.remote.calls))

    def test_rerun_cannot_rewrite_successful_release(self):
        self.publish()
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.publish()
        self.assertEqual(sum(call[:2] == ('release', 'upload') for call in self.remote.calls), 1)

    def test_tampered_upload_stays_draft(self):
        self.remote.corrupt_download = True
        with self.assertRaisesRegex(ValueError, 'uploaded bytes differ'):
            self.publish()
        self.assertTrue(self.remote.exists)
        self.assertFalse(self.remote.published)

    def test_unexpected_remote_asset_stays_draft(self):
        self.remote.extra_download = True
        with self.assertRaisesRegex(ValueError, 'inventory mismatch'):
            self.publish()
        self.assertFalse(self.remote.published)

    def test_failed_upload_cannot_publish_and_rerun_refuses_draft(self):
        self.remote.upload_fails = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.publish()
        self.assertFalse(self.remote.published)
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.publish()

    def test_invalid_signature_never_creates_release(self):
        self.remote.signature_fails = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.publish()
        self.assertFalse(self.remote.exists)

    def test_changed_local_bytes_or_missing_proof_never_create_release(self):
        (self.directory / 'gravity_node').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            self.publish()
        self.assertFalse(self.remote.exists)
        (self.directory / 'image-provenance.jsonl').unlink()
        with self.assertRaisesRegex(ValueError, 'inventory'):
            self.publish()

    def test_wrong_source_tag_or_workflow_never_create_release(self):
        path = self.directory / 'release-manifest.json'
        original = json.loads(path.read_text())
        for field, value in [('source_commit', 'c' * 40), ('tag', 'v0.0.1'),
                             ('repository', 'attacker/repo'), ('workflow', 'attacker/repo/build.yml')]:
            manifest = dict(original, **{field: value})
            path.write_text(json.dumps(manifest))
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'source identity'):
                self.publish()
        self.assertFalse(self.remote.exists)

    def test_linked_asset_rejected(self):
        (self.directory / 'gravity_cli').unlink()
        (self.directory / 'gravity_cli').symlink_to(self.directory / 'gravity_node')
        with self.assertRaisesRegex(ValueError, 'linked asset'):
            self.publish()

    def test_missing_live_policy_never_creates_release_and_restores_token(self):
        with patch.object(release, 'gh', side_effect=self.remote), patch.object(
                release, 'audit_policy', side_effect=ValueError('policy missing')):
            with self.assertRaisesRegex(ValueError, 'policy missing'):
                release.publish(self.directory, TAG, COMMIT)
        self.assertEqual(os.environ['GH_TOKEN'], 'test-publisher')
        self.assertFalse(self.remote.calls)

    def test_policy_reader_is_separate_from_publication_identity(self):
        def audit():
            self.assertEqual(os.environ['GH_TOKEN'], 'test-policy')
        def remote(*args):
            self.assertEqual(os.environ['GH_TOKEN'], 'test-publisher')
            return self.remote(*args)
        with patch.object(release, 'gh', side_effect=remote), patch.object(release, 'audit_policy', side_effect=audit):
            release.publish(self.directory, TAG, COMMIT)

    def test_nonimmutable_publication_is_reported_as_failure(self):
        self.remote.immutable = False
        with self.assertRaisesRegex(ValueError, 'immutability not confirmed'):
            self.publish()

    def test_manifest_cli_runs_without_network(self):
        for bundle in ('binary-provenance.jsonl', 'manifest-provenance.jsonl', 'image-provenance.jsonl'):
            (self.directory / bundle).unlink()
        subprocess.run(['python3', str(Path(release.__file__)), 'manifest', '--directory', str(self.directory),
                        '--tag', TAG, '--commit', COMMIT, '--image-digest', IMAGE_DIGEST,
                        '--run-id', '999', '--run-attempt', '2'], check=True)
        manifest = json.loads((self.directory / 'release-manifest.json').read_text())
        self.assertEqual((manifest['run_id'], manifest['run_attempt']), ('999', '2'))


if __name__ == '__main__':
    unittest.main()
