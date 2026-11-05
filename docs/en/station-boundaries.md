# Station and safety boundaries

English · [Tiếng Việt](../vi/station-boundaries.md)

[Documentation index](index.md)

Hardware owns local RPC 2.0, station lifecycles and private FPGA transport. Remote owns admission, API authorization, durable leases and quarantine, capture storage and acknowledgement of safe release. Consult the pinned [Lab-First architecture](https://github.com/cirnetlab/cirnet-remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/ARCHITECTURE.md) and [physical safety authority](https://github.com/cirnetlab/cirnet-remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/PHYSICAL_LAB_SAFETY.md).

| Station | Current behavior |
| --- | --- |
| Virtual | Each experiment has an independent session. Bounded spawned workers run the selected Simulator backend. The `legacy` backend supplies four Logic Analyzer channels and zero Scope or Generator channels. `ngspice` reports unavailable. Stop and reset invalidate pending generations. |
| Physical | Missing driver reports `OFFLINE` or unavailable, zero slots and channels, and `hardware_applied=false`. Serial and USB-SPI remain placeholder adapters. |

Physical Start requires a fresh scan and safety result, a matching executable station and experiment plan, observed endpoint and resource coverage, applied routing, guarded power and armed capture. Safety, topology and functional correctness are separate decisions. An unknown component or a topology mismatch alone does not determine electrical safety.

Digital NCAP bytes travel in `OperationReply.capture_payload`. The accompanying `result_json` contains metadata. Software tests, test drivers and RTL simulation do not certify electrical protection, a hardware ABI, real acquisition or timing.

Rollback stops admission and drains through acknowledged safe cleanup or quarantine before restoring matching Hardware, Contracts and Simulator artifacts and consumer pins. Preserve the database, captures, leases and quarantine. Never replay Start during rollback.
