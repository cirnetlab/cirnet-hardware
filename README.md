# CIRNET Hardware

[English documentation](docs/en/index.md) · [Tài liệu tiếng Việt](docs/vi/index.md)

CIRNET Hardware runs local Hardware RPC 2.0, manages virtual and physical station lifecycles, and owns private FPGA transport. CIRNET Remote owns API admission, durable leases, quarantine, and capture storage. [Lab-First architecture](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/ARCHITECTURE.md) · [Physical lab safety](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/PHYSICAL_LAB_SAFETY.md)

The selected CIRNET 0.1.4 system profile uses the Python `cirnet-hardware-service==0.1.2` wheel with matching Contracts and Simulator wheels. The Rust `cirnet-hardware` binary with the C++ Simulator worker is a separate virtual-only candidate. Gate 3 passed Windows x64 functionality, compatibility, and safety on r8-R2. Linux/Xeon performance, physical HIL, and production/release acceptance remain `NOT RUN`. [Selected release manifest](https://github.com/cirnetlab/cirnet-remote/blob/38aea6120920f48cea817827ac2277cce4bef25e/integration/release-set.json) · [Gate 3 evidence](docs/en/history/stage-3-windows-verification.md)

- [Start the service](docs/en/guides/getting-started.md)
- [Station and safety boundaries](docs/en/architecture/station-boundaries.md)
- [Build and verify](docs/en/contributing/verification.md)
