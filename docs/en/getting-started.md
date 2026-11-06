# Getting started with CIRNET Hardware

English · [Tiếng Việt](../vi/getting-started.md)

[Documentation index](index.md)

The current source builds `cirnet-hardware-service==0.1.2` for Python 3.11 or later. Install the Hardware wheel and its reviewed `cirnet-contracts==0.1.2` and `cirnet-simulator==0.1.2` wheels in a fresh environment. Use the exact artifact digests in the system release selection. Contracts supplies the generated RPC bindings. The Hardware install is independent of API source and sibling checkouts.

Start the service from the environment containing the installed wheels.

~~~sh
python -m hardware_service.server
~~~

The default address is `127.0.0.1:50051`. The service accepts loopback or Unix socket bindings. The browser reaches lab operations through the Remote API. The canonical service name is `cirnet.hardware.v2.HardwareService`. The generated bindings also serve `netcircuit.hardware.v2.HardwareService`, and the client tries that route only after `UNIMPLEMENTED`.

`CIRNET_*` settings take precedence over `NETCIRCUIT_*` aliases. For a virtual lab, align `CIRNET_SIMULATION_WORKERS` across API and Hardware. The default is `4`. The availability of physical stations depends on installed drivers and verified hardware. See [station and safety boundaries](station-boundaries.md) before interpreting an RPC result as physical readiness.

The published `v0.1.1` artifacts retain their original versions and SHA256 values. Reproduce those artifacts in a separate environment. For a new build, follow the [verification guide](verification.md).

## Rust Hardware candidate

The `cirnet-hardware` binary uses the pinned Rust Contracts crate and the
Stage 1 C++ worker. Select the absolute verified Contracts resource directory
with `CIRNET_CONTRACTS_ROOT`. Set `CIRNET_SIM_CPP_WORKER` to the absolute worker
path and `CIRNET_SIM_CPP_WORKER_SHA256` to its selected SHA-256. Align
`CIRNET_SIMULATION_WORKERS` with Remote and select `cpp` for the simulator.

~~~sh
cirnet-hardware --listen 127.0.0.1:50051
~~~

The service exposes both Hardware RPC 2.0 names. It advertises virtual digital
execution and reports the physical station unavailable. Python Remote can opt
in to the internal preview RPC with `CIRNET_HARDWARE_PREVIEW=true`. Enable this
combination only in the Stage 3 verification lane until its gate passes.

### Optional Stop timing proof on Windows

In the Stage 3 verification lane, a `StopExecution` call can request its
Hardware handler entry time by sending gRPC metadata
`x-cirnet-stop-ingress` with the value `qpc-ns-v1`. For an experiment ID of
32 lowercase hexadecimal characters, the service returns initial metadata
under the same key with the value `qpc-ns-v1:<experiment_id>:<nanoseconds>`.
The service samples Windows QueryPerformanceCounter at handler entry and
attaches the marker after executing Stop. [Windows QPC](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps)

The harness matches the marker to the experiment and checks its order
against the observation that all four workers are still alive, the target
worker exit, the safe ACK and durable lease release. Critical intervals must
exceed one QPC tick. Windows x64 verification passed 12 Hardware lifecycle
and protocol tests, including the metadata check for both RPC names, and
15 focused harness checks.

The unpaused r4 readiness diagnostic qualified 3 of 35 attempts with Python
Hardware B and 8 of 8 with Rust Hardware. The Python lane fell short of its
eight-sample target. The formal Stop retest for this candidate is `NOT RUN`.
It requires three windows of 100 qualified observations per side and the
existing p95/p99 limits.

### Optional Start alignment diagnostic on Windows

In the isolated Stage 3 verification lane, set
`CIRNET_HARDWARE_START_RENDEZVOUS=1` to enable an opt-in rendezvous at
`StartExecution` handler entry. Both RPC names share one rendezvous per
wave. A valid ticket in `x-cirnet-start-wave` names one wave and exactly
four distinct virtual experiment IDs. Each handler waits before reaching
station operations. A duplicate, a conflicting member, a cancellation
observed before release, or a missing member at the two-second deadline
aborts the group. Closed tickets remain closed for the lifetime of the
verification service. Calls without the ticket follow the ordinary Start
path.

The frozen Python B test wrapper and Rust Hardware each report all four
handler entry timestamps and a common Windows QPC release in gRPC response
metadata. Both use the same Remote dispatch rendezvous. The C++ workers
run unpaused with the pinned workload. The Stop harness still requires all
four workers to have read their exact request and written no reply through
the target worker exit, with a correlated Stop handler entry, safe ACK,
durable lease release and capture checks.

