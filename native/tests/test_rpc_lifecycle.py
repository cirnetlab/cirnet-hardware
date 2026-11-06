"""Opt-in RPC lifecycle gate for exact Stage 1 and Stage 3 artifacts."""

from contextlib import contextmanager, suppress
from hashlib import sha256
from importlib.metadata import distribution
import json
import os
from pathlib import Path
import subprocess
from time import monotonic, perf_counter_ns, sleep

import grpc
import psutil
import pytest
from google.protobuf.json_format import ParseDict

from contracts.capture_codec import decode_digital, encode_digital
from contracts.generated import rpc_v2_pb2 as pb


CANONICAL = "cirnet.hardware.v2.HardwareService"
LEGACY = "netcircuit.hardware.v2.HardwareService"
VIRTUAL = "virtual-station-01"
PHYSICAL = "physical-station-01"
GRAPH = {
    "schema_version": "1.0",
    "circuit_id": "native-rpc-led",
    "modules": [
        {"id": "X", "type": "DIGITAL_SWITCH", "properties": {"state": True}},
        {"id": "L", "type": "LED"},
    ],
    "connections": [{"source": "X.OUT", "destination": "L.IN"}],
}
CAPTURE = {"signals": ["output"], "duration_ns": 20}
EXPECTED = {
    "duration_ns": 20,
    "samples": [{"signal": "output", "time_ns": 0, "value": 1}],
}


def sha(path):
    return sha256(path.read_bytes()).hexdigest()


def selected(name, digest):
    path = Path(os.environ[name])
    assert path.is_absolute() and path.is_file() and not path.is_symlink()
    assert sha(path) == os.environ[digest]
    return path.resolve()


class Client:
    def __init__(self, channel, service=CANONICAL):
        self.channel = channel
        self.service = service

    def method(self, name):
        reply = pb.StationCapabilities if name == "GetCapabilities" else pb.OperationReply
        return self.channel.unary_unary(
            f"/{self.service}/{name}",
            request_serializer=lambda request: request.SerializeToString(),
            response_deserializer=reply.FromString,
        )

    def call(self, name, request, timeout=3):
        return self.method(name)(request, timeout=timeout)


def target(experiment, station=VIRTUAL):
    return pb.Target(station_id=station, experiment_id=experiment)


def prepare(client, context, *, graph=GRAPH, capture=CAPTURE, analyzer_node="L.IN"):
    fields = {
        "schema_version": "1.0",
        "station_id": context.station_id,
        "experiment_id": context.experiment_id,
        "power_policy": {"profile_id": "virtual-1.0", "guarded": True},
        "capture_config": capture,
        "runtime_safety_policy": {"profile_id": "virtual-1.0", "watchdog_required": True},
    }
    if analyzer_node is not None:
        fields["endpoint_bindings"] = [
            {"endpoint_id": "output", "node_id": analyzer_node, "role": "LOGIC_ANALYZER"},
        ]
        fields["instrument_allocations"] = [
            {"endpoint_id": "output", "instrument": "LOGIC_ANALYZER", "channel": 1},
        ]
    plan = ParseDict(fields, pb.InstrumentationExecutionPlan())
    operations = (
        ("EnterSafeState", context),
        ("ConfigureVirtualCircuit", pb.ConfigureVirtualRequest(
            target=context, design_graph_json=json.dumps(graph).encode("utf-8"))),
        ("PrepareExecution", pb.PrepareExecutionRequest(
            target=context, plan=plan, decision=pb.ExecutionDecision(
                schema_version="1.0", decision="RUN_NORMAL"))),
        ("ApplyRouting", context),
        ("GuardedPowerUp", context),
        ("ArmCapture", pb.ArmCaptureRequest(target=context, configuration=plan.capture_config)),
    )
    replies = []
    for name, request in operations:
        result = client.call(name, request)
        assert result.accepted, (name, result)
        replies.append(wire_result(result))
    return replies


