"""Opt-in real RPC and worker gate for an installed Stage 1 candidate."""

from contextlib import suppress
from hashlib import sha256
from importlib.metadata import distribution
import json
import os
from pathlib import Path
from threading import Event
from time import monotonic, sleep
from zipfile import ZipFile

import grpc
import psutil
import pytest
from google.protobuf.json_format import ParseDict

import cirnet_sim
from cirnet_sim import cpp_backend
from contracts.capture_codec import decode_digital
from contracts.generated import rpc_v2_pb2 as pb
from contracts.generated import rpc_v2_pb2_grpc as rpc
import hardware_service
from hardware_service.server import build_server
from hardware_service.station.virtual import close_simulation_pool


GRAPH = {
    'schema_version': '1.0',
    'circuit_id': 'native-control-led',
    'modules': [
        {'id': 'X', 'type': 'DIGITAL_SWITCH', 'properties': {'state': True}},
        {'id': 'L', 'type': 'LED'},
    ],
    'connections': [{'source': 'X.OUT', 'destination': 'L.IN'}],
}
CAPTURE = {'signals': ['L.IN'], 'duration_ns': 20}
EXPECTED = [{'signal': 'L.IN', 'time_ns': 0, 'value': 1}]


def selected_file(entry):
    path = Path(entry['path'])
    assert path.is_absolute() and path.is_file() and not path.is_symlink()
    assert sha256(path.read_bytes()).hexdigest() == entry['sha256']
    return path.resolve()


def assert_installed(entry, name, module, namespaces):
    wheel = selected_file(entry)
    artifact = distribution(name)
    direct = json.loads(artifact.read_text('direct_url.json') or '{}')
    assert artifact.version == '0.1.2' and 'dir_info' not in direct
    assert direct['archive_info']['hashes']['sha256'] == entry['sha256']
    assert Path(module.__file__).resolve() == Path(
        artifact.locate_file(f'{module.__name__}/__init__.py')).resolve()
    prefixes = tuple(namespace + '/' for namespace in namespaces)
    installed = {
        f'{namespace}/{path.relative_to(root).as_posix()}'
        for namespace in namespaces
        for root in (Path(artifact.locate_file(namespace)),)
        for path in root.rglob('*')
        if path.is_file() and path.suffix != '.pyc'
    }
    with ZipFile(wheel) as archive:
        selected = {member.filename for member in archive.infolist()
                    if not member.is_dir() and member.filename.startswith(prefixes)}
        assert selected == installed
        for name in selected:
            assert Path(artifact.locate_file(name)).read_bytes() == archive.read(name)


@pytest.fixture
def service(monkeypatch):
    manifest = Path(os.environ['CIRNET_NATIVE_STAGE1_CANDIDATE'])
    assert manifest.is_absolute()
    candidate = json.loads(manifest.read_bytes())
    assert candidate['schemaVersion'] == 1
    selected = candidate['artifacts']
    assert_installed(selected['Simulator'], 'cirnet-simulator', cirnet_sim,
                     ('cirnet_sim', 'netcircuit_sim'))
    assert_installed(selected['Hardware'], 'cirnet-hardware-service', hardware_service,
                     ('hardware_service',))
    worker = selected_file(selected['Worker'])
    close_simulation_pool()
    monkeypatch.setenv('CIRNET_SIMULATION_BACKEND', 'cpp')
    monkeypatch.setenv('CIRNET_SIMULATION_WORKERS', '1')
    monkeypatch.setenv('CIRNET_SIM_CPP_WORKER', str(worker))
    monkeypatch.setenv('CIRNET_SIM_CPP_WORKER_SHA256', selected['Worker']['sha256'])
    server = build_server()
    port = server.add_insecure_port('127.0.0.1:0')
    assert port
    server.start()
    try:
        with grpc.insecure_channel(f'127.0.0.1:{port}') as channel:
            grpc.channel_ready_future(channel).result(timeout=3)
            yield rpc.HardwareServiceStub(channel), worker, selected
    finally:
        server.stop(0).wait()
        close_simulation_pool()


def prepare(client, target):
    plan = ParseDict({
        'schema_version': '1.0',
        'station_id': target.station_id,
        'experiment_id': target.experiment_id,
        'power_policy': {'profile_id': 'virtual-1.0', 'guarded': True},
        'capture_config': CAPTURE,
        'runtime_safety_policy': {'profile_id': 'virtual-1.0', 'watchdog_required': True},
    }, pb.InstrumentationExecutionPlan())
    steps = [
        (client.EnterSafeState, target),
        (client.ConfigureVirtualCircuit,
         pb.ConfigureVirtualRequest(target=target, design_graph_json=json.dumps(GRAPH).encode())),
        (client.PrepareExecution, pb.PrepareExecutionRequest(
            target=target, plan=plan,
            decision=pb.ExecutionDecision(schema_version='1.0', decision='RUN_NORMAL'))),
        (client.ArmCapture, pb.ArmCaptureRequest(target=target, configuration=plan.capture_config)),
    ]
    for method, request in steps:
        reply = method(request, timeout=3)
        assert reply.accepted, reply


