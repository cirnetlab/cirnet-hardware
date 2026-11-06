"""Optional paired gRPC Stop diagnostic with one externally held worker."""

from collections import defaultdict
from contextlib import suppress
import json
import os
from pathlib import Path
import platform
import sys
from time import monotonic, monotonic_ns, sleep

import grpc
import psutil
import pytest
from contracts.generated import rpc_v2_pb2 as pb

from test_rpc_lifecycle import (
    CAPTURE,
    GRAPH,
    Client,
    assert_no_capture,
    assert_reference_install,
    assert_safe,
    loopback_server,
    prepare,
    reference_manifest,
    selected,
    sha,
    start_and_capture,
    target,
    wait_for,
    worker_children,
)


SAMPLES = 100
CAPABILITIES_SAMPLES = 1000
ORDERS = (("B", "C"), ("C", "B"), ("B", "C"))
HOLD_SECONDS = .02
REFERENCE_LAUNCH = """
from os import getpid
from hardware_service.server import build_server
server = build_server()
port = server.add_insecure_port("127.0.0.1:0")
if not port:
    raise RuntimeError("could not bind reference to loopback")
server.start()
print(f"CIRNET_HARDWARE_SERVER_PID={getpid()}", flush=True)
print(f"CIRNET_HARDWARE_READY=127.0.0.1:{port}", flush=True)
server.wait_for_termination()
"""