def wire_result(reply):
    return reply.accepted, reply.code, reply.state, json.loads(reply.result_json), reply.capture_payload


def start_and_capture(client, context):
    start = client.call("StartExecution", context, timeout=15)
    assert start.accepted and start.state == "RUNNING", start
    reply = client.call("ReadCapture", context)
    assert reply.accepted and reply.state == "CAPTURING", reply
    metadata = json.loads(reply.result_json)
    assert metadata["sample_count"] == 1 and "samples" not in metadata
    assert decode_digital(reply.capture_payload) == EXPECTED
    return wire_result(start), wire_result(reply)


def assert_safe(reply):
    assert reply.accepted and reply.state == "SAFE_STATE", reply
    assert json.loads(reply.result_json)["hardware_applied"] is False


def assert_no_capture(client, context):
    reply = client.call("ReadCapture", context)
    assert reply.accepted and reply.code == pb.NOT_IMPLEMENTED
    assert not reply.capture_payload
    assert json.loads(reply.result_json) == {"available": False, "code": "NOT_IMPLEMENTED"}


def wait_for(predicate, deadline, message):
    while monotonic() < deadline:
        value = predicate()
        if value:
            return value
        sleep(.01)
    pytest.fail(message)


def worker_children(pid, executable):
    with suppress(psutil.NoSuchProcess):
        for child in psutil.Process(pid).children():
            with suppress(psutil.NoSuchProcess):
                if Path(child.exe()).resolve() == executable:
                    yield child


@contextmanager
def native_server(tmp_path, worker_override=None, *, worker_count=1):
    owned = Path(os.environ["CIRNET_ARTIFACTS_SESSION"]).resolve(strict=True)
    assert tmp_path.resolve(strict=True).is_relative_to(owned)
    binary = selected("CIRNET_NATIVE_STAGE3_HARDWARE", "CIRNET_NATIVE_STAGE3_HARDWARE_SHA256")
    worker = selected("CIRNET_SIM_CPP_WORKER", "CIRNET_SIM_CPP_WORKER_SHA256")
    if worker_override is not None:
        assert worker_override.is_relative_to(owned)
        worker = worker_override
    argv = [str(binary), "--listen", "127.0.0.1:0"]
    with loopback_server(tmp_path, argv, worker, worker_count=worker_count) as server:
        yield server


