"""Offline regressions for the exact credential-bearing workflow boundary.

Only the stdlib is needed. Execute the workflow's inline validator, not a copied
implementation, on bounded synthetic artifacts. No registry calls or builds run.
"""
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import tomllib
import unittest


ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = (ROOT / '.github/workflows/publish-crate.yml').read_text()
SHA = 'a' * 40
VERSION = '1.2.3'
PREFIX = 'denki-' + VERSION


def inline_script(step_name):
    section = WORKFLOW.split('      - name: ' + step_name + '\n', 1)[1]
    section = section.split("          python3 -I - <<'PYTHON'\n", 1)[1]
    return textwrap.dedent(section.split('          PYTHON\n', 1)[0])


def inline_validator():
    return inline_script('Unpack the verified crate as data')


def file_entry(name, data=b'example', kind=tarfile.REGTYPE, linkname=''):
    if isinstance(data, str):
        data = data.encode()
    member = tarfile.TarInfo(name)
    member.type = kind
    member.mode = 0o7777  # Extraction must not preserve executable/set-ID bits.
    member.linkname = linkname
    member.size = len(data) if kind == tarfile.REGTYPE else 0
    return member, data


def ordinary_entries():
    return [
        file_entry(PREFIX, kind=tarfile.DIRTYPE),
        file_entry(PREFIX + '/Cargo.toml', '[package]\nname="denki"\nversion="1.2.3"\n'),
        file_entry(PREFIX + '/Cargo.toml.orig', '[package]\nname="denki"\nversion="1.2.3"\n'),
        file_entry(PREFIX + '/Cargo.lock', 'version=4\n[[package]]\nname="denki"\nversion="1.2.3"\n'),
        file_entry(PREFIX + '/.cargo_vcs_info.json', json.dumps({'git': {'sha1': SHA}})),
        file_entry(PREFIX + '/src/lib.rs', 'pub fn example() {}\n'),
    ]


def make_archive(entries):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode='w:gz') as archive:
        for member, data in entries:
            archive.addfile(member, io.BytesIO(data) if member.isfile() else None)
    return output.getvalue()


class ArchiveBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.artifact = self.root / 'verified-crate'
        self.artifact.mkdir()
        self.entries = ordinary_entries()
        self.evidence = {
            'package': 'denki', 'version': VERSION, 'source_sha': SHA,
            'archive_sha256': '',
        }

    def run_validator(self, *, archive=None, metadata=None, prepare=None, **environment):
        archive = make_archive(self.entries) if archive is None else archive
        (self.artifact / (PREFIX + '.crate')).write_bytes(archive)
        self.evidence['archive_sha256'] = hashlib.sha256(archive).hexdigest()
        if metadata:
            self.evidence.update(metadata)
        (self.artifact / 'release-preflight.json').write_text(json.dumps(self.evidence))
        if prepare:
            prepare()
        env = dict(os.environ, RUNNER_TEMP=str(self.root), SOURCE_SHA=SHA, RELEASE_TAG='v' + VERSION)
        env.update(environment)
        return subprocess.run([sys.executable, '-I', '-c', inline_validator()],
                              env=env, cwd=self.root, capture_output=True, text=True, timeout=15)

    def reject(self, **kwargs):
        result = self.run_validator(**kwargs)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse((self.root / 'crate-publish').exists(), result.stderr)
        return result

    def replace(self, filename, contents):
        self.entries = [entry for entry in self.entries if entry[0].name != PREFIX + '/' + filename]
        self.entries.append(file_entry(PREFIX + '/' + filename, contents))

    def test_valid_package_is_extracted_without_executable_modes(self):
        result = self.run_validator()
        self.assertEqual(result.returncode, 0, result.stderr)
        destination = self.root / 'crate-publish'
        self.assertEqual((destination / 'manifest-path').read_text(),
                         str(destination / PREFIX / 'Cargo.toml'))
        self.assertEqual((destination / PREFIX / 'src/lib.rs').stat().st_mode & 0o7777, 0o600)
        self.assertFalse((destination / PREFIX / 'Cargo.toml.orig').exists())
        self.assertEqual((self.root / 'publish-cargo-home').stat().st_mode & 0o777, 0o700)
        self.assertEqual(list((self.root / 'publish-cargo-home').iterdir()), [])

    def test_package_scripts_and_config_are_data_not_executed(self):
        self.entries += [
            file_entry(PREFIX + '/build.rs', 'compile_error!("must never build");'),
            file_entry(PREFIX + '/.cargo/config.toml', '[build]\nrustc="/must/not/execute"\n'),
            file_entry(PREFIX + '/rust-toolchain.toml', '[toolchain]\nchannel="unavailable"\n'),
        ]
        result = self.run_validator()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / 'crate-publish/.cargo').exists())

    def test_non_normal_paths_are_rejected(self):
        for name in ('../escape', '/' + PREFIX + '/file', PREFIX + '/../escape',
                     PREFIX + '/./file', PREFIX + '//file', PREFIX + '/file/',
                     PREFIX + '/a\\b', PREFIX + '/line\nbreak', 'other-root/file',
                     PREFIX + '/a/../../escape', PREFIX + '/' + 'a' * 1025):
            with self.subTest(name=name):
                self.entries = ordinary_entries() + [file_entry(name)]
                self.reject()
        self.assertFalse((self.root / 'escape').exists())

    def test_links_devices_and_fifos_are_rejected(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.CHRTYPE, tarfile.BLKTYPE,
                     tarfile.FIFOTYPE, tarfile.CONTTYPE):
            with self.subTest(kind=kind):
                self.entries = ordinary_entries() + [file_entry(PREFIX + '/evil', kind=kind,
                                                               linkname='../outside')]
                self.reject()

    def test_duplicate_paths_are_rejected(self):
        self.entries.append(file_entry(PREFIX + '/src/lib.rs', 'replacement'))
        self.reject()

    def test_regular_file_cannot_replace_package_root(self):
        self.entries[0] = file_entry(PREFIX)
        self.reject()

    def test_file_directory_conflicts_are_rejected_in_either_order(self):
        for reverse in (False, True):
            with self.subTest(reverse=reverse):
                conflict = [file_entry(PREFIX + '/child'), file_entry(PREFIX + '/child/data')]
                self.entries = ordinary_entries() + (list(reversed(conflict)) if reverse else conflict)
                self.reject()

    def test_empty_archive_is_rejected(self):
        self.entries = []
        self.reject()

    def test_too_many_members_are_rejected(self):
        self.entries += [file_entry(PREFIX + '/files/' + str(i), b'') for i in range(4096)]
        self.reject()

    def test_oversized_member_is_rejected(self):
        self.entries.append(file_entry(PREFIX + '/large', b'x' * (8 * 1024 * 1024 + 1)))
        self.reject()

    def test_compressed_padding_bomb_is_rejected_before_tar_parsing(self):
        self.reject(archive=gzip.compress(b'\0' * (64 * 1024 * 1024 + 1)))

    def test_oversized_artifact_is_rejected(self):
        self.reject(archive=b'x' * (16 * 1024 * 1024 + 1))

    def test_malformed_gzip_and_tar_are_rejected(self):
        for archive in (b'not gzip', gzip.compress(b'not a tar'), make_archive(self.entries)[:-15]):
            with self.subTest(archive_length=len(archive)):
                self.reject(archive=archive)

    def test_digest_mismatch_is_rejected(self):
        self.reject(metadata={'archive_sha256': 'f' * 64})

    def test_metadata_identity_and_shape_are_checked(self):
        for metadata in ({'package': 'other'}, {'version': '../1.2.3'}, {'version': '01.2.3'},
                         {'version': '1.2.3-rc.1'}, {'source_sha': 'b' * 40},
                         {'source_sha': 'bad'}, {'version': None}, {'extra': 'unexpected'},
                         {'archive_sha256': 'invalid'}):
            with self.subTest(metadata=metadata):
                self.evidence = {'package': 'denki', 'version': VERSION, 'source_sha': SHA,
                                 'archive_sha256': ''}
                self.reject(metadata=metadata)

    def test_tag_mismatch_is_rejected(self):
        self.reject(RELEASE_TAG='v1.2.4')

    def test_source_environment_mismatch_is_rejected(self):
        self.reject(SOURCE_SHA='b' * 40)

    def test_oversized_and_duplicate_metadata_are_rejected(self):
        for data in (' ' * 4097, '{"package":"denki","package":"denki"}'):
            with self.subTest(data_length=len(data)):
                self.reject(prepare=lambda: (self.artifact / 'release-preflight.json').write_text(data))

    def test_unexpected_artifact_entries_are_rejected(self):
        self.reject(prepare=lambda: (self.artifact / 'extra.crate').write_bytes(b'extra'))

    def test_symlink_artifact_files_are_rejected(self):
        for name in (PREFIX + '.crate', 'release-preflight.json'):
            with self.subTest(name=name):
                def prepare():
                    target = self.artifact / name
                    outside = self.root / ('original-' + name)
                    target.rename(outside)
                    target.symlink_to(outside)
                self.reject(prepare=prepare)
                (self.artifact / name).unlink()

    def test_symlink_artifact_directory_is_rejected(self):
        def prepare():
            destination = self.root / 'moved-artifact'
            self.artifact.rename(destination)
            self.artifact.symlink_to(destination, target_is_directory=True)
        self.reject(prepare=prepare)

    def test_manifest_mismatch_is_rejected(self):
        self.replace('Cargo.toml', '[package]\nname="other"\nversion="1.2.3"\n')
        self.reject()

    def test_missing_manifest_is_rejected(self):
        self.entries = [entry for entry in self.entries if entry[0].name != PREFIX + '/Cargo.toml']
        self.reject()

    def test_lock_version_mismatch_and_registry_substitution_are_rejected(self):
        for lock in ('version=4\n[[package]]\nname="denki"\nversion="1.2.4"\n',
                     'version=4\n[[package]]\nname="denki"\nversion="1.2.3"\nsource="registry+example"\n',
                     'version=4\n' + '[[package]]\nname="denki"\nversion="1.2.3"\n' * 2):
            with self.subTest(lock=lock):
                self.replace('Cargo.lock', lock)
                self.reject()

    def test_vcs_mismatch_and_dirty_sources_are_rejected(self):
        for git in ({'sha1': 'b' * 40}, {'sha1': SHA, 'dirty': True},
                    {'sha1': SHA, 'dirty': 'false'}):
            with self.subTest(git=git):
                self.replace('.cargo_vcs_info.json', json.dumps({'git': git}))
                self.reject()

    def test_invalid_utf8_or_toml_is_rejected(self):
        for data in (b'\xff', b'[package'):
            with self.subTest(data=data):
                self.replace('Cargo.toml', data)
                self.reject()


