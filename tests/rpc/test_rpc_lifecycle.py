"""RPC lifecycle checks against selected native Hardware and C++ Simulator."""

from contextlib import contextmanager, suppress
from hashlib import sha256
import json
import os
from pathlib import Path
import struct
import subprocess
from time import monotonic, perf_counter_ns, sleep

import grpc
import psutil
import pytest
from google.protobuf.json_format import ParseDict

import rpc_v2_pb2 as pb


CANONICAL = "cirnet.hardware.v2.HardwareService"
RETIRED = "netcircuit.hardware.v2.HardwareService"
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


def decode_digital(payload):
    # Check the public NCAP v1 layout independently of Hardware's encoder.
    assert payload.startswith(b"NCAP\x01")
    (header_size,) = struct.unpack_from("<I", payload, 5)
    assert 0 < header_size <= 65_536
    body_offset = 9 + header_size
    header = json.loads(payload[9:body_offset])
    assert set(header) == {"encoding", "signals", "sample_count", "duration_ns"}
    assert header["encoding"] == "digital_events"
    signals = header["signals"]
    assert 0 < len(signals) <= 4096 and len(set(signals)) == len(signals)
    duration = header["duration_ns"]
    sample_count = header["sample_count"]
    assert 0 < sample_count <= 100_000
    assert len(payload) == body_offset + sample_count * 11
    samples = []
    for channel, time_ns, value in struct.iter_unpack("<HQB", payload[body_offset:]):
        assert channel < len(signals) and time_ns <= duration and value <= 2
        samples.append({"signal": signals[channel], "time_ns": time_ns, "value": [0, 1, "X"][value]})
    return {"samples": samples, "duration_ns": duration}


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


def test_virtual_plan_and_capture_are_bound_to_the_experiment(tmp_path):
    owner = target("owner")
    other = target("other")
    with native_server(tmp_path) as (channel, *_):
        client = Client(channel)
        prepare(client, owner)
        start_and_capture(client, owner)
        assert_no_capture(client, other)
        request = pb.PrepareExecutionRequest(
            target=other,
            plan=pb.InstrumentationExecutionPlan(
                schema_version="1.0", station_id=owner.station_id,
                experiment_id=owner.experiment_id,
            ),
            decision=pb.ExecutionDecision(schema_version="1.0", decision="RUN_NORMAL"),
        )
        rejected = client.call("PrepareExecution", request)
        assert not rejected.accepted and rejected.code == pb.RESOURCE_CONFLICT
        assert not rejected.capture_payload

        assert_safe(client.call("StopExecution", other))
        reply = client.call("ReadCapture", owner)
        assert reply.accepted and reply.state == "CAPTURING"
        assert decode_digital(reply.capture_payload) == EXPECTED
        assert_safe(client.call("StopExecution", owner))
        assert_no_capture(client, owner)


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


def run_worker_children(pid, executable, preview_worker):
    # psutil process equality compares PID and creation time.
    return (child for child in worker_children(pid, executable) if child != preview_worker)


def windows_prepared_preview():
    return os.name == "nt" and os.environ.get("CIRNET_WINDOWS_PREPARED_PREVIEW") == "1"


@contextmanager
def native_server(tmp_path, worker_override=None, *, worker_count=1):
    owned = Path(os.environ["CIRNET_ARTIFACTS_SESSION"]).resolve(strict=True)
    assert tmp_path.resolve(strict=True).is_relative_to(owned)
    binary = selected("CIRNET_NATIVE_HARDWARE", "CIRNET_NATIVE_HARDWARE_SHA256")
    worker = selected("CIRNET_SIM_CPP_WORKER", "CIRNET_SIM_CPP_WORKER_SHA256")
    if worker_override is not None:
        assert worker_override.is_relative_to(owned)
        worker = worker_override
    argv = [str(binary), "--listen", "127.0.0.1:0"]
    with loopback_server(
        tmp_path, argv, worker, worker_count=worker_count,
        expect_preview_worker=windows_prepared_preview(),
    ) as server:
        yield server


