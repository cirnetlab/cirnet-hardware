# Hardware verification

[Tiáº¿ng Viá»‡t](../../vi/contributing/verification.md)

The Windows x64 workflow runs on pushes to `master` with exact Contracts Next and Simulator Next commits selected in the workflow. A manual run can override both commits. CI builds the worker and runs Hardware’s Rust parity and RPC lifecycle suites.

Create the Actions secret `CIRNET_DEPENDENCIES_READ_TOKEN` in [the cirnetlab organization settings](https://github.com/organizations/cirnetlab/settings/secrets/actions) with **Selected repositories** access. Include the repositories that run the workflows. Use a fine-grained token owned by `cirnetlab` with `Contents: read` on the private dependencies listed below. Hardware needs `cirnet-contracts-next` and `cirnet-simulator-next`.

Use the pinned Rust toolchain. Allocate an owned external session with the Contracts Next artifact allocator. Set CARGO_TARGET_DIR inside the session and put temporary files and logs there. Run cargo fmt --all --check and cargo test --locked.

With the [Simulator environment](../guides/getting-started.md) set, run `cargo test --locked --test simulator_parity -- --ignored`. It exercises the selected worker and the canonical Contracts resources, including rejection of an incorrect digest. The Rust suite covers station cancellation, generation fences, worker shutdown, safe acknowledgement and malformed NCAP evidence. The `tests/rpc` suite checks that plans, captures and Stop stay bound to their experiment. The suite uses checked-in wire and NCAP fixtures from the verified reference. Python is used only for verification.

For the RPC suite, use Python 3.14 in an external virtual environment. Install the third-party wheels with `python -m pip install --only-binary=:all: --require-hashes -r requirements-verification.lock`. For offline runs, add `--no-index --find-links <wheelhouse>`. Generate the test bindings from the selected Contracts Next schema with the Protobuf compiler from the Cargo-locked `protoc-bin-vendored` package for your platform.

```powershell
$proto = Join-Path $env:CIRNET_CONTRACTS_ROOT 'contracts/hardware/v2'
$bindings = Join-Path $env:CIRNET_ARTIFACTS_SESSION 'rpc-bindings'
New-Item -ItemType Directory -Path $bindings
& $protoc --proto_path=$proto --python_out=$bindings (Join-Path $proto 'rpc_v2.proto')
$env:PYTHONPATH = $bindings
$env:CIRNET_NATIVE_HARDWARE = $hardware
$env:CIRNET_NATIVE_HARDWARE_SHA256 = $hardwareSha256
python -B -m pytest -p no:cacheprovider --basetemp (Join-Path $env:CIRNET_ARTIFACTS_SESSION 'rpc-pytest') tests/rpc
```

Use the selected absolute executable and reviewed digest for `$hardware` and `$hardwareSha256`, along with the selected Simulator worker and digest. Set `CIRNET_WINDOWS_PREPARED_PREVIEW=1` when the selected Windows Hardware instance enables a dedicated prepared Preview worker. Set `CIRNET_NATIVE_STOP_DIAGNOSTIC=1` or `CIRNET_NATIVE_RPC_DIAGNOSTIC=1` to measure native busy Stop or capabilities throughput. Keep the new receipts distinct from the [historical diagnostic results](../../../tests/rpc/fixtures/README.md).

Record the exact inputs and outcomes. A software regression result does not establish physical capability. Physical execution and ngspice remain UNAVAILABLE. Remote Next selects a system candidate and records the component artifact hashes.