class RecordingTests(unittest.TestCase):
    def test_record_uses_the_publish_dry_run_artifact_not_stale_package_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / 'runner-temp'
            output.mkdir()
            archive_dir = root / 'target/package/tmp-registry'
            archive_dir.mkdir(parents=True)
            archive = make_archive(ordinary_entries())
            (archive_dir / (PREFIX + '.crate')).write_bytes(archive)
            (archive_dir.parent / (PREFIX + '.crate')).write_bytes(b'stale package output')
            (root / 'Cargo.toml').write_text('[package]\nname="denki"\nversion="1.2.3"\n')
            result = subprocess.run([sys.executable, '-I', '-c', inline_script('Record the verified package')],
                                    cwd=root, env=dict(os.environ, RUNNER_TEMP=str(output),
                                                       SOURCE_SHA=SHA, RELEASE_TAG='v' + VERSION),
                                    capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            retained = output / 'verified-crate'
            self.assertEqual((retained / (PREFIX + '.crate')).read_bytes(), archive)
            evidence = json.loads((retained / 'release-preflight.json').read_text())
            self.assertEqual(evidence, dict(package='denki', version=VERSION, source_sha=SHA,
                                           archive_sha256=hashlib.sha256(archive).hexdigest()))


class PublishingBoundaryTests(unittest.TestCase):
    def test_oidc_only_exists_in_fresh_publisher(self):
        verify, publish = WORKFLOW.split('  publish:\n', 1)
        self.assertNotIn('id-token:', verify)
        self.assertEqual(WORKFLOW.count('id-token: write'), 1)
        self.assertIn('    needs: verify\n', publish)
        self.assertNotIn('actions/checkout@', publish)
        self.assertNotIn('uses: ./', publish)
        self.assertNotIn('rust-cache', publish)
        self.assertNotIn('scripts/', publish)
        self.assertIn('          persist-credentials: false\n', verify)
        self.assertIn('python3 scripts/test_publish_workflow.py -v', verify)

    def test_publisher_only_runs_pinned_actions_validator_and_no_build_publish(self):
        publish = WORKFLOW.split('  publish:\n', 1)[1]
        uses = re.findall(r'^\s+uses: (.+)$|^\s+- uses: (.+)$', publish, re.MULTILINE)
        self.assertEqual([a or b for a, b in uses], [
            'dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master',
            'actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2',
            'rust-lang/crates-io-auth-action@3268eb413133e2960fa4ae0e963ca76380c54cd9 # v1.0.6',
        ])
        runs = re.findall(r'^        run: (.+)$', publish, re.MULTILINE)
        self.assertEqual(runs, ['|', 'cargo +1.99.1 publish --no-verify --locked --registry crates-io '
                               '--manifest-path "$(cat manifest-path)"'])
        self.assertIn('        working-directory: ${{ runner.temp }}/crate-publish\n', publish)
        self.assertIn('          CARGO_HOME: ${{ runner.temp }}/publish-cargo-home\n', publish)
        self.assertIn('          toolchain: 1.99.1\n', publish)
        self.assertLess(publish.index('Unpack the verified crate as data'),
                        publish.index('Authenticate with crates.io'))
        self.assertNotIn('${{', inline_validator())  # No direct event interpolation into Python.
        self.assertNotIn('assert ', inline_validator())  # -O must not disable validation.

    def test_artifact_is_bound_to_the_verification_job(self):
        self.assertIn('artifact_id: ${{ steps.artifact.outputs.artifact-id }}', WORKFLOW)
        self.assertIn('artifact-ids: ${{ needs.verify.outputs.artifact_id }}', WORKFLOW)
        self.assertIn('SOURCE_SHA: ${{ needs.verify.outputs.source_sha }}', WORKFLOW)
        self.assertIn('actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2', WORKFLOW)
        self.assertIn('cargo publish --locked --dry-run --registry crates-io', WORKFLOW.split('  publish:\n', 1)[0])
        self.assertIn('          cargo audit\n', WORKFLOW.split('  publish:\n', 1)[0])

    def test_regular_ci_runs_the_boundary_tests(self):
        self.assertIn('python3 scripts/test_publish_workflow.py -v',
                      (ROOT / '.github/workflows/ci.yml').read_text())
        self.assertIn('cargo publish --locked --dry-run --registry crates-io\n          python3 scripts/test_publish_workflow.py --real-preflight',
                      (ROOT / '.github/workflows/ci.yml').read_text())


def verify_real_preflight():
    """Exercise the recorder and validator on the actual clean Cargo artifact."""
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    with tempfile.TemporaryDirectory(prefix='denki-preflight-') as directory:
        env = dict(os.environ, RUNNER_TEMP=directory, SOURCE_SHA=sha, RELEASE_TAG='v' + version)
        for name in ['Record the verified package', 'Unpack the verified crate as data']:
            subprocess.run([sys.executable, '-I', '-c', inline_script(name)], cwd=ROOT,
                           env=env, check=True, timeout=30)
    print('Actual clean Cargo preflight artifact passed recorder and publisher validation')


if __name__ == '__main__':
    if sys.argv[1:] == ['--real-preflight']:
        verify_real_preflight()
    else:
        unittest.main()
