"""Real allocator regressions keep canonical and pinned legacy session outputs aligned."""
import importlib.util
import json
import os
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def adapter(monkeypatch):
    monkeypatch.syspath_prepend(str(ROOT / 'scripts'))
    spec = importlib.util.spec_from_file_location('allocator_environment_adapter', ROOT / 'scripts/artifact_paths.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize('selection', ['explicit-parent', 'empty-canonical-session'])
def test_new_allocation_exports_both_prefixes_and_second_call_reuses_session(tmp_path, monkeypatch, selection):
    selected_context = os.environ.get('CIRNET_ARTIFACTS_CONTEXT') or os.environ.get('NETCIRCUIT_ARTIFACTS_CONTEXT')
    assert selected_context, 'Bootstrap an owned external verification context before running allocator tests.'
    context = {**json.loads(selected_context), 'owner': 'cirnet-allocator-regression:' + tmp_path.name}
    # Record every artifact input/output that the installed bridge may mutate.
    for prefix in ('CIRNET_', 'NETCIRCUIT_'):
        for suffix in ('ARTIFACTS_CONTEXT', 'ARTIFACTS_SESSION', 'ARTIFACTS_DIR'):
            key = prefix + suffix
            monkeypatch.setenv(key, os.environ.get(key, ''))
    module = adapter(monkeypatch)
    previous_parent = tmp_path / 'previous-parent'
    new_parent = tmp_path / 'new-parent'
    previous_parent.mkdir()
    new_parent.mkdir()
    try:
        previous = module._installed.artifact_directory(directory=previous_parent, context=context)
        for prefix in ('CIRNET_', 'NETCIRCUIT_'):
            monkeypatch.setenv(prefix + 'ARTIFACTS_CONTEXT', json.dumps(context))
            monkeypatch.setenv(prefix + 'ARTIFACTS_SESSION', str(previous))
            monkeypatch.setenv(prefix + 'ARTIFACTS_DIR', str(previous_parent))
        monkeypatch.setenv('CIRNET_UNRELATED_SETTING', 'canonical')
        monkeypatch.setenv('NETCIRCUIT_UNRELATED_SETTING', 'legacy')
        if selection == 'empty-canonical-session':
            monkeypatch.setenv('CIRNET_ARTIFACTS_SESSION', '')
            monkeypatch.setenv('CIRNET_ARTIFACTS_DIR', str(new_parent))
            first = module.artifact_directory(context=context)
        else:
            first = module.artifact_directory(directory=new_parent, context=context)
        assert first.resolve().parent == new_parent.resolve()
        assert first != previous
        # An explicit parent deliberately creates a separate session; the DEV
        # contract preserves the inherited active session in that mode.
        active = first if selection == 'empty-canonical-session' else previous
        assert os.environ['CIRNET_ARTIFACTS_SESSION'] == os.environ['NETCIRCUIT_ARTIFACTS_SESSION'] == str(active)
        assert os.environ['CIRNET_ARTIFACTS_CONTEXT'] == os.environ['NETCIRCUIT_ARTIFACTS_CONTEXT']
        assert os.environ['CIRNET_UNRELATED_SETTING'] == 'canonical'
        assert os.environ['NETCIRCUIT_UNRELATED_SETTING'] == 'legacy'
        second = module.artifact_directory('second-call', context=context)
        assert second.parent == active
        assert previous.is_dir()
        assert len(list(new_parent.iterdir())) == 1
        assert len(list(previous_parent.iterdir())) == 1
    finally:
        # Never clean through inherited session state. Only direct, marker-owned
        # children of these two unique fixture parents may reach the allocator.
        for parent in (previous_parent, new_parent):
            for child in list(parent.iterdir()):
                assert child.resolve().parent == parent.resolve()
                assert any((child / marker).is_file() for marker in (
                    '.netcircuit-artifact-session.json', '.cirnet-artifact-session.json'))
                module._installed.cleanup_artifact_session(child, context=context)