@contextmanager
def loopback_server(tmp_path, argv, worker, *, reports_server_pid=False, worker_count=1):
    owned = Path(os.environ["CIRNET_ARTIFACTS_SESSION"]).resolve(strict=True)
    assert tmp_path.resolve(strict=True).is_relative_to(owned)
    contracts = Path(os.environ["CIRNET_CONTRACTS_ROOT"]).resolve(strict=True)
    env = {
        **os.environ,
        "CIRNET_CONTRACTS_ROOT": str(contracts),
        "CIRNET_SIM_CPP_WORKER": str(worker),
        "CIRNET_SIM_CPP_WORKER_SHA256": sha(worker),
        "CIRNET_SIMULATION_BACKEND": "cpp",
        "CIRNET_SIMULATION_WORKERS": str(worker_count),
    }
    out = tmp_path / "hardware.stdout.log"
    err = tmp_path / "hardware.stderr.log"
    with out.open("wb") as stdout, err.open("wb") as stderr:
        process = subprocess.Popen(
            argv,
            stdout=stdout, stderr=stderr, env=env, cwd=tmp_path,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        launcher = psutil.Process(process.pid)
        server = launcher
        try:
            def ready():
                assert process.poll() is None, err.read_text(encoding="utf-8", errors="replace")
                lines = out.read_text().splitlines()
                address = next((line.split("=", 1)[1] for line in lines
                                if line.startswith("CIRNET_HARDWARE_READY=127.0.0.1:")), None)
                if address is None:
                    return None
                if not reports_server_pid:
                    return address, launcher
                # On Windows a venv's python.exe can be a redirector. The
                # serving interpreter must be this process or its descendant.
                pid, = (int(line.split("=", 1)[1]) for line in lines
                        if line.startswith("CIRNET_HARDWARE_SERVER_PID="))
                serving_process = psutil.Process(pid)
                assert serving_process == launcher or serving_process in launcher.children(recursive=True)
                return address, serving_process

            address, server = wait_for(ready, monotonic() + 15, "Hardware did not bind loopback")
            with grpc.insecure_channel(address) as channel:
                grpc.channel_ready_future(channel).result(timeout=5)
                yield channel, server.pid, worker, address
        finally:
            children = list(worker_children(server.pid, worker))
            if server != launcher:
                with suppress(psutil.NoSuchProcess):
                    server.terminate()
                    server.wait(timeout=5)
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
            _, retained = psutil.wait_procs(children, timeout=3)
            # Only the exact worker children of this test-owned process can be killed.
            for child in retained:
                with suppress(psutil.NoSuchProcess):
                    child.kill()
            psutil.wait_procs(retained, timeout=3)
            assert not retained, [child.pid for child in retained]


def reference_manifest():
    candidate_path = Path(os.environ["CIRNET_NATIVE_STAGE1_CANDIDATE"])
    assert candidate_path.is_absolute()
    return json.loads(candidate_path.read_bytes())


def assert_reference_install(manifest, key, package):
    assert_wheel_install(package, manifest["artifacts"][key]["sha256"])


def assert_wheel_install(package, digest):
    installed = distribution(package)
    origin = json.loads(installed.read_text("direct_url.json"))
    assert origin["archive_info"]["hashes"]["sha256"] == digest


def assert_remote_install():
    digest = os.environ.get("CIRNET_NATIVE_STAGE3_REMOTE_SHA256")
    if digest is None:
        digest = reference_manifest()["artifacts"]["Remote"]["sha256"]
    assert_wheel_install("cirnet-api", digest)


@contextmanager
def reference_server(monkeypatch):
    from hardware_service.server import build_server
    from hardware_service.station.virtual import close_simulation_pool

    manifest = reference_manifest()
    assert_reference_install(manifest, "Hardware", "cirnet-hardware-service")
    assert_reference_install(manifest, "Simulator", "cirnet-simulator")
    assert os.environ["CIRNET_SIM_CPP_WORKER_SHA256"] == manifest["artifacts"]["Worker"]["sha256"]
    monkeypatch.setenv("CIRNET_SIMULATION_BACKEND", "cpp")
    monkeypatch.setenv("CIRNET_SIMULATION_WORKERS", "1")
    close_simulation_pool()
    server = build_server()
    port = server.add_insecure_port("127.0.0.1:0")
    assert port
    server.start()
    try:
        with grpc.insecure_channel(f"127.0.0.1:{port}") as channel:
            grpc.channel_ready_future(channel).result(timeout=5)
            yield channel
    finally:
        server.stop(0).wait()
        close_simulation_pool()


@pytest.mark.parametrize("service", [CANONICAL, LEGACY])
def test_capabilities_run_capture_reset_and_reentry_match_reference(tmp_path, monkeypatch, service):
    with reference_server(monkeypatch) as reference, native_server(tmp_path) as (native, _, _, _):
        context = target("parity")
        old, new = Client(reference, service), Client(native, service)
        for station in [VIRTUAL, PHYSICAL]:
            assert old.call("GetCapabilities", target("", station)).SerializeToString() == (
                new.call("GetCapabilities", target("", station)).SerializeToString()
            )
        for station, graph_json, code in [
            (VIRTUAL, b"{", pb.INVALID_CONFIGURATION),
            (VIRTUAL, b"null", pb.UNEXECUTABLE),
            (PHYSICAL, b"{", pb.UNEXECUTABLE),
        ]:
            request = pb.ConfigureVirtualRequest(
                target=target("invalid-graph", station), design_graph_json=graph_json,
            )
            expected = old.call("ConfigureVirtualCircuit", request)
            assert expected.code == code
            assert wire_result(new.call("ConfigureVirtualCircuit", request)) == wire_result(expected)
        assert prepare(new, context) == prepare(old, context)
        assert start_and_capture(new, context) == start_and_capture(old, context)
        assert wire_result(new.call("Reset", context)) == wire_result(old.call("Reset", context))
        assert_no_capture(new, context)
        assert prepare(new, context) == prepare(old, context)
        assert start_and_capture(new, context) == start_and_capture(old, context)
        assert wire_result(new.call("StopExecution", context)) == wire_result(old.call("StopExecution", context))
        assert_no_capture(new, context)


def test_large_clock_ncap_matches_reference(tmp_path, monkeypatch):
    # Keep the pinned external baseline clock graph, probe and event budget.
    # Extend its duration only for this maximum-sample boundary test.
    graph = {
        "schema_version": "1.0",
        "circuit_id": "baseline-clock",
        "modules": [{"id": "C", "type": "CLOCK", "properties": {"frequency_hz": 100_000_000}}],
        "connections": [],
    }
    sample_count = 100_000
    duration = 5 * (sample_count - 1)
    capture = {"signals": ["C.OUT"], "duration_ns": duration, "max_events": sample_count}
    expected = [
        {"signal": "C.OUT", "time_ns": 5 * index, "value": index % 2}
        for index in range(sample_count)
    ]
    digest = sha256(encode_digital(expected, duration)).hexdigest()
    with reference_server(monkeypatch) as reference, native_server(tmp_path) as (native, _, _, _):
        context = target("clock-limit")
        replies = []
        for channel in (reference, native):
            client = Client(channel)
            prepare(client, context, graph=graph, capture=capture, analyzer_node=None)
            start = client.call("StartExecution", context, timeout=15)
            assert start.accepted and start.state == "RUNNING", start
            reply = client.call("ReadCapture", context, timeout=10)
            assert reply.accepted and reply.code == pb.OK and reply.state == "CAPTURING", reply
            metadata = json.loads(reply.result_json)
            assert metadata["sample_count"] == sample_count
            assert metadata["duration_ns"] == duration and "samples" not in metadata
            assert sha256(reply.capture_payload).hexdigest() == digest
            replies.append(wire_result(reply))
            assert_safe(client.call("StopExecution", context))
        assert replies[0] == replies[1]
        payload = replies[1][-1]
        assert decode_digital(payload) == {"samples": expected, "duration_ns": duration}
        print(f"NCAP samples={sample_count} bytes={len(payload)} sha256={digest}")


def test_physical_driver_unavailable_and_no_safe_ack(tmp_path):
    with native_server(tmp_path) as (channel, _, _, _):
        client = Client(channel)
        context = target("no-driver", PHYSICAL)
        caps = client.call("GetCapabilities", context)
        assert not caps.available and not caps.physical_digital
        assert not caps.guarded_run_supported and caps.physical_execution_slots == 0
        assert all(
            getattr(caps.instrument_capabilities, instrument).channels == 0
            for instrument in ("logic_analyzer", "oscilloscope", "generator")
        )
        for name in ("StartExecution", "ApplyRouting", "GuardedPowerUp"):
            reply = client.call(name, context)
            assert not reply.accepted and reply.code == pb.HARDWARE_UNAVAILABLE
        for name in ("EnterSafeState", "StopExecution", "Reset"):
            reply = client.call(name, context)
            assert not reply.accepted and reply.state == "OFFLINE"
            assert json.loads(reply.result_json)["hardware_applied"] is False
        with pytest.raises(grpc.RpcError) as error:
            client.call("GetCapabilities", target("", "missing-station"))
        assert error.value.code() == grpc.StatusCode.NOT_FOUND


@pytest.mark.skipif(os.name != "nt", reason="Windows QPC ingress evidence")
@pytest.mark.parametrize("service", [CANONICAL, LEGACY])
def test_stop_ingress_metadata_is_opt_in_and_uses_the_client_qpc_epoch(tmp_path, service):
    with native_server(tmp_path) as (channel, _, _, _):
        stop_rpc = Client(channel, service).method("StopExecution")
        context = target("1" * 32)
        key = "x-cirnet-stop-ingress"
        ordinary, call = stop_rpc.with_call(context, timeout=3)
        assert_safe(ordinary)
        assert key not in dict(call.initial_metadata())
        invalid_requests = (
            (context, ((key, "qpc-ns-v1:1:123"),)),
            (context, ((key, "qpc-ns-v1"), (key, "qpc-ns-v1"))),
            (target("non-uuid"), ((key, "qpc-ns-v1"),)),
        )
        for request, metadata in invalid_requests:
            rejected_probe, call = stop_rpc.with_call(request, metadata=metadata, timeout=3)
            assert_safe(rejected_probe)
            assert key not in dict(call.initial_metadata())
        for _ in range(3):
            sent = perf_counter_ns()
            traced, call = stop_rpc.with_call(context, metadata=((key, "qpc-ns-v1"),), timeout=3)
            returned = perf_counter_ns()
            assert traced == ordinary
            proof, = [value for name, value in call.initial_metadata() if name == key]
            clock, experiment, entered = proof.split(":")
            assert clock == "qpc-ns-v1" and experiment == context.experiment_id
            assert sent <= int(entered) <= returned


def test_remote_recovery_quarantines_physical_lease_without_safe_ack(tmp_path):
    from app.services.capture_store import FilesystemCaptureStore
    from app.services.experiments import ExperimentManager
    from app.services.hardware_client import GatewayError, HardwareServiceClient
    from app.services.persistence import Repository
    from sqlalchemy import select

    assert_remote_install()
    with native_server(tmp_path) as (_, _, _, address):
        database_url = "sqlite:///" + (tmp_path / "recovery.sqlite").as_posix()
        gateway = HardwareServiceClient(address)
        repository = Repository(database_url)
        experiment = "2" * 32
        owner = "stage3-recovery"
        history = ["CREATED", "QUEUED", "RESERVED", "RUNNING"]
        lease = {
            "station_id": PHYSICAL, "slot": 0, "experiment_id": experiment,
            "expires_at": 120.0, "quarantined": False,
        }
        try:
            caps = gateway.capabilities(PHYSICAL)
            assert caps["mode"] == "hardware" and not caps["available"]
            # Model a lease retained from the previous service process.
            # No test operation attempts to power or drive a physical station.
            repository.create_experiment(owner, {
                "schema_version": "1.0", "experiment_id": experiment,
                "circuit_id": experiment, "station_id": PHYSICAL, "mode": "hardware",
                "state": "RUNNING", "state_history": history, "revision": len(history) - 1,
                "lease_expires_at": lease["expires_at"], "capture": None,
            })
            assert repository.reserve(PHYSICAL, 0, experiment, lease["expires_at"])
            assert repository.leases() == [lease]
            with pytest.raises(GatewayError) as rejected:
                gateway.call(PHYSICAL, experiment, "stop_execution")
            assert rejected.value.code == "HARDWARE_UNAVAILABLE"
            recovered = ExperimentManager(
                repository, gateway, capture_store=FilesystemCaptureStore(tmp_path / "captures"),
            )
            try:
                record = recovered.get(experiment, owner)
                assert record["state"] == "FAULT" and record["error_code"] == "RESTART_RECOVERY"
                assert record["state_history"] == [*history, "FAULT"]
                assert record["lease_expires_at"] is None and record["capture"] is None
            finally:
                recovered.close()
        finally:
            gateway.close()
            repository.close()

        reopened = Repository(database_url)
        try:
            assert reopened.leases() == [{**lease, "quarantined": True}]
            with reopened.engine.connect() as db:
                assert db.execute(select(reopened.audit.c.action).where(
                    reopened.audit.c.subject_id == experiment,
                )).scalar_one() == "EXPERIMENT_RESTART_QUARANTINED"
        finally:
            reopened.close()


@pytest.mark.parametrize("trigger,code", [("expiry", "TIMEOUT"), ("disconnect", "DISCONNECTED")])
def test_remote_cancelled_run_releases_lease_after_safe_stop(tmp_path, trigger, code):
    from app.services.capture_store import FilesystemCaptureStore
    from app.services.experiments import ExperimentManager
    from app.services.hardware_client import HardwareServiceClient
    from app.services.persistence import Repository

    assert_remote_install()
    with native_server(tmp_path) as (channel, pid, worker, address):
        client = Client(channel)
        warm = target("warm")
        prepare(client, warm)
        start_and_capture(client, warm)
        assert_safe(client.call("StopExecution", warm))
        child, = worker_children(pid, worker)
        repository = Repository("sqlite:///" + (tmp_path / "remote.sqlite").as_posix())
        gateway = HardwareServiceClient(address)
        now = [100.0]
        manager = ExperimentManager(
            repository, gateway, clock=lambda: now[0], lease_seconds=1,
            capture_store=FilesystemCaptureStore(tmp_path / "captures"),
        )
        owner = "stage3-control"
        try:
            record = manager.create(owner, {
                "station_id": VIRTUAL, "circuit_graph": GRAPH,
                "capture_config": {"signals": ["L.IN"], "duration_ns": CAPTURE["duration_ns"]},
            })
            eid = record["experiment_id"]
            context = target(eid)
            child.suspend()
            manager.run(eid, owner, control_session_id="session", control_client_id="tab")
            wait_for(
                lambda: client.call("ReadCapture", context).state == "RUNNING",
                monotonic() + 3, "Remote did not start the native worker",
            )
            sleep(.02)
            lease, = repository.leases()
            assert lease["experiment_id"] == eid and not lease["quarantined"]
            if trigger == "expiry":
                now[0] += 2
                manager.expire_leases()
            else:
                manager.disconnect_control("session", "tab")
            wait_for(
                lambda: manager.get(eid, owner)["state"] == "COMPLETE",
                monotonic() + 4, "Remote did not finish after confirmed Stop",
            )
            result = manager.get(eid, owner)
            assert result["error_code"] == code and result["capture"] is None
            assert result["state_history"][-4:] == ["STOPPING", "SAFE_STATE", "RELEASE", "COMPLETE"]
            assert not child.is_running() and repository.leases() == []
            assert_no_capture(client, context)
        finally:
            with suppress(psutil.NoSuchProcess):
                child.resume()
            manager.close()
            gateway.close()
            repository.close()


@pytest.mark.parametrize("fault", ["stop", "reset", "crash", "timeout", "disconnect"])
def test_real_cpp_worker_control_fences_faults_and_restarts(tmp_path, fault):
    with native_server(tmp_path) as (channel, pid, worker, _):
        client = Client(channel)
        context = target("owner")
        prepare(client, context)
        _, capture = start_and_capture(client, context)
        assert_safe(client.call("StopExecution", context))
        children = list(worker_children(pid, worker))
        assert len(children) == 1
        child, = children
        assert child.is_running()
        prepare(client, context)
        peer = target("peer")
        prepare(client, peer)
        child.suspend()
        try:
            run = client.method("StartExecution").future(context, timeout=15)
            wait_for(
                lambda: client.call("ReadCapture", context).state == "RUNNING",
                monotonic() + 3, "worker job never entered RUNNING",
            )
            assert not run.done()
            if fault in ("stop", "reset"):
                # With all native lanes busy, another Start fails without using
                # the resources needed to Stop either experiment.
                unavailable = client.call("StartExecution", peer, timeout=1)
                assert not unavailable.accepted and unavailable.code == pb.RESOURCE_CONFLICT
                assert_safe(client.call("StopExecution", peer, timeout=2))
                stop = "StopExecution" if fault == "stop" else "Reset"
                assert_safe(client.call(stop, context))
                assert not child.is_running(), "safe ACK preceded the child exit"
            elif fault == "crash":
                child.kill()
            elif fault == "disconnect":
                assert run.cancel()
                wait_for(
                    lambda: not child.is_running(),
                    monotonic() + 3, "cancelled RPC did not reap the owned child",
                )
            if fault != "disconnect":
                reply = run.result(timeout=15)
                expected = {"crash": pb.DRIVER_ERROR, "timeout": pb.TIMEOUT}.get(fault, pb.INVALID_STATE)
                assert not reply.accepted and reply.code == expected, reply
            if fault not in ("stop", "reset"):
                assert_safe(client.call("StopExecution", context))
            assert not child.is_running(), "faulted native worker was not reaped"
        finally:
            with suppress(psutil.NoSuchProcess):
                child.resume()

        assert_no_capture(client, context)
        prepare(client, context)
        assert start_and_capture(client, context)[1] == capture
        assert_safe(client.call("StopExecution", context))


@pytest.fixture
def stale_worker(tmp_path):
    source = tmp_path / "stale_worker.rs"
    executable = tmp_path / ("stale_worker.exe" if os.name == "nt" else "stale_worker")
    source.write_text(r'''
use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let (mut stdin, mut stdout) = (io::stdin().lock(), io::stdout().lock());
    let mut prefix = [0; 4];
    stdin.read_exact(&mut prefix)?;
    let mut request = vec![0; u32::from_le_bytes(prefix) as usize];
    stdin.read_exact(&mut request)?;
    let generation = u64::from_le_bytes(request[12..20].try_into().unwrap());
    let mut reply = request[..12].to_vec();
    reply.extend_from_slice(&generation.wrapping_sub(1).to_le_bytes());
    reply.extend_from_slice(&[0; 5]);
    stdout.write_all(&(reply.len() as u32).to_le_bytes())?;
    stdout.write_all(&reply)?;
    stdout.flush()?;
    // Wait for the supervised parent to close or reap this child.
    let _ = stdin.read(&mut prefix);
    Ok(())
}
''', encoding="utf-8")
    subprocess.run(
        ["rustc", "--edition=2024", str(source), "-o", str(executable)],
        cwd=tmp_path, capture_output=True, check=True, timeout=30,
    )
    return executable.resolve()


def test_worker_reply_with_stale_generation_never_seals_capture(tmp_path, stale_worker):
    with native_server(tmp_path, stale_worker) as (channel, pid, worker, _):
        client = Client(channel)
        context = target("stale")
        prepare(client, context)
        reply = client.call("StartExecution", context)
        assert not reply.accepted and reply.code == pb.PROTOCOL_MISMATCH, reply
        assert_no_capture(client, context)
        assert_safe(client.call("StopExecution", context))
        assert not list(worker_children(pid, worker))

START_WAVE_HEADER = "x-cirnet-start-wave"
START_WAVE_ENV = "CIRNET_HARDWARE_START_RENDEZVOUS"


def start_wave_metadata(wave, contexts):
    members = sorted(context.experiment_id for context in contexts)
    return ((START_WAVE_HEADER, ":".join(("v1", wave, *members))),)


@pytest.mark.skipif(os.name != "nt", reason="Windows QPC Start alignment evidence")
@pytest.mark.parametrize("service", [CANONICAL, LEGACY])
@pytest.mark.parametrize("fault", ["missing", "duplicate", "cancelled", "mismatch", "outsider"])
def test_start_wave_aborts_before_station_side_effects(tmp_path, monkeypatch, service, fault):
    monkeypatch.setenv(START_WAVE_ENV, "1")
    contexts = [target(f"{index:032x}") for index in range(1, 5)]
    wave = "f" * 32
    metadata = start_wave_metadata(wave, contexts)
    with native_server(tmp_path) as (channel, hardware_pid, worker, _):
        client = Client(channel, service)
        for context in contexts:
            prepare(client, context)
        start = client.method("StartExecution")
        arrivals = [start.future(context, metadata=metadata, timeout=4) for context in contexts[:2]]
        # Existing Start must be waiting before any station or child side effect.
        sleep(.15)
        assert all(not arrival.done() for arrival in arrivals)
        assert not list(worker_children(hardware_pid, worker))
        expected = grpc.StatusCode.DEADLINE_EXCEEDED
        if fault == "duplicate":
            arrivals.append(start.future(contexts[0], metadata=metadata, timeout=4))
            expected = grpc.StatusCode.FAILED_PRECONDITION
        elif fault == "cancelled":
            assert arrivals.pop().cancel()
            expected = grpc.StatusCode.CANCELLED
        elif fault == "mismatch":
            other = start_wave_metadata(wave, [*contexts[:3], target("9" * 32)])
            arrivals.append(start.future(contexts[2], metadata=other, timeout=4))
            expected = grpc.StatusCode.FAILED_PRECONDITION
        elif fault == "outsider":
            arrivals.append(start.future(target("9" * 32), metadata=metadata, timeout=4))
            expected = grpc.StatusCode.FAILED_PRECONDITION
        for arrival in arrivals:
            with pytest.raises(grpc.RpcError) as aborted:
                arrival.result()
            assert aborted.value.code() == expected
        # A closed wave cannot be reconstructed by late arrivals.
        with pytest.raises(grpc.RpcError) as late:
            start(contexts[-1], metadata=metadata, timeout=3)
        assert late.value.code() == expected
        assert not list(worker_children(hardware_pid, worker))
        for context in contexts:
            assert_no_capture(client, context)
            start_and_capture(client, context)


@pytest.mark.skipif(os.name != "nt", reason="Windows QPC Start alignment evidence")
def test_start_wave_releases_four_members_across_namespaces(tmp_path, monkeypatch):
    monkeypatch.setenv(START_WAVE_ENV, "1")
    contexts = [target(f"{index:032x}") for index in range(1, 5)]
    wave = "e" * 32
    metadata = start_wave_metadata(wave, contexts)
    with native_server(tmp_path, worker_count=4) as (channel, _, _, _):
        clients = [Client(channel, service) for service in (CANONICAL, LEGACY) * 2]
        for client, context in zip(clients, contexts):
            prepare(client, context)
        sent = perf_counter_ns()
        arrivals = [
            client.method("StartExecution").future(context, metadata=metadata, timeout=6)
            for client, context in zip(clients, contexts)
        ]
        common_release = set()
        for client, context, arrival in zip(clients, contexts, arrivals):
            assert arrival.result().accepted
            received = perf_counter_ns()
            stamp, = [value for key, value in arrival.initial_metadata() if key == START_WAVE_HEADER]
            version, stamped_wave, identity, entered, released = stamp.split(":")
            assert (version, stamped_wave, identity) == ("qpc-ns-v1", wave, context.experiment_id)
            assert sent <= int(entered) <= int(released) <= received
            common_release.add(int(released))
            capture = client.call("ReadCapture", context)
            assert decode_digital(capture.capture_payload) == EXPECTED
        assert len(common_release) == 1



@pytest.mark.parametrize("service", [CANONICAL, LEGACY])
def test_start_wave_header_is_inactive_by_default(tmp_path, monkeypatch, service):
    monkeypatch.delenv(START_WAVE_ENV, raising=False)
    context = target("1" * 32)
    with native_server(tmp_path) as (channel, _, _, _):
        client = Client(channel, service)
        prepare(client, context)
        response, call = client.method("StartExecution").with_call(
            context, metadata=((START_WAVE_HEADER, "unused"),), timeout=3,
        )
        assert response.accepted
        assert START_WAVE_HEADER not in dict(call.initial_metadata())
        assert decode_digital(client.call("ReadCapture", context).capture_payload) == EXPECTED