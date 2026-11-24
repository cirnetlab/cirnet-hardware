# Station and safety boundaries

English · [Tiếng Việt](../../vi/architecture/station-boundaries.md)

[Documentation index](../index.md)

The 0.3.0-dev.1 service implements local Hardware RPC and supervises a
SHA-256-verified C++ Simulator worker. It exposes virtual stations.
[Verification](../contributing/verification.md)

| Owner | Boundary |
| --- | --- |
| Hardware | Local-only Hardware RPC 2.0, station lifecycle, Simulator worker supervision, guarded Start, safe Stop and private FPGA transport. Hardware reports failed cleanup so Remote can quarantine the lease while the station remains fenced. |
| Remote | Web API, authentication and authorization, durable leases, quarantine and capture persistence. Remote releases a lease only after accepted safe cleanup. |
| Simulator, Contracts and FPGA | Simulator owns electrical models and private CNP1/CNP2 framing. Contracts owns canonical RPC and logical schemas. FPGA owns RTL and board qualification. |

Use the [Lab-First Architecture](https://github.com/cirnetlab/cirnet-remote/blob/master/docs/cirnet-architecture-lab-first-en.md) for system safety and the [Remote physical safety contract](https://github.com/cirnetlab/cirnet-remote/blob/master/docs/en/architecture/physical-lab-safety-contract.md) for the application-to-station requirements.

| Station | Current behavior |
| --- | --- |
| Virtual | Each experiment has an independent session. The `cpp` backend uses supervised native workers selected by SHA-256. It exposes four Logic Analyzer channels and zero Scope or Generator channels. `ngspice` reports unavailable. |
| Physical | Missing driver reports `OFFLINE` or unavailable, zero slots and channels, and `hardware_applied=false`. Serial and USB-SPI remain placeholder adapters. |

For a qualified physical station, Start requires authorization, a fresh safe result, an executable plan for the exact station and experiment, and hardware evidence of protection and resource coverage. Bind endpoints to observed topology before applying infrastructure routing. The Physical Compiler preserves the student’s ICs and wiring. Guarded power and capture may follow only under the matching hardware permits. Safety, topology and functional correctness are separate decisions. An unknown component or a topology mismatch alone does not determine electrical safety.

Digital NCAP bytes travel in `OperationReply.capture_payload`. The accompanying `result_json` contains metadata. Software tests, test drivers and RTL simulation do not certify electrical protection, a hardware ABI, real acquisition or timing.

On Windows, the native service starts one hash-verified worker for the
dedicated preview lane and checks an empty CNP exchange before reporting
readiness. This prepares the preview validators and private pipes. Run
workers occupy a separate pool, and supervised shutdown retires both pools.

The private `cirnet.hardware.internal.v1.SimulatorPreview` endpoint has its
own version. `Describe` advertises integer `1` and `Preview` carries graph
and result JSON as protobuf bytes. Remote reports `HARDWARE_UNAVAILABLE` for
physical preview when `Describe` returns `UNIMPLEMENTED`. A different advertised
version or a missing `Preview` after successful negotiation is a protocol
error. Hardware uses CNP1 for digital worker frames and CNP2 for plans with a
DC projection. Worker validation checks the echoed tag, request ID and
generation. Release verification exercises digital NCAP parity and powered DC
on the selected worker digest.
[Private Preview definition](../../../proto/preview.proto) · [Worker parity suite](../../../tests/simulator_parity.rs)

Native Start rejects excess work immediately, leaving RPC
threads for Stop and reset. Both invalidate pending generations. For a native
job in flight, safe acknowledgement waits for confirmed cancellation and
cleanup. Hardware rejects the acknowledgement if it cannot confirm cleanup,
leaving the owner fenced so Remote can quarantine the lease.

Rollback stops admission and drains through acknowledged safe cleanup or quarantine before restoring matching Hardware, Contracts and Simulator artifacts and consumer pins. Preserve the database, captures, leases and quarantine. Never replay Start during rollback.