The Windows x64 Rust executable with SHA-256
`22d3d0a1c57aad0d2d01d512f3a890149566bc799228559b6943843cba1e85e6`
passed 15 focused Hardware RPC checks. The Python B wrapper and harness
passed 22 checks. With the r4 Remote wheels, the diagnostic qualified
8 of 10 attempts on Python B (80.0%) and 8 of 8 on Rust. The earlier
unaligned diagnostic qualified 3 of 35 on Python B (8.6%) and 8 of 8 on
Rust. The paired runs took 199.80 and 180.96 seconds respectively, with
different numbers of qualified observations and warmups.

All six diagnostic p95/p99 comparisons met the existing limits with eight
qualified observations per side. The receipt is
`native-hardware-stop/c27bcece564342159e0726c69ff8d16d/stop-summary.json`
with SHA-256
`001033e2476c81f65aae46fdb754f38203d8e91a717c8e299a514499b6aac712`.
This eight-sample diagnostic preceded the formal r8–R2 result below.

The approved formal variant is selected in the Remote harness by
`CIRNET_NATIVE_STAGE3_STOP_METHOD=natural-loaded-stop-hardware-rendezvous-v1`.
The unaligned method is the default. Each formal window allows up to 403
attempts to collect its 100 qualified observations per side. The final
executable passed all 25 Hardware lifecycle and RPC cases against the
pinned reference. Three Remote r7 cases also passed for durable lease
release on expiry or disconnect and quarantine without a safe ACK.

### Windows functional and safety acceptance

The Windows i5 acceptance scope is `windows-functional-safety-v1`. Select
it with `CIRNET_NATIVE_STAGE3_STOP_ACCEPTANCE_SCOPE` and use the approved
Hardware Start rendezvous method. The scope keeps three paired windows,
100 qualified samples per side per window and at most 403 attempts per
window. Each qualified observation must prove that all four workers are
active at Stop ingress, confirm the target reap before safe ACK and
verify capture fencing and durable lease disposition.

| Frozen operational Stop ceiling | p95 ms | p99 ms |
| --- | ---: | ---: |
| HTTP Stop | 197.554060 | 581.156400 |
| Initial gRPC ACK | 139.204780 | 526.711790 |
| Durable lease release | 2,534.556860 | 2,598.374680 |

These six frozen ceilings remain blocking on Windows. The 110% paired
comparison against Python B is recorded separately as a performance
diagnostic, as are Windows CPU, memory and throughput measurements.
Hardware cleanup and process reap deadlines remain mandatory. A missing
or unconfirmed cleanup ACK prevents safe release and Remote retains
quarantine. Linux/Xeon performance acceptance is `NOT RUN`. Its thresholds
must be set before acceptance.

#### r8–R2 loaded Stop result

**PASS** on Windows x64 under `windows-functional-safety-v1` with
`natural-loaded-stop-hardware-rendezvous-v1`. All six windows qualified
100 Stops each within the 403-attempt cap. Python B used 175, 153 and
174 attempts. Rust C used 100 in each window. Pytest took 2,953.75 seconds.
The timing values are medians of the three window percentiles.

| Stop timing in ms | Python B p95/p99 | Rust C p95/p99 | Frozen ceilings |
| --- | ---: | ---: | --- |
| HTTP Stop | 78.6032 / 100.8966 | 89.1368 / 100.4389 | PASS |
| Initial gRPC ACK | 56.3787 / 69.3376 | 62.2677 / 78.5544 | PASS |
| Durable lease release | 145.9619 / 168.5140 | 144.0377 / 163.8268 | PASS |

The paired performance diagnostic exceeded the 110%-of-B limit for HTTP
p95 (89.1368 vs 86.46352 ms), initial gRPC ACK p95 (62.2677 vs 62.01657 ms)
and initial gRPC ACK p99 (78.5544 vs 76.27136 ms). The other three paired
checks passed. The Windows Stop verdict uses the six frozen ceilings.

A read-only audit verified all 600 retained observations. Each used four
distinct experiments with one QPC release and four unpaused workers. The
counter snapshots confirmed four complete 2,806,507-byte requests and
zero reply writes through target exit. Each target exited before safe ACK
and durable lease release and had no capture. The audit rehashed all
1,800 peer NCAP files and confirmed empty leases and service shutdown
for all six lanes.

The selected manifest SHA-256 is
`ebc49c57c052e0a78c98074af240c78010f787872a4ad32ac23617ada1fc15bd`.
The Rust Hardware binary is
`22d3d0a1c57aad0d2d01d512f3a890149566bc799228559b6943843cba1e85e6`
and the r8 Remote wheel is
`0e3280dc41d81e1b00b4ee7a5e35a3646b06b32276181000ee874b43e113bd01`.
The receipt `native-hardware-stop/97612df8bfb942bfbb131dbaf78c1b10/stop-summary.json`
has SHA-256
`41b41bb221ec29118e324637119cf2fdfd5a267760b2b7a1da8d9da3310593b0`.

Linux/Xeon performance and physical HIL remain `NOT RUN`. The selected
configuration advertises virtual execution and reports physical execution
unavailable.
