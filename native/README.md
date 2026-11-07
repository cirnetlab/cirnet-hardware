# CIRNET Hardware Rust service

This directory contains the Rust `cirnet-hardware` binary, its local RPC and station implementation, and the adapter that supervises the separate C++ Simulator worker. The candidate serves both Hardware RPC 2.0 namespaces for virtual digital execution and reports physical stations unavailable. The root `Cargo.toml` defines the crate. `tests/` contains the native parity and loopback RPC suites.

- [Candidate setup](../docs/en/getting-started.md#run-the-rust-hardware-candidate) · [Cấu hình ứng viên](../docs/vi/getting-started.md#chạy-ứng-viên-hardware-rust)
- [Build and verification](../docs/en/verification.md#rust-hardware-candidate) · [Dựng và kiểm chứng](../docs/vi/verification.md#ứng-viên-hardware-rust)
