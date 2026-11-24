"""Optional native RPC diagnostics using the reference workloads."""

from contextlib import suppress
import json
import os
from pathlib import Path
import platform
from time import monotonic, monotonic_ns, sleep

import grpc
import psutil
import pytest
import rpc_v2_pb2 as pb

from test_rpc_lifecycle import (
    CAPTURE,
    GRAPH,
    Client,
    assert_no_capture,
    assert_safe,
    native_server,
    prepare,
    reference_record,
    run_worker_children,
    selected,
    sha,
    start_and_capture,
    target,
    wait_for,
)


SAMPLES = 100
CAPABILITIES_SAMPLES = 1000
BLOCKS = 3
HOLD_SECONDS = .02


def percentiles(samples):
    if not samples:
        return {}
    ordered = sorted(samples)
    return {
        f"p{p}_ns": ordered[(p * len(ordered) + 99) // 100 - 1]
        for p in (50, 95, 99)
    }


def diagnostic_provenance():
    worker = selected("CIRNET_SIM_CPP_WORKER", "CIRNET_SIM_CPP_WORKER_SHA256")
    native = selected("CIRNET_NATIVE_HARDWARE", "CIRNET_NATIVE_HARDWARE_SHA256")
    return {
        "hardware_sha256": sha(native),
        "cpp_worker_sha256": sha(worker),
        "harness_sha256": sha(Path(__file__)),
        "server_harness_sha256": sha(Path(__file__).with_name("test_rpc_lifecycle.py")),
        "environment": {
            "platform": platform.platform(), "machine": platform.machine(),
            "python": platform.python_version(), "grpcio": grpc.__version__,
            "logical_cpus": os.cpu_count(),
        },
        "workers": 1,
    }


def sample_stop(client, server, worker, preview_worker, index):
    context = target(f"stop-{index}")
    prepare(client, context)
    start_and_capture(client, context)
    assert_safe(client.call("StopExecution", context))
    child, = run_worker_children(server.pid, worker, preview_worker)
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
    os.environ.get("CIRNET_NATIVE_STOP_DIAGNOSTIC") != "1",
    reason="Set CIRNET_NATIVE_STOP_DIAGNOSTIC=1 for the busy Stop diagnostic.",
)
def test_busy_stop_diagnostic(tmp_path):
    totals = []
    receipt = {
        **diagnostic_provenance(),
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
        for number in range(1, BLOCKS + 1):
            directory = tmp_path / f"block-{number}"
            directory.mkdir()
            latencies = []
            block = {"block": number, "stop_rpc_ns": latencies}
            receipt["blocks"].append(block)
            with native_server(directory) as (channel, pid, worker, _, preview_worker):
                server, client = psutil.Process(pid), Client(channel)
                before = server.cpu_times()
                start = monotonic_ns()
                max_server_rss = max_worker_rss = 0
                for index in range(SAMPLES):
                    elapsed, worker_rss = sample_stop(client, server, worker, preview_worker, index)
                    latencies.append(elapsed)
                    totals.append(elapsed)
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
        assert len(totals) == BLOCKS * SAMPLES
        receipt["status"] = "PASS"
    finally:
        receipt["totals"] = {"samples": len(totals), **percentiles(totals)}
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(output)


@pytest.mark.skipif(
    os.environ.get("CIRNET_NATIVE_RPC_DIAGNOSTIC") != "1",
    reason="Set CIRNET_NATIVE_RPC_DIAGNOSTIC=1 for the GetCapabilities diagnostic.",
)
def test_capabilities_throughput_diagnostic(tmp_path):
    totals = []
    elapsed = 0
    receipt = {
        **diagnostic_provenance(),
        "status": "FAIL",
        "workload": "Serial warm GetCapabilities RPC against virtual station",
        "samples_per_block": CAPABILITIES_SAMPLES,
        "blocks": [],
    }
    output = tmp_path / "capabilities-diagnostic.json"
    expected = bytes.fromhex(reference_record()["capabilities"]["virtual-station-01"])
    request = target("")
    try:
        for number in range(1, BLOCKS + 1):
            directory = tmp_path / f"block-{number}"
            directory.mkdir()
            latencies = []
            block = {"block": number, "rpc_ns": latencies}
            receipt["blocks"].append(block)
            with native_server(directory) as (channel, pid, *_):
                server = psutil.Process(pid)
                read = Client(channel).method("GetCapabilities")
                warm = read(request, timeout=3)
                assert warm.available and warm.mode == "simulation"
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
                totals.extend(latencies)
                elapsed += wall
                block.update(
                    wall_ns=wall,
                    rpc_per_second=CAPABILITIES_SAMPLES * 1e9 / wall,
                    server_cpu_seconds=(after.user - before.user) + (after.system - before.system),
                    server_rss_bytes=server.memory_info().rss,
                    **percentiles(latencies),
                )
        assert len(totals) == BLOCKS * CAPABILITIES_SAMPLES
        receipt["status"] = "PASS"
    finally:
        receipt["totals"] = {
            "samples": len(totals),
            "rpc_per_second": len(totals) * 1e9 / elapsed if elapsed else None,
            **percentiles(totals),
        }
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(output)
