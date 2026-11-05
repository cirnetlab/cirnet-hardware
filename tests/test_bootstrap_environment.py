"""Bootstrap subprocesses contain temporary files before pip can create output."""
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def test_ci_selects_node_26_before_bootstrapping_allocator():
    workflow = (ROOT / '.github/workflows/hardware-service.yml').read_text(encoding='utf-8')
    steps = re.split(r'^      - ', workflow, flags=re.MULTILINE)[1:]
    node = [index for index, step in enumerate(steps) if step.startswith('uses: actions/setup-node@')]
    bootstrap = next(index for index, step in enumerate(steps)
                     if 'run: python scripts/bootstrap_contract_tools.py' in step)
    assert len(node) == 1 and node[0] < bootstrap, 'select Node before the first allocator call'
    assert re.search(r"^          node-version: ['\"]26['\"]$", steps[node[0]], re.MULTILINE)


def test_bootstrap_contains_actual_pip_child_temporary_output(tmp_path, monkeypatch):
    monkeypatch.syspath_prepend(str(ROOT/'scripts'))
    spec = importlib.util.spec_from_file_location('contained_bootstrap', ROOT/'scripts/bootstrap_contract_tools.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    session = tmp_path/'allocated'
    session.mkdir()
    lock_root = tmp_path/'input'
    (lock_root/'integration').mkdir(parents=True)
    node = b'canonical mock verified by independent integrity tests'
    import hashlib
    (lock_root/'integration/contracts-version.json').write_text(json.dumps({'dev':{
        'wheel_sha256':'a'*64, 'node_sha256':hashlib.sha256(node).hexdigest()}}))
    wheel = tmp_path/'input.whl'
    wheel.write_bytes(b'mocked verified wheel')
    monkeypatch.setattr(module, 'ROOT', lock_root)
    monkeypatch.setattr(module, 'verified_dev_payload', lambda *args: node)
    monkeypatch.setattr(module, 'materialize_allocator', lambda *args: None)
    monkeypatch.setattr(module.shutil, 'which', lambda name: 'mock-node')
    envfile = tmp_path/'github-env'
    monkeypatch.setenv('GITHUB_ENV', str(envfile))
    for key in ('TMP','TEMP','TMPDIR','PIP_CACHE_DIR','npm_config_cache'):
        outside = tmp_path/('inherited-'+key)
        outside.mkdir()
        monkeypatch.setenv(key,str(outside))
    for key in ('CIRNET_ARTIFACTS_SESSION','CIRNET_ARTIFACTS_CONTEXT', 'NETCIRCUIT_ARTIFACTS_SESSION','NETCIRCUIT_ARTIFACTS_CONTEXT'):
        monkeypatch.setenv(key,os.environ.get(key,''))
    monkeypatch.setenv('PYTHONDONTWRITEBYTECODE','0')
    real_run = subprocess.run
    observed = []
    def boundary(command, **kwargs):
        if command[0] == 'mock-node':
            return subprocess.CompletedProcess(command,0,json.dumps({'session':str(session),'context':{'owner':'mock','roots':[str(ROOT)]}}),'')
        assert 'pip' in command
        published = dict(line.split('=',1) for line in envfile.read_text().splitlines())
        env = kwargs.get('env',os.environ)
        for key in ('TMP','TEMP','TMPDIR','PIP_CACHE_DIR','npm_config_cache'):
            assert Path(env[key]).is_relative_to(session), (key, env[key])
            assert Path(env[key]).is_dir(), key
            assert published[key] == env[key], key
        assert env['PYTHONDONTWRITEBYTECODE'] == published['PYTHONDONTWRITEBYTECODE'] == '1'
        # Exercise a real Python process and tempfile rather than trusting the
        # dictionary's appearance or installing into the test interpreter.
        program = 'import json,tempfile,pathlib; p=pathlib.Path(tempfile.mkdtemp()); (p/"probe").write_text("child output"); print(json.dumps(str(p)))'
        child = real_run([sys.executable,'-B','-c',program],env=env,capture_output=True,text=True,check=True)
        output = Path(json.loads(child.stdout))
        assert output.is_relative_to(Path(env['TMPDIR']))
        assert (output/'probe').read_text() == 'child output'
        observed.append(output)
        return subprocess.CompletedProcess(command,0)
    monkeypatch.setattr(module.subprocess,'run',boundary)
    module.bootstrap(wheel=str(wheel),digest='a'*64,repository_root=ROOT)
    assert len(observed) == 1