def percentiles(samples):
    if not samples:
        return {}
    ordered = sorted(samples)
    return {
        f"p{p}_ns": ordered[(p * len(ordered) + 99) // 100 - 1]
        for p in (50, 95, 99)
    }


def paired_inputs():
    manifest = reference_manifest()
    assert_reference_install(manifest, "Hardware", "cirnet-hardware-service")
    assert_reference_install(manifest, "Simulator", "cirnet-simulator")
    worker = selected("CIRNET_SIM_CPP_WORKER", "CIRNET_SIM_CPP_WORKER_SHA256")
    assert sha(worker) == manifest["artifacts"]["Worker"]["sha256"]
    native = selected("CIRNET_NATIVE_STAGE3_HARDWARE", "CIRNET_NATIVE_STAGE3_HARDWARE_SHA256")
    argv = {
        "B": [sys.executable, "-I", "-B", "-c", REFERENCE_LAUNCH],
        "C": [str(native), "--listen", "127.0.0.1:0"],
    }
    provenance = {
        "stage1_candidate_manifest_sha256": sha(Path(os.environ["CIRNET_NATIVE_STAGE1_CANDIDATE"])),
        "stage1_hardware_sha256": manifest["artifacts"]["Hardware"]["sha256"],
        "stage1_simulator_sha256": manifest["artifacts"]["Simulator"]["sha256"],
        "stage3_hardware_sha256": sha(native),
        "cpp_worker_sha256": sha(worker),
        "harness_sha256": sha(Path(__file__)),
        "server_harness_sha256": sha(Path(__file__).with_name("test_rpc_lifecycle.py")),
        "environment": {
            "platform": platform.platform(), "machine": platform.machine(),
            "python": platform.python_version(), "grpcio": grpc.__version__,
            "logical_cpus": os.cpu_count(),
        },
        "workers": 1,
        "block_orders": ORDERS,
    }
    return argv, worker, provenance


def sample_stop(client, server, worker, index):
    context = target(f"stop-{index}")
    prepare(client, context)
    start_and_capture(client, context)
    assert_safe(client.call("StopExecution", context))
    child, = worker_children(server.pid, worker)
    prepare(client, context)
    child.suspend()
    try:
        run = client.method("StartExecution").future(context, timeout=15)
        wait_for(
            lambda: client.call("ReadCapture", context).state == "RUNNING",
            monotonic() + 3, "worker job never entered RUNNING",
        )
        sleep(HOLD_SECONDS)
        assert not run.done() and child.is_running()
        worker_rss = child.memory_info().rss
        started = monotonic_ns()
        stopped = client.call("StopExecution", context)
        elapsed = monotonic_ns() - started
        assert_safe(stopped)
        assert not child.is_running(), "safe ACK preceded worker exit"
        invalidated = run.result(timeout=3)
        assert not invalidated.accepted and invalidated.code == pb.INVALID_STATE
        assert_no_capture(client, context)
        # ReadCapture creates a fresh named session after Stop retires its
        # predecessor. Retire this probe session outside the measured window.
        assert_safe(client.call("StopExecution", context))
        return elapsed, worker_rss
    finally:
        with suppress(psutil.NoSuchProcess):
            child.resume()


@pytest.mark.skipif(
    os.environ.get("CIRNET_NATIVE_STAGE3_STOP_DIAGNOSTIC") != "1",
    reason="Set CIRNET_NATIVE_STAGE3_STOP_DIAGNOSTIC=1 for the paired Stop diagnostic.",
)
def test_paired_busy_stop_diagnostic(tmp_path):
    argv, worker, provenance = paired_inputs()
    totals = defaultdict(list)
    receipt = {
        **provenance,
        "status": "FAIL",
        "workload": "Single warmed C++ worker suspended while Start is in flight",
        "scope": "Direct Stop RPC diagnostic",
        "graph": GRAPH,
        "capture": CAPTURE,
        "minimum_post_running_hold_ms": HOLD_SECONDS * 1000,
        "samples_per_block": SAMPLES,
        "blocks": [],
    }
    output = tmp_path / "stop-diagnostic.json"
    try:
        for number, order in enumerate(ORDERS, 1):
            for side in order:
                directory = tmp_path / f"block-{number}-{side}"
                directory.mkdir()
                latencies = []
                block = {"block": number, "side": side, "stop_rpc_ns": latencies}
                receipt["blocks"].append(block)
                with loopback_server(
                    directory, argv[side], worker, reports_server_pid=side == "B",
                ) as (channel, pid, _, _):
                    server, client = psutil.Process(pid), Client(channel)
                    before = server.cpu_times()
                    start = monotonic_ns()
                    max_server_rss = max_worker_rss = 0
                    for index in range(SAMPLES):
                        elapsed, worker_rss = sample_stop(client, server, worker, index)
                        latencies.append(elapsed)
                        totals[side].append(elapsed)
                        max_worker_rss = max(max_worker_rss, worker_rss)
                        max_server_rss = max(max_server_rss, server.memory_info().rss)
                    wall = monotonic_ns() - start
                    after = server.cpu_times()
                    block.update(
                        wall_ns=wall,
                        server_cpu_seconds=(after.user - before.user) + (after.system - before.system),
                        max_sampled_server_rss_bytes=max_server_rss,
                        max_sampled_warm_worker_rss_bytes=max_worker_rss,
                        **percentiles(latencies),
                    )
        assert all(len(totals[side]) == len(ORDERS) * SAMPLES for side in argv)
        receipt["status"] = "PASS"
    finally:
        receipt["totals"] = {
            side: {"samples": len(totals[side]), **percentiles(totals[side])} for side in argv
        }
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(output)


@pytest.mark.skipif(
    os.environ.get("CIRNET_NATIVE_STAGE3_RPC_DIAGNOSTIC") != "1",
    reason="Set CIRNET_NATIVE_STAGE3_RPC_DIAGNOSTIC=1 for the paired GetCapabilities diagnostic.",
)
def test_paired_capabilities_throughput_diagnostic(tmp_path):
    argv, worker, provenance = paired_inputs()
    totals = defaultdict(list)
    elapsed = defaultdict(int)
    receipt = {
        **provenance,
        "status": "FAIL",
        "workload": "Serial warm GetCapabilities RPC against virtual station",
        "samples_per_block": CAPABILITIES_SAMPLES,
        "blocks": [],
    }
    output = tmp_path / "capabilities-diagnostic.json"
    expected = None
    request = target("")
    try:
        for number, order in enumerate(ORDERS, 1):
            for side in order:
                directory = tmp_path / f"block-{number}-{side}"
                directory.mkdir()
                latencies = []
                block = {"block": number, "side": side, "rpc_ns": latencies}
                receipt["blocks"].append(block)
                with loopback_server(
                    directory, argv[side], worker, reports_server_pid=side == "B",
                ) as (channel, pid, _, _):
                    server = psutil.Process(pid)
                    read = Client(channel).method("GetCapabilities")
                    warm = read(request, timeout=3)
                    assert warm.available and warm.mode == "simulation"
                    if expected is None:
                        expected = warm.SerializeToString()
                    assert warm.SerializeToString() == expected
                    before = server.cpu_times()
                    start = monotonic_ns()
                    for _ in range(CAPABILITIES_SAMPLES):
                        called = monotonic_ns()
                        reply = read(request, timeout=3)
                        latencies.append(monotonic_ns() - called)
                        assert reply == warm
                    wall = monotonic_ns() - start
                    after = server.cpu_times()
                    totals[side].extend(latencies)
                    elapsed[side] += wall
                    block.update(
                        wall_ns=wall,
                        rpc_per_second=CAPABILITIES_SAMPLES * 1e9 / wall,
                        server_cpu_seconds=(after.user - before.user) + (after.system - before.system),
                        server_rss_bytes=server.memory_info().rss,
                        **percentiles(latencies),
                    )
        assert all(len(totals[side]) == len(ORDERS) * CAPABILITIES_SAMPLES for side in argv)
        receipt["status"] = "PASS"
    finally:
        receipt["totals"] = {
            side: {
                "samples": len(totals[side]),
                "rpc_per_second": len(totals[side]) * 1e9 / elapsed[side] if elapsed[side] else None,
                **percentiles(totals[side]),
            }
            for side in argv
        }
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(output)
