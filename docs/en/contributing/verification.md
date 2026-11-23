# Hardware verification

[Tiếng Việt](../../vi/contributing/verification.md)

The Windows x64 workflow runs on pushes to `master` with exact Contracts Next and Simulator Next commits selected in the workflow. A manual run can override both commits. CI builds the worker and runs the Hardware parity suite.

Create the Actions secret `CIRNET_DEPENDENCIES_READ_TOKEN` in [the cirnetlab organization settings](https://github.com/organizations/cirnetlab/settings/secrets/actions) with **Selected repositories** access. Include the repositories that run the workflows. Use a fine-grained token owned by `cirnetlab` with `Contents: read` on the private dependencies listed below. Hardware needs `cirnet-contracts-next` and `cirnet-simulator-next`.

Use the pinned Rust toolchain. Allocate an owned external session with the Contracts Next artifact allocator. Set CARGO_TARGET_DIR inside the session and put temporary files and logs there. Run cargo fmt --all --check and cargo test --locked.

With the [Simulator environment](../guides/getting-started.md) set, run cargo test --locked --test simulator_parity -- --ignored. It exercises the current worker and the canonical Contracts resources. The Rust suite covers station cancellation, generation fences, worker shutdown and safe acknowledgement. The optional tests/rpc suite includes Python reference comparisons and requires separately supplied reference fixtures in an isolated environment. Python is used only for that verification lane.

Record the exact inputs and outcomes. A software regression result does not establish physical capability. Physical execution and ngspice remain UNAVAILABLE. Remote Next selects a system candidate and records the component artifact hashes.