@contextmanager
def loopback_server(
    tmp_path, argv, worker, *, worker_count=1, expect_preview_worker=False,
):
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
        try:
            def ready():
                assert process.poll() is None, err.read_text(encoding="utf-8", errors="replace")
                lines = out.read_text().splitlines()
                address = next((line.split("=", 1)[1] for line in lines
                                if line.startswith("CIRNET_HARDWARE_READY=127.0.0.1:")), None)
                return address

            address = wait_for(ready, monotonic() + 15, "Hardware did not bind loopback")
            preview_worker = None
            if expect_preview_worker:
                preview_worker, = worker_children(process.pid, worker)
            with grpc.insecure_channel(address) as channel:
                grpc.channel_ready_future(channel).result(timeout=5)
                yield channel, process.pid, worker, address, preview_worker
            if preview_worker is not None:
                assert preview_worker in worker_children(process.pid, worker)
        finally:
            children = list(worker_children(process.pid, worker))
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


def reference_record():
    path = Path(__file__).with_name("fixtures") / "hardware-rpc-v2.json"
    return json.loads(path.read_bytes())


def fixture_reply(result):
    return [*result[:-1], result[-1].hex()]


def test_capabilities_run_capture_reset_and_reentry_match_reference(tmp_path):
    expected = reference_record()
    with native_server(tmp_path) as (channel, *_):
        context = target("parity")
        client = Client(channel)
        for station in (VIRTUAL, PHYSICAL):
            received = client.call("GetCapabilities", target("", station)).SerializeToString()
            assert received.hex() == expected["capabilities"][station]
        args = [
            (VIRTUAL, b"{", pb.INVALID_CONFIGURATION),
            (VIRTUAL, b"null", pb.UNEXECUTABLE),
            (PHYSICAL, b"{", pb.UNEXECUTABLE),
        ]
        for (station, graph_json, code), reference in zip(args, expected["invalid_graphs"], strict=True):
            request = pb.ConfigureVirtualRequest(
                target=target("invalid-graph", station), design_graph_json=graph_json,
            )
            reply = client.call("ConfigureVirtualCircuit", request)
            assert reply.code == code
            assert fixture_reply(wire_result(reply)) == reference

        def assert_execution():
            assert [fixture_reply(reply) for reply in prepare(client, context)] == expected["prepare"]
            assert [fixture_reply(reply) for reply in start_and_capture(client, context)] == expected["start_and_capture"]

        assert_execution()
        assert fixture_reply(wire_result(client.call("Reset", context))) == expected["reset"]
        assert_no_capture(client, context)
        assert_execution()
        assert fixture_reply(wire_result(client.call("StopExecution", context))) == expected["stop"]
        assert_no_capture(client, context)


def test_large_clock_ncap_matches_reference(tmp_path):
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
    reference = reference_record()["clock_capture"]
    with native_server(tmp_path) as (channel, *_):
        context = target("clock-limit")
        client = Client(channel)
        prepare(client, context, graph=graph, capture=capture, analyzer_node=None)
        start = client.call("StartExecution", context, timeout=15)
        assert start.accepted and start.state == "RUNNING", start
        reply = client.call("ReadCapture", context, timeout=10)
        assert reply.accepted and reply.code == pb.OK and reply.state == "CAPTURING", reply
        result = wire_result(reply)
        metadata = result[3]
        assert metadata["sample_count"] == sample_count
        assert metadata["duration_ns"] == duration and "samples" not in metadata
        assert list(result[:-1]) == reference["reply"]
        payload = reply.capture_payload
        assert len(payload) == reference["bytes"]
        assert sha256(payload).hexdigest() == reference["sha256"]
        assert decode_digital(payload) == {"samples": expected, "duration_ns": duration}
        assert_safe(client.call("StopExecution", context))
        print(f"NCAP samples={sample_count} bytes={len(payload)} sha256={reference['sha256']}")


def test_physical_driver_unavailable_and_no_safe_ack(tmp_path):
    with native_server(tmp_path) as (channel, _, _, _, _):
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


def test_native_rejects_retired_hardware_namespace(tmp_path):
    with native_server(tmp_path) as (channel, _, _, _, _):
        context = target("retired-namespace")
        assert Client(channel).call("GetCapabilities", context).available
        retired = Client(channel, RETIRED)
        for method in ("GetCapabilities", "StartExecution", "StopExecution"):
            with pytest.raises(grpc.RpcError) as error:
                retired.call(method, context)
            assert error.value.code() == grpc.StatusCode.UNIMPLEMENTED


@pytest.mark.skipif(os.name != "nt", reason="Windows QPC ingress evidence")
def test_stop_ingress_metadata_is_opt_in_and_uses_the_client_qpc_epoch(tmp_path):
    with native_server(tmp_path) as (channel, _, _, _, _):
        stop_rpc = Client(channel).method("StopExecution")
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


