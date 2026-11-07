# Getting started with CIRNET Hardware

English · [Tiếng Việt](../../vi/guides/getting-started.md)

[Documentation index](../index.md)

## Run the selected Python service

In a fresh Python 3.11 or later environment, install the selected
`cirnet-hardware-service==0.1.2`, `cirnet-contracts==0.1.2`, and
`cirnet-simulator==0.1.2` wheels. Use the source revisions and SHA-256 digests
in the [CIRNET 0.1.4 release manifest](https://github.com/cirnetlab/cirnet-remote/blob/38aea6120920f48cea817827ac2277cce4bef25e/integration/release-set.json). Contracts supplies generated
RPC bindings. Hardware runs independently of API source and sibling checkouts.

Current source also builds version `0.1.2`. Verify a fresh build separately from the selected wheel.

~~~sh
python -m hardware_service.server
~~~

The default bind is `127.0.0.1:50051`. The service accepts loopback or Unix
socket bindings. Browsers reach lab operations through the Remote API. The
canonical RPC service is `cirnet.hardware.v2.HardwareService`. The bindings also
serve `netcircuit.hardware.v2.HardwareService`. Compatible clients retry that
name after `UNIMPLEMENTED`.

`CIRNET_*` settings take precedence over `NETCIRCUIT_*` aliases. The selected
profile uses the `legacy` simulation backend. Set `CIRNET_SIMULATION_WORKERS`
to the same value in Remote and Hardware (default `4`). Review the
[station and safety boundaries](../architecture/station-boundaries.md) before using a physical
station.

## Run the Rust Hardware candidate

Use the verified `cirnet-hardware` executable and matching Stage 1 C++ worker
from the candidate selection. Set the following values before launching.

| Setting | Value |
| --- | --- |
| `CIRNET_CONTRACTS_ROOT` | Absolute resource root containing the verified `contracts/` tree. |
| `CIRNET_SIM_CPP_WORKER` | Absolute path to the selected C++ worker. |
| `CIRNET_SIM_CPP_WORKER_SHA256` | Selected worker SHA-256. |
| `CIRNET_SIMULATION_BACKEND` | `cpp`. |
| `CIRNET_SIMULATION_WORKERS` | The same count in Remote and Hardware, between 1 and 64 for Rust Hardware. The default is `4`. |

~~~sh
cirnet-hardware --listen 127.0.0.1:50051
~~~

The Rust binary serves both RPC 2.0 names, advertises virtual digital execution
and reports physical stations unavailable. For Python Remote, set
`CIRNET_HARDWARE_PREVIEW=true` to use the internal preview RPC in this
candidate configuration. See [Rust build and test instructions](../contributing/verification.md#rust-hardware-candidate)
and the [Windows Gate 3 evidence](../history/stage-3-windows-verification.md) for the
measured r8-R2 combination. Use the selected Python service for the published
release profile.
