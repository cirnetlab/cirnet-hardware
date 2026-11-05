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
