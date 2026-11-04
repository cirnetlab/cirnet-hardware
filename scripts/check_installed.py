"""Verify exact Hardware wheel ownership before testing installed runtime."""
import argparse
import base64
import csv
from email.parser import Parser
import hashlib
from importlib.metadata import distribution
import io
import json
from pathlib import Path
import sys
from zipfile import ZipFile

from contracts_artifact import installed_provenance as contracts_provenance
from simulator_artifact import installed_provenance as simulator_provenance

VERSION = '0.1.1'
MODULES = ('__init__.py', 'fpga_transport.py', 'models.py', 'rpc.py', 'server.py',
           'station/__init__.py', 'station/base.py', 'station/physical.py', 'station/virtual.py')
OWNED = {'hardware_service/' + name for name in MODULES}
DEPENDENCIES = {'cirnet-contracts==0.1.1', 'cirnet-simulator==0.1.1',
                'grpcio==1.84.0', 'protobuf==7.36.2', 'jsonschema==4.26.0'}


def verify_selected_wheel(wheel, artifact):
    wheel = Path(wheel)
    if not wheel.is_absolute() or wheel.suffix != '.whl':
        raise ValueError('The selected Hardware wheel must be an absolute wheel path.')
    payload = wheel.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    direct = json.loads(artifact.read_text('direct_url.json') or '{}')
    info = direct.get('archive_info', {})
    installed_digest = info.get('hashes', {}).get('sha256')
    if installed_digest is None and info.get('hash', '').startswith('sha256='):
        installed_digest = info['hash'].split('=', 1)[1]
    if installed_digest != digest:
        raise ValueError('Installed Hardware archive digest differs from the selected wheel; force reinstall the selected wheel.')
    prefix = f'cirnet_hardware_service-{VERSION}.dist-info/'
    expected = OWNED | {prefix + name for name in ('licenses/LICENSE', 'METADATA', 'WHEEL', 'top_level.txt', 'RECORD')}
    with ZipFile(io.BytesIO(payload)) as archive:
        if len(archive.namelist()) != len(expected) or set(archive.namelist()) != expected:
            raise ValueError('Unexpected selected Hardware wheel payload inventory.')
        metadata = Parser().parsestr(archive.read(prefix + 'METADATA').decode('utf-8'))
        if metadata['Name'] != 'cirnet-hardware-service' or metadata['Version'] != VERSION:
            raise ValueError('Expected cirnet-hardware-service==0.1.1.')
        dependencies = {item.replace(' ', '') for item in metadata.get_all('Requires-Dist', []) if ';' not in item}
        if dependencies != DEPENDENCIES:
            raise ValueError('Hardware runtime dependency pins differ from the approved release set.')
        selected_records = {row[0]: row[1:] for row in csv.reader(io.StringIO(archive.read(prefix + 'RECORD').decode()))}
        installed_records = {row[0]: row[1:] for row in csv.reader(io.StringIO(artifact.read_text('RECORD') or ''))}
        if set(selected_records) != expected:
            raise ValueError('Selected Hardware wheel RECORD inventory differs from its archive.')
        for name in expected - {prefix + 'RECORD'}:
            if installed_records.get(name) != selected_records[name]:
                raise ValueError('Installed RECORD differs from the selected Hardware wheel: ' + name)
            if Path(artifact.locate_file(name)).read_bytes() != archive.read(name):
                raise ValueError('Installed payload differs from the selected Hardware wheel: ' + name)
    return {'selected_sha256': digest, 'selected_files': len(expected), 'selected_payload_files': len(expected) - 1}


def check_installed(*, wheel=None):
    import hardware_service
    artifact = distribution('cirnet-hardware-service')
    if artifact.version != VERSION or hardware_service.__version__ != VERSION:
        raise ValueError('Expected cirnet-hardware-service==0.1.1.')
    if not artifact.read_text('WHEEL') or not artifact.read_text('RECORD'):
        raise ValueError('Hardware distribution is shadowed by source metadata.')
    direct = json.loads(artifact.read_text('direct_url.json') or '{}')
    if 'dir_info' in direct:
        raise ValueError('Hardware requires a wheel; editable/source-directory installs are unsupported.')
    entry = Path(artifact.locate_file('hardware_service/__init__.py')).resolve()
    if Path(hardware_service.__file__).resolve() != entry:
        raise ValueError('Hardware imports are shadowed by source.')
    checked = set()
    for item in artifact.files or ():
        name = str(item).replace('\\', '/')
        if not name.startswith('hardware_service/') or item.suffix == '.pyc':
            continue
        target = Path(artifact.locate_file(item)).resolve()
        if not target.is_relative_to(entry.parent) or not target.is_file() or item.hash is None or item.hash.mode != 'sha256':
            raise ValueError('Hardware package lacks valid wheel RECORD ownership: ' + name)
        digest = base64.urlsafe_b64encode(hashlib.sha256(target.read_bytes()).digest()).rstrip(b'=').decode()
        if digest != item.hash.value:
            raise ValueError('Hardware package differs from wheel RECORD: ' + name)
        checked.add(name)
    if checked != OWNED:
        raise ValueError('Expected the exact nine-module Hardware runtime inventory.')
    physical = {'hardware_service/' + path.relative_to(entry.parent).as_posix()
                for path in entry.parent.rglob('*') if path.is_file()
                and '__pycache__' not in path.parts and path.suffix != '.pyc'}
    if physical != OWNED:
        raise ValueError('Hardware package contains unexpected or unowned runtime files.')
    for name, module in tuple(sys.modules.items()):
        if name == 'hardware_service' or name.startswith('hardware_service.'):
            origin = getattr(module, '__file__', None)
            if origin and not Path(origin).resolve().is_relative_to(entry.parent):
                raise ValueError('Hardware submodule imports are shadowed by source: ' + name)
    result = {'hardware': {'version': artifact.version, 'root': str(entry.parent), 'owned_files': len(checked)},
              'contracts': contracts_provenance(), 'simulator': simulator_provenance()}
    if wheel is not None:
        result['hardware'].update(verify_selected_wheel(wheel, artifact))
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wheel', type=Path, required=True, help='Bind verification to this exact newly built archive.')
    arguments = parser.parse_args()
    print(json.dumps(check_installed(wheel=arguments.wheel), indent=2))
