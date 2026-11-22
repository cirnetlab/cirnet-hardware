# Build and start

[Tiếng Việt](../../vi/guides/getting-started.md)

Place cirnet-contracts-next alongside this checkout and build the Rust binary with cargo build --locked --bin cirnet-hardware. Build the C++ worker from cirnet-simulator-next. Set CIRNET_CONTRACTS_ROOT to the Contracts Next root, CIRNET_SIM_CPP_WORKER to the absolute path of the worker and CIRNET_SIM_CPP_WORKER_SHA256 to its SHA-256 digest.

Run cirnet-hardware --listen 127.0.0.1:50051. On Unix, a local socket is also available. Configure Remote to use the same local endpoint. See [verification](../contributing/verification.md) for the regression suite.
