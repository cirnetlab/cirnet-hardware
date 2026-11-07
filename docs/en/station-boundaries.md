# Station and safety boundaries

English · [Tiếng Việt](../vi/station-boundaries.md)

[Documentation index](index.md)

Hardware owns local RPC 2.0, station lifecycles and private FPGA transport. Remote owns admission, API authorization, durable leases and quarantine, capture storage and acknowledgement of safe release. Consult the pinned [Lab-First architecture](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/ARCHITECTURE.md) and [physical safety authority](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/PHYSICAL_LAB_SAFETY.md).

| Station | Current behavior |
| --- | --- |
| Virtual | Each experiment has an independent session. The default `legacy` backend uses bounded spawned Python workers. The experimental `cpp` backend requires a matching Simulator artifact and uses its supervised native workers. Both expose four Logic Analyzer channels and zero Scope or Generator channels. `ngspice` reports unavailable. |
| Physical | Missing driver reports `OFFLINE` or unavailable, zero slots and channels, and `hardware_applied=false`. Serial and USB-SPI remain placeholder adapters. |

Physical Start requires a fresh scan and safety result, a matching executable station and experiment plan, observed endpoint and resource coverage, applied routing, guarded power and armed capture. Safety, topology and functional correctness are separate decisions. An unknown component or a topology mismatch alone does not determine electrical safety.

Digital NCAP bytes travel in `OperationReply.capture_payload`. The accompanying `result_json` contains metadata. Software tests, test drivers and RTL simulation do not certify electrical protection, a hardware ABI, real acquisition or timing.

Select the experimental backend with `CIRNET_SIMULATION_BACKEND=cpp` and the
reviewed Simulator worker path and digest in `CIRNET_SIM_CPP_WORKER` and
`CIRNET_SIM_CPP_WORKER_SHA256`. Set the same `CIRNET_SIMULATION_WORKERS` in
Remote and Hardware. Native Start rejects excess work immediately, leaving RPC
threads for Stop and reset. Both invalidate pending generations. For a native
job in flight, safe acknowledgement waits for confirmed cancellation and
cleanup. Hardware rejects the acknowledgement if it cannot confirm cleanup,
leaving the owner fenced so Remote can quarantine the lease.

Rollback stops admission and drains through acknowledged safe cleanup or quarantine before restoring matching Hardware, Contracts and Simulator artifacts and consumer pins. Preserve the database, captures, leases and quarantine. Never replay Start during rollback.