def capture(client, target):
    reply = client.ReadCapture(target, timeout=3)
    assert reply.accepted
    metadata = json.loads(reply.result_json)
    assert metadata['available'] is True and metadata['sample_count'] == 1
    assert decode_digital(reply.capture_payload) == {'duration_ns': 20, 'samples': EXPECTED}
    return reply.capture_payload


def active_worker(executable):
    workers = []
    for child in psutil.Process(os.getpid()).children():
        try:
            if Path(child.exe()).resolve() == executable:
                workers.append(child)
        except psutil.NoSuchProcess:
            continue
    assert len(workers) == 1
    return workers[0]


def wait_for_exit(child):
    deadline = monotonic() + .75
    while child.is_running() and monotonic() < deadline:
        sleep(.005)
    assert not child.is_running(), f'worker {child.pid} remains alive'


def assert_safe(reply):
    assert reply.accepted and reply.state == 'SAFE_STATE'
    assert json.loads(reply.result_json)['hardware_applied'] is False


@pytest.mark.parametrize('fault', ['stop', 'reset', 'crash', 'timeout', 'late_reply'])
def test_real_worker_lifecycle(service, monkeypatch, tmp_path, fault):
    client, executable, selected = service
    target = pb.Target(station_id='virtual-station-01', experiment_id='native-control')
    prepare(client, target)
    assert client.StartExecution(target, timeout=15).accepted
    warm_capture = capture(client, target)
    assert_safe(client.StopExecution(target, timeout=3))
    child = active_worker(executable)
    assert child.is_running(), 'normal Stop lost its healthy idle worker'

    prepare(client, target)
    reading, release = Event(), Event()
    read_reply = cpp_backend._read_reply

    def observe_reply(pipe):
        if fault != 'late_reply':
            reading.set()  # The installed IO loop has flushed the actual request.
            return read_reply(pipe)
        reply = read_reply(pipe)
        reading.set()  # Actual reply bytes arrived before Stop.
        assert release.wait(5), 'test did not release the received reply'
        return reply

    with monkeypatch.context() as hook:
        hook.setattr(cpp_backend, '_read_reply', observe_reply)
        suspended = False
        try:
            if fault != 'late_reply':
                child.suspend()
                suspended = True
            running = client.StartExecution.future(target, timeout=15)
            assert reading.wait(3), 'the installed worker did not receive the native request'
            assert not running.done()
            assert child.is_running()
            if fault == 'stop':
                extra = pb.Target(station_id=target.station_id, experiment_id='extra')
                prepare(client, extra)
                rejected = client.StartExecution(extra, timeout=1)
                assert not rejected.accepted and rejected.code == pb.RESOURCE_CONFLICT
                assert_safe(client.StopExecution(extra, timeout=3))
            if fault in ('stop', 'reset'):
                operation = client.StopExecution if fault == 'stop' else client.Reset
                assert_safe(operation(target, timeout=3))
                assert not child.is_running(), 'safe acknowledgement preceded native worker exit'
            elif fault == 'crash':
                child.kill()
            elif fault == 'late_reply':
                stopping = client.StopExecution.future(target, timeout=3)
                wait_for_exit(child)
                release.set()
                assert_safe(stopping.result(timeout=3))
            result = running.result(timeout=15)
        except BaseException:
            if suspended:
                with suppress(psutil.NoSuchProcess):
                    child.resume()
            raise
        finally:
            release.set()

    expected_error = {'crash': pb.DRIVER_ERROR, 'timeout': pb.TIMEOUT}.get(fault, pb.INVALID_STATE)
    assert not result.accepted and result.code == expected_error
    if fault in ('crash', 'timeout'):
        assert_safe(client.StopExecution(target, timeout=3))
    assert not child.is_running(), 'faulted native worker was not reaped'

    empty = client.ReadCapture(target, timeout=3)
    assert empty.accepted and not empty.capture_payload
    assert json.loads(empty.result_json)['available'] is False
    prepare(client, target)
    assert client.StartExecution(target, timeout=15).accepted
    assert capture(client, target) == warm_capture
    replacement = active_worker(executable)
    assert replacement.pid != child.pid
    assert_safe(client.StopExecution(target, timeout=3))
    close_simulation_pool()
    assert not replacement.is_running(), 'worker survived the installed service cleanup'
    (tmp_path / 'lifecycle.json').write_text(json.dumps({
        'fault': fault,
        'sha256': {name: entry['sha256'] for name, entry in selected.items()},
        'protocol': selected['Worker']['protocol'],
        'retired_worker_pid': child.pid,
        'replacement_worker_pid': replacement.pid,
        'start_error': pb.ErrorCode.Name(result.code),
        'capture_sha256': sha256(warm_capture).hexdigest(),
        'scope': 'installed loopback RPC and real native child with test-controlled faults',
    }, indent=2) + '\n', encoding='utf-8')
