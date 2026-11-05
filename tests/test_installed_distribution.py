"""The suite must exercise installed runtime wheels, never product source."""
from importlib.metadata import distribution
from pathlib import Path

import contracts
import hardware_service
import cirnet_sim


def test_runtime_imports_are_owned_by_exact_installed_wheels():
    checkout = Path(__file__).resolve().parents[1]
    for name, version, module in [('cirnet-hardware-service', '0.1.2', hardware_service),
        ('cirnet-contracts', '0.1.2', contracts), ('cirnet-simulator', '0.1.2', cirnet_sim)]:
        artifact = distribution(name)
        assert artifact.version == version
        assert artifact.read_text('WHEEL') and artifact.read_text('RECORD')
        entry = Path(module.__file__).resolve()
        assert entry == Path(artifact.locate_file(module.__name__ + '/__init__.py')).resolve()
        assert not entry.is_relative_to(checkout)
    assert hardware_service.__version__ == '0.1.2'