@pytest.mark.parametrize("fault", ["stop", "reset", "crash", "timeout", "disconnect"])
def test_real_cpp_worker_control_fences_faults_and_restarts(tmp_path, fault):
    with native_server(tmp_path) as (channel, pid, worker, _, preview_worker):
        client = Client(channel)
        context = target("owner")
        prepare(client, context)
        _, capture = start_and_capture(client, context)
        assert_safe(client.call("StopExecution", context))
        children = list(run_worker_children(pid, worker, preview_worker))
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
    // Acknowledge only the Windows startup preview's empty CNP1 request.
    let readiness = cfg!(windows)
        && request.len() == 52
        && request.starts_with(b"CNP1")
        && request[20..28] == [0; 8]
        && request[28..32] == 10_000_u32.to_le_bytes()
        && request[32..] == [0; 20];
    let reply_generation = if readiness { generation } else { generation.wrapping_sub(1) };
    let mut reply = request[..12].to_vec();
    reply.extend_from_slice(&reply_generation.to_le_bytes());
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
    with native_server(tmp_path, stale_worker) as (channel, pid, worker, _, preview_worker):
        client = Client(channel)
        context = target("stale")
        prepare(client, context)
        reply = client.call("StartExecution", context)
        assert not reply.accepted and reply.code == pb.PROTOCOL_MISMATCH, reply
        assert_no_capture(client, context)
        assert_safe(client.call("StopExecution", context))
        assert not list(run_worker_children(pid, worker, preview_worker))

START_WAVE_HEADER = "x-cirnet-start-wave"
START_WAVE_ENV = "CIRNET_HARDWARE_START_RENDEZVOUS"


def start_wave_metadata(wave, contexts):
    members = sorted(context.experiment_id for context in contexts)
    return ((START_WAVE_HEADER, ":".join(("v1", wave, *members))),)


@pytest.mark.skipif(os.name != "nt", reason="Windows QPC Start alignment evidence")
@pytest.mark.parametrize("fault", ["missing", "duplicate", "cancelled", "mismatch", "outsider"])
def test_start_wave_aborts_before_station_side_effects(tmp_path, monkeypatch, fault):
    monkeypatch.setenv(START_WAVE_ENV, "1")
    contexts = [target(f"{index:032x}") for index in range(1, 5)]
    wave = "f" * 32
    metadata = start_wave_metadata(wave, contexts)
    with native_server(tmp_path) as (channel, hardware_pid, worker, _, preview_worker):
        client = Client(channel)
        for context in contexts:
            prepare(client, context)
        start = client.method("StartExecution")
        arrivals = [start.future(context, metadata=metadata, timeout=4) for context in contexts[:2]]
        # Start must be waiting before any station or run-worker side effect.
        sleep(.15)
        assert all(not arrival.done() for arrival in arrivals)
        assert not list(run_worker_children(hardware_pid, worker, preview_worker))
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
        assert not list(run_worker_children(hardware_pid, worker, preview_worker))
        for context in contexts:
            assert_no_capture(client, context)
            start_and_capture(client, context)


@pytest.mark.skipif(os.name != "nt", reason="Windows QPC Start alignment evidence")
def test_start_wave_releases_four_members_together(tmp_path, monkeypatch):
    monkeypatch.setenv(START_WAVE_ENV, "1")
    contexts = [target(f"{index:032x}") for index in range(1, 5)]
    wave = "e" * 32
    metadata = start_wave_metadata(wave, contexts)
    with native_server(tmp_path, worker_count=4) as (channel, _, _, _, _):
        client = Client(channel)
        for context in contexts:
            prepare(client, context)
        sent = perf_counter_ns()
        start = client.method("StartExecution")
        arrivals = [
            start.future(context, metadata=metadata, timeout=6)
            for context in contexts
        ]
        common_release = set()
        for context, arrival in zip(contexts, arrivals):
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



def test_start_wave_header_is_inactive_by_default(tmp_path, monkeypatch):
    monkeypatch.delenv(START_WAVE_ENV, raising=False)
    context = target("1" * 32)
    with native_server(tmp_path) as (channel, _, _, _, _):
        client = Client(channel)
        prepare(client, context)
        response, call = client.method("StartExecution").with_call(
            context, metadata=((START_WAVE_HEADER, "unused"),), timeout=3,
        )
        assert response.accepted
        assert START_WAVE_HEADER not in dict(call.initial_metadata())
        assert decode_digital(client.call("ReadCapture", context).capture_payload) == EXPECTED
