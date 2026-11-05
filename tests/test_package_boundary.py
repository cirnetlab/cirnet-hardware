"""Negative installed-wheel probes reject source shadowing and damaged payloads."""
from importlib.metadata import distribution
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from urllib.parse import urlparse
from urllib.request import url2pathname

import pytest

ROOT = Path(__file__).resolve().parents[1]


def selected_wheel():
    direct = json.loads(distribution('cirnet-hardware-service').read_text('direct_url.json'))
    return Path(url2pathname(urlparse(direct['url']).path))


def probe(*paths, wheel=None):
    script = ROOT / 'scripts/check_installed.py'
    # Test-only injected paths deliberately exercise the rejection boundary.
    # Normal build/test commands never add product checkout paths.
    program = ('import sys,runpy; sys.path[:0]=sys.argv[1:-2]; '
        'script,wheel=sys.argv[-2:]; sys.argv=[script,"--wheel",wheel]; '
        'runpy.run_path(script,run_name="__main__")')
    return subprocess.run([sys.executable, '-I', '-B', '-c', program, *map(str, paths),
        str(ROOT / 'scripts'), str(script), str(wheel or selected_wheel())], capture_output=True, text=True)


def test_selected_installed_hardware_wheel_and_dependencies_have_provenance():
    result = probe()
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout)
    assert data['hardware']['version'] == '0.1.2'
    assert data['hardware']['owned_files'] == 9
    assert data['hardware']['selected_files'] == 14
    assert data['contracts']['name'] == 'cirnet-contracts'
    assert data['contracts']['version'] == '0.1.2'
    assert data['simulator']['version'] == '0.1.2'
    assert data['simulator']['owned_files'] == 10


def test_product_source_shadowing_is_rejected():
    result = probe(ROOT)
    assert result.returncode != 0
    assert 'shadowed by source' in result.stderr


def test_legacy_contracts_distribution_is_rejected(tmp_path):
    legacy = tmp_path / 'netcircuit_contracts-0.1.1.dist-info'
    legacy.mkdir()
    (legacy / 'METADATA').write_text('Name: netcircuit-contracts\nVersion: 0.1.1\n', encoding='utf-8')
    result = probe(tmp_path)
    assert result.returncode != 0
    assert 'shares the contracts import namespace' in result.stderr


@pytest.mark.parametrize('change', ['modified', 'unowned'])
def test_modified_installed_payload_is_rejected(tmp_path, change):
    artifact = distribution('cirnet-hardware-service')
    for item in artifact.files:
        relative = Path(str(item))
        if relative.parts[0] == 'hardware_service' or relative.parts[0].endswith('.dist-info'):
            if relative.suffix == '.pyc':
                continue
            destination = tmp_path / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(artifact.locate_file(item), destination)
    if change == 'modified':
        with (tmp_path / 'hardware_service/models.py').open('a', encoding='utf-8') as output:
            output.write('\n# corrupted probe payload\n')
    else:
        (tmp_path / 'hardware_service/unowned.py').write_text('# unowned runtime module\n')
    result = probe(tmp_path)
    message = 'differs from wheel RECORD' if change == 'modified' else 'unowned runtime files'
    assert result.returncode != 0 and message in result.stderr


def test_different_same_version_archive_is_rejected(tmp_path):
    from zipfile import ZipFile, ZIP_DEFLATED
    wheel = selected_wheel()
    candidate = tmp_path / wheel.name
    with ZipFile(wheel) as archive, ZipFile(candidate, 'w', ZIP_DEFLATED) as output:
        for name in archive.namelist():
            content = archive.read(name)
            if name.endswith('/METADATA'):
                content += b'\nDifferent same-version candidate.\n'
            output.writestr(name, content)
    result = probe(wheel=candidate)
    assert result.returncode != 0
    assert 'archive digest differs from the selected wheel' in result.stderr


def test_bootstrap_failure_publishes_owned_cleanup_before_pip(tmp_path):
    pin = json.loads((ROOT / 'integration/contracts-version.json').read_text())['dev']
    filename = f"{pin['name'].replace('-', '_')}-{pin['version']}-py3-none-any.whl"
    wheel = Path(os.environ['CIRNET_ARTIFACTS_SESSION']) / 'hw-leaf-dev-bootstrap' / filename
    assert wheel.is_file()
    env = {**os.environ, 'CIRNET_ARTIFACTS_DIR': str(tmp_path),
           'GITHUB_ENV': str(tmp_path / 'github-env'),
           'PIP_REQUIREMENT': str(tmp_path / 'intentionally-missing-requirements')}
    env.pop('CIRNET_ARTIFACTS_SESSION', None)
    env.pop('NETCIRCUIT_ARTIFACTS_SESSION', None)
    result = subprocess.run([sys.executable, str(ROOT / 'scripts/bootstrap_contract_tools.py'),
        '--wheel', str(wheel), '--sha256', pin['wheel_sha256']], env=env, capture_output=True, text=True)
    assert result.returncode != 0 and 'DEV wheel install failed' in result.stderr
    published = dict(line.split('=', 1) for line in (tmp_path / 'github-env').read_text().splitlines())
    session = Path(published['CIRNET_ARTIFACTS_SESSION'])
    try:
        cli = Path(published['CIRNET_ARTIFACTS_CLI'])
        assert cli.is_file() and cli.is_relative_to(session)
        assert (session / 'hw-leaf-dev-bootstrap/install.log').read_text()
        cleaned = subprocess.run(['node', str(cli), '--context', published['CIRNET_ARTIFACTS_CONTEXT'],
            '--cleanup', str(session)], capture_output=True, text=True)
        assert cleaned.returncode == 0, cleaned.stderr
    finally:
        try:
            from cirnet_artifact_paths import cleanup_artifact_session
        except ModuleNotFoundError:
            from netcircuit_artifact_paths import cleanup_artifact_session
        if session.exists():
            cleanup_artifact_session(session, context=json.loads(published['CIRNET_ARTIFACTS_CONTEXT']))
    assert not session.exists()
